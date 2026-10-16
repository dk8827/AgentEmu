//! Headless real-time execution protocol. Hosts own only clocks and transport;
//! frame stepping, input pulses, replay and failure decisions stay in Rust.
use crate::{Mode, Model, Replay, Session, clock::FrameClock};
use game_integrations::control::Decision;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Setup {
    pub name: String,
    pub models: [Option<Model>; 2],
    pub modes: [Mode; 2],
    pub ownership: [u8; 2],
    pub alternate: bool,
    pub replay: Option<(Replay, usize)>,
}
impl Setup {
    pub fn from_session(s: &Session, replay: Option<(Replay, usize)>) -> Self {
        Self {
            name: s.info.title.clone(),
            models: s.models.clone(),
            modes: s.modes,
            ownership: s.ownership,
            alternate: s.alternate,
            replay,
        }
    }
}
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct Input {
    pub human: [u8; 2],
    pub pulse: [u8; 2],
    pub aim: [i16; 2],
    pub fire_frames: u8,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct View {
    pub frame: u64,
    pub decisions: Option<[Decision; 2]>,
    pub replay_index: Option<usize>,
    pub finished: bool,
    pub error: Option<String>,
    pub fatal: bool,
    pub dropped_frames: f64,
}
pub struct Output {
    pub view: View,
    pub pixels: Vec<u16>,
    pub audio: Vec<f32>,
    pub battery: Option<Vec<u8>>,
}
pub struct Runtime {
    pub session: Session,
    clock: FrameClock,
    input: Input,
    replay: Option<(Replay, usize)>,
    finished: bool,
    halted: bool,
    error: Option<String>,
}
impl Runtime {
    pub fn new(rom: Vec<u8>, snapshot: &[u8], setup: Setup) -> Result<Self, String> {
        let mut session = Session::new(&setup.name, rom)?;
        session.restore(snapshot)?;
        session.alternate = setup.alternate;
        for slot in 0..2 {
            session.ownership[slot] = setup.ownership[slot];
            if let Some(model) = setup.models[slot].clone() {
                session.load_model(slot, model, setup.modes[slot])?;
            } else {
                session.set_mode(slot, setup.modes[slot])?;
            }
        }
        if let Some((replay, index)) = &setup.replay
            && *index > replay.inputs.len()
        {
            return Err("invalid replay position".into());
        }
        if let Some((replay, _)) = &setup.replay {
            replay.validate_metadata(&session.machine)?;
        }
        session.resume_playback(setup.replay.is_some())?;
        Ok(Self {
            session,
            clock: FrameClock::default(),
            input: Input {
                aim: [-1, -1],
                ..Default::default()
            },
            replay: setup.replay,
            finished: false,
            halted: false,
            error: None,
        })
    }
    pub fn input(&mut self, input: Input) {
        self.input.human = input.human;
        for slot in 0..2 {
            self.input.pulse[slot] |= input.pulse[slot] & 12;
        }
        self.input.aim = input.aim;
        self.input.fire_frames = self.input.fire_frames.max(input.fire_frames.min(20));
    }
    pub fn update(&mut self, now: f64, queued: Option<usize>) -> Option<Output> {
        if self.finished || self.halted {
            return None;
        }
        let frames =
            self.clock
                .frames_with_audio(now, self.session.machine.frame_rate(), true, queued);
        if frames == 0 {
            return None;
        }
        let mut audio = Vec::with_capacity(frames * 1000);
        for _ in 0..frames {
            self.session.pulse = self.input.pulse;
            self.input.pulse = [0; 2];
            self.session.machine.set_zapper(
                self.input.aim[0],
                self.input.aim[1],
                self.input.fire_frames > 0,
            );
            self.input.fire_frames = self.input.fire_frames.saturating_sub(1);
            let result = if let Some((replay, index)) = &mut self.replay {
                if let Some(&buttons) = replay.inputs.get(*index) {
                    self.session.replay_frame(buttons).and_then(|audio| {
                        *index += 1;
                        replay.verify_checkpoint(*index, &self.session.machine)?;
                        Ok(audio)
                    })
                } else {
                    self.finished = true;
                    break;
                }
            } else {
                self.session.frame(self.input.human)
            };
            match result {
                Ok(samples) => audio.extend(samples),
                Err(e) => {
                    self.error = Some(e);
                    self.finished = true;
                    break;
                }
            }
        }
        if self
            .replay
            .as_ref()
            .is_some_and(|(r, i)| *i == r.inputs.len())
        {
            use sha2::{Digest, Sha256};
            let (r, _) = self.replay.as_ref().unwrap();
            if format!("{:x}", Sha256::digest(self.session.machine.save_state()))
                != r.result.final_state_sha256
            {
                self.error = Some("Replay final state does not match the recording".into());
            }
            self.finished = true;
        }
        Some(Output {
            view: self.view(),
            pixels: self.session.machine.ppu().framebuffer().to_vec(),
            audio,
            battery: self.session.machine.battery_ram().map(|b| b.to_vec()),
        })
    }
    pub fn view(&self) -> View {
        View {
            frame: self.session.machine.ppu().completed_frames(),
            decisions: self.session.decisions.clone(),
            replay_index: self.replay.as_ref().map(|(_, i)| *i),
            finished: self.finished,
            error: self.error.clone(),
            fatal: self.session.machine.fault().is_some(),
            dropped_frames: self.clock.dropped,
        }
    }
    pub fn pause(&mut self) {
        self.session.pause();
        self.halted = true;
        self.clock.reset();
    }
    pub fn snapshot(&self) -> Vec<u8> {
        self.session.machine.save_state()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_fault_transfer_cannot_turn_a_fault_into_a_skipped_instruction() {
        let mut rom = nes_core::diagnostic::controller_rom();
        rom[16] = 0x02;
        let mut host = Session::new("Fault", rom.clone()).unwrap();
        let valid = host.machine.save_state();
        let mut runtime = Runtime::new(rom, &valid, Setup::from_session(&host, None)).unwrap();
        let output = runtime.update(0., None).unwrap();
        assert!(output.view.fatal);
        assert!(output.view.finished);
        assert!(host.restore(&runtime.snapshot()).is_err());
        host.adopt_transport(&runtime.snapshot(), output.view.error.as_deref())
            .unwrap();
        let faulted = host.machine.save_state();
        assert!(host.resume().is_err());
        assert!(host.frame([0; 2]).is_err());
        assert_eq!(faulted, host.machine.save_state());
        host.restore(&valid).unwrap();
        assert!(host.resume().is_ok());
    }
    #[test]
    fn realtime_and_direct_sessions_match_exactly_and_release_pulses() {
        let rom = nes_core::diagnostic::controller_rom();
        let mut direct = Session::new("Controllers", rom.clone()).unwrap();
        let mut runtime = Runtime::new(
            rom,
            &direct.machine.save_state(),
            Setup::from_session(&direct, None),
        )
        .unwrap();
        runtime.input(Input {
            human: [130, 1],
            pulse: [8, 0],
            aim: [-1, -1],
            fire_frames: 0,
        });
        direct.pulse = [8, 0];
        for frame in 0..60 {
            let out = runtime.update(frame as f64 / direct.machine.frame_rate(), None);
            if let Some(out) = out {
                while direct.machine.ppu().completed_frames() < out.view.frame {
                    direct.frame([130, 1]).unwrap();
                }
                assert_eq!(direct.machine.save_state(), runtime.snapshot());
            }
        }
        runtime.pause();
        assert!(runtime.update(100., None).is_none());
        assert_eq!(runtime.session.pulse, [0; 2]);
    }
}
