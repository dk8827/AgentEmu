pub use crate::contracts::adapter::ARCHITECTURE;
use crate::{Result, adapter::INPUTS};
use agent_runtime::{FixedPolicy, decode};
use nes_core::machine::Machine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub const ACTION_SCHEMA: &str = "nes-factorized-3-3-2-2-v1";
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub name: String,
    pub architecture: String,
    pub observation: String,
    pub action: String,
    pub rom_sha256: String,
    pub weights_sha256: String,
    pub decision_frames: u32,
    pub training: serde_json::Value,
}
pub struct Bundle {
    pub manifest: Manifest,
    actor: Actor,
}
enum Actor {
    Autonomous(FixedPolicy<INPUTS>),
    Assistance(FixedPolicy<268>),
    History(FixedPolicy<272>),
    HistoryShared(FixedPolicy<284>),
    Aim(FixedPolicy<296>),
    AimShared(FixedPolicy<308>),
}
impl Bundle {
    pub fn load(manifest: &str, weights: &[u8]) -> Result<Self> {
        if manifest.len() > 16 * 1024 {
            return Err("manifest exceeds 16 KiB".into());
        }
        let m: Manifest = serde_json::from_str(manifest)?;
        if m.version != 1
            || !crate::registry::schema(&m.observation).is_some_and(|d| {
                d.rom_sha256 == m.rom_sha256
                    && ((d.architecture == m.architecture && d.observation == m.observation)
                        || (d.assistance_architecture == m.architecture
                            && d.assistance_observation == m.observation)
                        || d.variants.iter().any(|v| {
                            v.architecture == m.architecture && v.observation == m.observation
                        }))
            })
            || m.action != ACTION_SCHEMA
            || m.decision_frames != 1
            || m.name.is_empty()
            || m.name.len() > 120
        {
            return Err("incompatible agent model manifest".into());
        }
        if weights.len() > agent_runtime::MAX_WEIGHT_BYTES {
            return Err("weights exceed limit".into());
        }
        if format!("{:x}", Sha256::digest(weights)) != m.weights_sha256 {
            return Err("weights digest mismatch".into());
        }
        let definition = crate::registry::schema(&m.observation).ok_or("unknown observation")?;
        let actor = if let Some(v) = crate::registry::variant(&m.observation) {
            match v.inputs {
                272 => Actor::History(FixedPolicy::load(weights)?),
                284 => Actor::HistoryShared(FixedPolicy::load(weights)?),
                296 => Actor::Aim(FixedPolicy::load(weights)?),
                308 => Actor::AimShared(FixedPolicy::load(weights)?),
                _ => return Err("unsupported policy width".into()),
            }
        } else if definition.assistance_observation == m.observation {
            Actor::Assistance(FixedPolicy::load(weights)?)
        } else {
            Actor::Autonomous(FixedPolicy::load(weights)?)
        };
        Ok(Self { manifest: m, actor })
    }
    pub fn logits(&self, observation: &[f32]) -> Result<[f32; 10]> {
        Ok(match &self.actor {
            Actor::Autonomous(a) => a.forward(observation)?,
            Actor::Assistance(a) => a.forward(observation)?,
            Actor::History(a) => a.forward(observation)?,
            Actor::HistoryShared(a) => a.forward(observation)?,
            Actor::Aim(a) => a.forward(observation)?,
            Actor::AimShared(a) => a.forward(observation)?,
        })
    }
    pub fn buttons_with_context(
        &self,
        observation: &[f32],
        human: u8,
        ownership: u8,
    ) -> Result<u8> {
        match self.actor {
            Actor::Autonomous(_) | Actor::History(_) | Actor::Aim(_) => self.buttons(observation),
            Actor::Assistance(_) | Actor::HistoryShared(_) | Actor::AimShared(_) => self.buttons(
                &crate::control::assistance_features(observation, human, ownership)?,
            ),
        }
    }
    pub fn buttons(&self, observation: &[f32]) -> Result<u8> {
        Ok(decode(&self.logits(observation)?)?)
    }
}
#[derive(Default)]
pub struct Agents {
    ports: [Option<Bundle>; 2],
    enabled: [bool; 2],
    ownership: [u8; 2],
}
impl Agents {
    pub fn load(&mut self, m: &Machine, slot: usize, manifest: &str, weights: &[u8]) -> Result<()> {
        if slot > 1 {
            return Err("player must be 0 or 1".into());
        }
        let next = Bundle::load(manifest, weights)?;
        if m.cartridge_hash() != next.manifest.rom_sha256 {
            return Err("model requires its exact ROM revision".into());
        }
        self.ports[slot] = Some(next);
        self.enabled[slot] = true;
        self.ownership[slot] = crate::control::GAMEPLAY;
        Ok(())
    }
    pub fn clear(&mut self, slot: usize) -> Result<()> {
        if slot > 1 {
            return Err("player must be 0 or 1".into());
        }
        self.ports[slot] = None;
        self.enabled[slot] = false;
        Ok(())
    }
    pub fn toggle(&mut self, slot: usize) -> Result<()> {
        if slot > 1 || self.ports[slot].is_none() {
            return Err("load an agent for this player first".into());
        }
        self.enabled[slot] = !self.enabled[slot];
        Ok(())
    }
    pub fn active(&self) -> [bool; 2] {
        std::array::from_fn(|p| self.enabled[p] && self.ports[p].is_some())
    }
    /// Change ownership without changing the loaded network. Direction pairs are atomic.
    pub fn set_ownership(&mut self, slot: usize, ai_groups: u8) -> Result<()> {
        if slot > 1 || !crate::control::valid_ownership(ai_groups) {
            return Err("invalid control-group ownership".into());
        }
        if self.ports[slot].is_none() {
            return Err("load an agent first".into());
        }
        self.ownership[slot] = ai_groups;
        Ok(())
    }
    fn phase_observation(
        &self,
        m: &Machine,
        slot: usize,
    ) -> Result<(crate::control::Phase, Option<Vec<f32>>)> {
        use crate::control::Phase;
        let Some(p) = self.ports[slot].as_ref().filter(|_| self.enabled[slot]) else {
            return Ok((Phase::Menu, None));
        };
        if m.cartridge_hash() != p.manifest.rom_sha256 {
            return Err("model ROM mismatch".into());
        }
        crate::registry::policy_observation(m, slot, &p.manifest.observation)
    }

