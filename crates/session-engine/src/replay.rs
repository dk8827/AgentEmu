//! Durable replay metadata and checkpoint validation.
use crate::Session;
use nes_core::machine::Machine;
use serde::{Deserialize, Serialize};
#[derive(Clone, Deserialize, Serialize)]
pub struct Replay {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<Checkpoint>,
    pub game: String,
    pub reset: String,
    pub rom_sha256: String,
    pub warmup: u32,
    pub inputs: Vec<[u8; 2]>,
    pub result: ReplayResult,
    pub entry_state_sha256: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct ReplayResult {
    pub final_state_sha256: String,
    pub input_sha256: String,
}
/// Bump on emulation changes that intentionally invalidate deterministic traces.
/// Application versions and visual redesigns do not change this contract.
pub const CORE_ABI: &str = "nes-frame-state6-v1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub system: String,
    pub core_abi: String,
    pub producer: String,
    pub timing: String,
    pub mapper: u8,
    pub submapper: u8,
    pub prg_sha256: String,
    pub chr_sha256: String,
    pub policy_sha256: [Option<String>; 2],
}
impl Provenance {
    pub fn current(machine: &Machine) -> Self {
        let identity = machine.cartridge_identity();
        Self {
            system: "nes".into(),
            core_abi: CORE_ABI.into(),
            producer: concat!("session-engine/", env!("CARGO_PKG_VERSION")).into(),
            timing: if machine.frame_rate() < 55. {
                "pal"
            } else {
                "ntsc"
            }
            .into(),
            mapper: machine.mapper_id(),
            submapper: identity.submapper,
            prg_sha256: identity.prg_sha256.clone(),
            chr_sha256: identity.chr_sha256.clone(),
            policy_sha256: [None, None],
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub frame: usize,
    pub state_sha256: String,
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Replay {
    pub fn validate_metadata(&self, machine: &Machine) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        if self.rom_sha256 != machine.cartridge_hash()
            || self.inputs.is_empty()
            || self.inputs.len() > 108000
            || self.warmup > 240
        {
            return Err("Replay does not match this cartridge or frame budget".into());
        }
        let bytes: Vec<u8> = self.inputs.iter().flatten().copied().collect();
        if format!("{:x}", Sha256::digest(&bytes)) != self.result.input_sha256 {
            return Err("Replay input checksum failed".into());
        }
        if !digest(&self.result.final_state_sha256) || !digest(&self.result.input_sha256) {
            return Err("Replay verification hashes are invalid".into());
        }
        if self.version == 1 {
            if self.provenance.is_some() || !self.checkpoints.is_empty() {
                return Err("Replay v2 metadata requires version 2".into());
            }
            return Ok(());
        }
        if self.version != 2 {
            return Err("Unsupported replay version".into());
        }
        let p = self
            .provenance
            .as_ref()
            .ok_or("Replay v2 requires provenance")?;
        let current = Provenance::current(machine);
        if p.system != current.system
            || p.core_abi != current.core_abi
            || p.timing != current.timing
            || p.mapper != current.mapper
            || p.submapper != current.submapper
            || p.prg_sha256 != current.prg_sha256
            || p.chr_sha256 != current.chr_sha256
            || p.producer.is_empty()
            || p.producer.len() > 128
            || p.policy_sha256.iter().flatten().any(|h| !digest(h))
        {
            return Err("Replay hardware or core contract does not match".into());
        }
        if self.checkpoints.is_empty() || self.checkpoints.len() > 1024 {
            return Err("Replay v2 requires 1..1024 checkpoints".into());
        }
        let mut previous = 0;
        for checkpoint in &self.checkpoints {
            if checkpoint.frame <= previous
                || checkpoint.frame > self.inputs.len()
                || !digest(&checkpoint.state_sha256)
            {
                return Err("Replay checkpoints must be valid and strictly ordered".into());
            }
            previous = checkpoint.frame;
        }
        let last = self.checkpoints.last().unwrap();
        if last.frame != self.inputs.len() || last.state_sha256 != self.result.final_state_sha256 {
            return Err("Replay final checkpoint does not match its result".into());
        }
        Ok(())
    }
    pub fn verify_checkpoint(&self, frame: usize, machine: &Machine) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        if let Ok(index) = self.checkpoints.binary_search_by_key(&frame, |c| c.frame)
            && format!("{:x}", Sha256::digest(machine.save_state()))
                != self.checkpoints[index].state_sha256
        {
            return Err(format!("Replay diverged at frame {frame}"));
        }
        Ok(())
    }

    pub fn prepare(bytes: &[u8], session: &Session) -> Result<(Self, Machine), String> {
        use sha2::{Digest, Sha256};
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Replay exceeds 16 MiB.".into());
        }
        let r: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        r.validate_metadata(&session.machine)?;
        if r.rom_sha256 != session.machine.cartridge_hash()
            || r.warmup > 240
            || r.inputs.is_empty()
            || r.inputs.len() > 108000
        {
            return Err("Replay does not match this cartridge or format.".into());
        }
        let expected = game_integrations::registry::task(&r.game)
            .ok_or("Unknown replay task.")?
            .reset;
        if r.reset != expected {
            return Err("Replay reset protocol does not match its task.".into());
        }
        let input: Vec<u8> = r.inputs.iter().flatten().copied().collect();
        if format!("{:x}", Sha256::digest(&input)) != r.result.input_sha256 {
            return Err("Replay input checksum failed.".into());
        }
        let m = if r.game == "contra-jungle" {
            let mut e =
                crate::episode::Episode::new(&session.rom, 108000).map_err(|e| e.to_string())?;
            e.reset_timing(r.warmup).map_err(|e| e.to_string())?;
            e.machine
        } else {
            let entry = if r.game == "contra-base1" {
                let b = session.machine.save_state();
                if Some(format!("{:x}", Sha256::digest(&b))) != r.entry_state_sha256 {
                    return Err("Open the replay's exact Base 1 entry save first.".into());
                }
                Some(b)
            } else {
                None
            };
            let task = game_integrations::tasks::Task::parse(&r.game).map_err(|e| e.to_string())?;
            let mut e = crate::tasks::Episode::new(&session.rom, task, 108000, entry.as_deref())
                .map_err(|e| e.to_string())?;
            e.reset(r.warmup).map_err(|e| e.to_string())?;
            e.machine
        };
        Ok((r, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn replay_metadata_rejects_wrong_hardware_and_locates_first_divergence() {
        let mut session = Session::new("Homebrew", nes_core::diagnostic::controller_rom()).unwrap();
        let inputs = vec![[128, 1], [129, 2]];
        let start = session.machine.save_state();
        let mut checkpoints = Vec::new();
        for (i, input) in inputs.iter().enumerate() {
            session.replay_frame(*input).unwrap();
            checkpoints.push(Checkpoint {
                frame: i + 1,
                state_sha256: format!("{:x}", Sha256::digest(session.machine.save_state())),
            });
        }
        let mut replay = Replay {
            version: 2,
            game: "homebrew-test".into(),
            reset: "power-on".into(),
            rom_sha256: session.machine.cartridge_hash(),
            warmup: 0,
            inputs,
            result: ReplayResult {
                input_sha256: format!("{:x}", Sha256::digest([128, 1, 129, 2])),
                final_state_sha256: checkpoints.last().unwrap().state_sha256.clone(),
            },
            entry_state_sha256: None,
            provenance: Some(Provenance::current(&session.machine)),
            checkpoints,
        };
        replay.validate_metadata(&session.machine).unwrap();
        session.restore(&start).unwrap();
        session.replay_frame([128, 1]).unwrap();
        replay.verify_checkpoint(1, &session.machine).unwrap();
        session.replay_frame([0, 0]).unwrap();
        assert!(
            replay
                .verify_checkpoint(2, &session.machine)
                .unwrap_err()
                .contains("frame 2")
        );
        replay.provenance.as_mut().unwrap().core_abi = "future".into();
        assert!(replay.validate_metadata(&session.machine).is_err());
        replay.provenance = Some(Provenance::current(&session.machine));
        replay.checkpoints[0].frame = 2;
        assert!(replay.validate_metadata(&session.machine).is_err());
        replay.version = 1;
        assert!(replay.validate_metadata(&session.machine).is_err());
        replay.provenance = None;
        replay.checkpoints.clear();
        replay.validate_metadata(&session.machine).unwrap();
    }
}
