use crate::metadata::{GameInfo, identify};
use game_integrations::{
    Agents,
    control::{Decision, GAMEPLAY, Phase},
};
use nes_core::{cartridge::Cartridge, machine::Machine};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Human,
    Ai,
    Shared,
    Off,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Human => "Human",
            Self::Ai => "AI",
            Self::Shared => "Shared",
            Self::Off => "No input",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Model {
    pub manifest: String,
    pub weights: Vec<u8>,
    pub name: String,
    pub scope: String,
}
impl Model {
    pub fn new(manifest: String, weights: Vec<u8>) -> Result<Self, String> {
        let b = game_integrations::Bundle::load(&manifest, &weights).map_err(|e| e.to_string())?;
        Ok(Self {
            name: b.manifest.name.clone(),
            scope: crate::metadata::scope(&b.manifest.observation).into(),
            manifest,
            weights,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Ready,
    Running,
    Paused,
    Handoff(String),
    Faulted(String),
    Failed(String),
}
pub struct Session {
    pub machine: Machine,
    pub rom: Vec<u8>,
    pub info: GameInfo,
    pub agents: Agents,
    pub modes: [Mode; 2],
    pub models: [Option<Model>; 2],
    pub ownership: [u8; 2],
    pub state: SessionState,
    pub has_run: bool,
    pub decisions: Option<[Decision; 2]>,
    pub pulse: [u8; 2],
    pulse_held: [u8; 2],
    pulse_frames: [u8; 2],
    pub alternate: bool,
}
impl Session {
    pub fn new(name: &str, rom: Vec<u8>) -> Result<Self, String> {
        let machine = Machine::new(Cartridge::from_ines(&rom).map_err(|e| e.to_string())?);
        let info = identify(&machine.cartridge_hash(), name);
        let modes = [
            Mode::Human,
            if info.players == 1 {
                Mode::Off
            } else {
                Mode::Human
            },
        ];
        Ok(Self {
            machine,
            rom,
            info,
            agents: Agents::default(),
            modes,
            models: [None, None],
            ownership: [2, 2],
            state: SessionState::Ready,
            has_run: false,
            decisions: None,
            pulse: [0; 2],
            pulse_held: [0; 2],
            pulse_frames: [0; 2],
            alternate: false,
        })
    }
    pub fn running(&self) -> bool {
        self.state == SessionState::Running
    }
    fn record_failure(&mut self, error: &str) {
        self.state = if self.machine.fault().is_some() {
            SessionState::Faulted(error.into())
        } else if self.unsupported().unwrap_or(false) {
            SessionState::Handoff(error.into())
        } else {
            SessionState::Failed(error.into())
        };
    }
    pub fn adopt_transport(&mut self, bytes: &[u8], error: Option<&str>) -> Result<(), String> {
        self.machine
            .adopt_transport_state(bytes)
            .map_err(|e| e.to_string())?;
        self.state = SessionState::Paused;
        self.has_run = true;
        self.release_inputs();
        if let Some(e) = error {
            self.record_failure(e);
        }
        Ok(())
    }
    pub fn pause(&mut self) {
        if self.running() {
            self.state = SessionState::Paused;
        }
        self.release_inputs();
        self.machine.set_zapper(-1, -1, false);
        self.machine.take_audio();
    }
    pub fn resume(&mut self) -> Result<(), String> {
        self.resume_playback(false)
    }
    pub fn resume_playback(&mut self, replay: bool) -> Result<(), String> {
        if let Some(e) = self.machine.fault() {
            let message =
                format!("Emulation stopped: {e}. Restart or open a valid save to continue.");
            self.state = SessionState::Faulted(message.clone());
            return Err(message);
        }
        if !replay && self.unsupported()? {
            let message="This agent has reached an unsupported stage. Take full control or choose a compatible agent.".to_string();
            self.state = SessionState::Handoff(message.clone());
            return Err(message);
        }
        self.state = SessionState::Running;
        self.has_run = true;
        Ok(())
    }
    pub fn unsupported(&self) -> Result<bool, String> {
        Ok(self
            .agents
            .phases(&self.machine)
            .map_err(|e| e.to_string())?
            .contains(&Phase::UnsupportedGameplay))
    }
    pub fn set_mode(&mut self, slot: usize, mode: Mode) -> Result<(), String> {
        if slot > 1 {
            return Err("Player must be 0 or 1.".into());
        }
        if self.running() {
            return Err("Pause before changing controls.".into());
        }
        if matches!(mode, Mode::Ai | Mode::Shared) {
            let model = self.models[slot].as_ref().ok_or("Choose an agent first.")?;
            self.agents
                .load(&self.machine, slot, &model.manifest, &model.weights)
                .map_err(|e| e.to_string())?;
            self.agents
                .set_ownership(
                    slot,
                    if mode == Mode::Ai {
                        GAMEPLAY
                    } else {
                        self.ownership[slot]
                    },
                )
                .map_err(|e| e.to_string())?;
        } else {
            self.agents.clear(slot).map_err(|e| e.to_string())?;
        }
        self.modes[slot] = mode;
        self.decisions = None;
        self.release_inputs();
        Ok(())
    }
    pub fn load_model(&mut self, slot: usize, model: Model, mode: Mode) -> Result<(), String> {
        if slot > 1 {
            return Err("Player must be 0 or 1.".into());
        }
        if self.running() {
            return Err("Pause before changing agents.".into());
        }
        // Validation happens before the current assignment changes.
        self.agents
            .load(&self.machine, slot, &model.manifest, &model.weights)
            .map_err(|e| e.to_string())?;
        self.models[slot] = Some(model);
        self.set_mode(slot, mode)
    }
    pub fn set_ownership(&mut self, slot: usize, mask: u8) -> Result<(), String> {
        if slot > 1 {
            return Err("Player must be 0 or 1.".into());
        }
        if self.running() {
            return Err("Pause before changing ownership.".into());
        }
        if !game_integrations::control::valid_ownership(mask) {
            return Err("Keep direction pairs together.".into());
        }
        self.agents
            .set_ownership(slot, mask)
            .map_err(|e| e.to_string())?;
        self.ownership[slot] = mask;
        self.release_inputs();
        self.decisions = None;
        Ok(())
    }
    pub fn release_inputs(&mut self) {
        self.pulse = [0; 2];
        self.pulse_held = [0; 2];
        self.pulse_frames = [0; 2];
        self.machine.set_controllers([0; 2]);
    }
    pub fn frame(&mut self, mut human: [u8; 2]) -> Result<Vec<f32>, String> {
        for (i, h) in human.iter_mut().enumerate() {
            // An on-screen Start/Select click must survive games that only
            // poll controllers on alternate frames. This is three NES frames,
            // independent of host refresh rate or catch-up batching.
            if self.pulse[i] != 0 {
                self.pulse_held[i] |= self.pulse[i];
                self.pulse_frames[i] = 3;
            }
            *h = if self.modes[i] == Mode::Off {
                0
            } else {
                *h | self.pulse_held[i]
            };
            self.pulse_frames[i] = self.pulse_frames[i].saturating_sub(1);
            if self.pulse_frames[i] == 0 {
                self.pulse_held[i] = 0;
            }
        }
        self.pulse = [0; 2];
        match crate::runner::advance(
            &mut self.machine,
            crate::runner::Input::Human {
                agents: &self.agents,
                buttons: human,
            },
        ) {
            Ok(frame) => {
                self.decisions = frame.decisions;
                Ok(frame.audio)
            }
            Err(e) => {
                self.pause();
                self.record_failure(&e);
                Err(e)
            }
        }
    }
    pub fn replay_frame(&mut self, buttons: [u8; 2]) -> Result<Vec<f32>, String> {
        match crate::runner::advance(&mut self.machine, crate::runner::Input::Replay(buttons)) {
            Ok(frame) => {
                self.decisions = None;
                Ok(frame.audio)
            }
            Err(e) => {
                self.pause();
                self.record_failure(&e);
                Err(e)
            }
        }
    }
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
        nes_core::state_file::restore(&mut self.machine, bytes).map_err(|e| e.to_string())?;
        self.has_run = true;
        self.pause();
        self.state = SessionState::Paused;
        self.decisions = None;
        Ok(())
    }
    pub fn reset(&mut self) -> Result<(), String> {
        let battery = self.machine.battery_ram().map(|b| b.to_vec());
        let mut cart = Cartridge::from_ines(&self.rom).map_err(|e| e.to_string())?;
        if self.alternate {
            cart.set_mmc3_alternate(true).map_err(|e| e.to_string())?;
        }
        let mut next = Machine::new(cart);
        if let Some(b) = battery {
            next.restore_battery_ram(&b).map_err(|e| e.to_string())?;
        }
        self.machine = next;
        self.pause();
        self.has_run = false;
        self.state = SessionState::Ready;
        self.decisions = None;
        Ok(())
    }
    pub fn contra_two_player(&mut self) -> Result<(), String> {
        let next = crate::episode::Episode::new(&self.rom, 108000).map_err(|e| e.to_string())?;
        self.machine = next.machine;
        self.pause();
        self.has_run = true;
        self.state = SessionState::Paused;
        self.decisions = None;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_click_survives_three_emulated_frames_and_pause_cancels_it() {
        let mut s = Session::new("Playground", nes_core::diagnostic::playground_rom()).unwrap();
        s.pulse = [8, 4];
        for _ in 0..3 {
            s.frame([0; 2]).unwrap();
            assert_eq!(
                s.decisions.as_ref().unwrap().each_ref().map(|d| d.applied),
                [8, 4]
            );
        }
        s.frame([0; 2]).unwrap();
        assert_eq!(
            s.decisions.as_ref().unwrap().each_ref().map(|d| d.applied),
            [0, 0]
        );
        s.pulse = [8, 4];
        s.frame([0; 2]).unwrap();
        s.pause();
        s.resume().unwrap();
        s.frame([0; 2]).unwrap();
        assert_eq!(
            s.decisions.as_ref().unwrap().each_ref().map(|d| d.applied),
            [0, 0]
        );
    }
    #[test]
    fn pause_never_queues_start_and_bad_restore_is_transactional() {
        let mut s = Session::new("Playground", nes_core::diagnostic::playground_rom()).unwrap();
        s.resume().unwrap();
        s.pulse = [8, 4];
        s.pause();
        assert_eq!(s.pulse, [0, 0]);
        let before = s.machine.save_state();
        assert!(s.restore(b"garbage").is_err());
        assert_eq!(s.machine.save_state(), before);
        s.resume().unwrap();
        assert_eq!(s.pulse, [0, 0]);
        assert!(s.set_mode(0, Mode::Off).is_err());
    }
    #[test]
    fn fatal_error_blocks_resume_until_restoration_or_reset() {
        let mut rom = nes_core::diagnostic::controller_rom();
        rom[16] = 0x02;
        let mut s = Session::new("Fault", rom).unwrap();
        let before = s.machine.save_state();
        s.resume().unwrap();
        assert!(s.frame([0; 2]).is_err());
        s.pause();
        let stopped = s.machine.save_state();
        assert!(s.resume().is_err());
        assert_eq!(s.machine.save_state(), stopped);
        assert!(s.restore(b"bad save").is_err());
        assert!(s.resume().is_err());
        s.restore(&before).unwrap();
        assert!(s.resume().is_ok());
        assert!(s.frame([0; 2]).is_err());
        s.reset().unwrap();
        assert!(s.resume().is_ok());
    }
    #[test]
    fn no_input_releases_all_buttons_and_rejects_invalid_ownership() {
        let mut s = Session::new("Controllers", nes_core::diagnostic::controller_rom()).unwrap();
        s.set_mode(1, Mode::Off).unwrap();
        s.resume().unwrap();
        s.frame([0xff, 0xff]).unwrap();
        let d = s.decisions.as_ref().unwrap();
        assert_eq!(d[1].applied, 0);
        assert_eq!(d[0].applied & 12, 12);
        s.pause();
        assert!(s.set_ownership(0, 128).is_err());
    }
}