    pub fn phases(&self, m: &Machine) -> Result<[crate::control::Phase; 2]> {
        Ok([
            self.phase_observation(m, 0)?.0,
            self.phase_observation(m, 1)?.0,
        ])
    }
    /// Same immutable pre-frame observations, shared by every frontend.
    pub fn decisions(&self, m: &Machine, human: [u8; 2]) -> Result<[crate::control::Decision; 2]> {
        use crate::control::{Decision, Phase, arbitrate, normalize};
        let mut decisions = human.map(|h| Decision {
            human: normalize(h),
            proposal: None,
            applied: normalize(h),
            ai_groups: 0,
            phase: Phase::Menu,
        });
        for slot in 0..2 {
            let Some(p) = self.ports[slot].as_ref().filter(|_| self.enabled[slot]) else {
                continue;
            };
            let (phase, observation) = self.phase_observation(m, slot)?;
            let proposal = observation
                .map(|o| p.buttons_with_context(&o, human[slot], self.ownership[slot]))
                .transpose()?;
            let applied = if phase == Phase::Menu {
                normalize(human[slot])
            } else {
                arbitrate(human[slot], proposal.unwrap_or(0), self.ownership[slot])
            };
            decisions[slot] = Decision {
                human: normalize(human[slot]),
                proposal,
                applied,
                ai_groups: self.ownership[slot],
                phase,
            };
        }
        Ok(decisions)
    }
    pub fn controls(&self, m: &Machine, human: [u8; 2]) -> Result<[u8; 2]> {
        let decisions = self.decisions(m, human)?;
        if decisions
            .iter()
            .any(|d| d.phase == crate::control::Phase::UnsupportedGameplay)
        {
            return Err("Agent does not support this stage or game mode; take human control before continuing".into());
        }
        Ok(decisions.map(|d| d.applied))
    }
}
