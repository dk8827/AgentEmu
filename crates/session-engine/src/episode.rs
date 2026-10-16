//! Single-frame simultaneous stepping, deterministic menu reset and bounded progress reward.
use game_integrations::{Frame, Result, observe, validate_rom};
use nes_core::{cartridge::Cartridge, machine::Machine};
use serde::Serialize;

#[derive(Serialize)]
pub struct Step {
    pub observations: [Vec<f32>; 2],
    pub rewards: [f32; 2],
    pub terminated: bool,
    pub truncated: bool,
    pub info: Info,
}
#[derive(Clone, Serialize)]
pub struct Info {
    pub player_count: u8,
    pub frames: u32,
    pub max_progress: [u16; 2],
    pub deaths: [u32; 2],
    pub success: bool,
    pub state: Frame,
}
pub struct Episode {
    pub machine: Machine,
    initial: nes_core::machine::Checkpoint,
    info: Info,
    horizon: u32,
    finished: bool,
}
/// Local training/planning checkpoint. Includes reward accounting and the terminal flag,
/// so restoring cannot accidentally carry progress rewards across branches.
pub struct Checkpoint {
    machine: nes_core::machine::Checkpoint,
    info: Info,
    horizon: u32,
    finished: bool,
}
fn terminal(state: &Frame) -> bool {
    state.level == 1 || state.players.iter().all(|p| !p.active) || !state.supported
}
impl Episode {
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            machine: self.machine.checkpoint(),
            info: self.info.clone(),
            horizon: self.horizon,
            finished: self.finished,
        }
    }
    pub fn restore(&mut self, checkpoint: &Checkpoint) -> Result<Step> {
        if checkpoint.info.player_count != self.info.player_count {
            return Err("checkpoint player mode differs from episode".into());
        }
        self.machine.restore_checkpoint(&checkpoint.machine)?;
        self.info = checkpoint.info.clone();
        self.horizon = checkpoint.horizon;
        self.finished = checkpoint.finished;
        let terminated = terminal(&self.info.state);
        self.output([0.; 2], terminated, self.finished && !terminated)
    }
    pub fn new(rom: &[u8], horizon: u32) -> Result<Self> {
        Self::new_with_players(rom, horizon, 2)
    }
    pub fn new_with_players(rom: &[u8], horizon: u32, player_count: u8) -> Result<Self> {
        if !matches!(player_count, 1 | 2) {
            return Err("Contra supports one or two players".into());
        }
        if !(1..=108_000).contains(&horizon) {
            return Err("episode horizon must be 1..108000 frames".into());
        }
        let mut machine = Machine::new(Cartridge::from_ines(rom)?);
        validate_rom(&machine)?;
        // Select advances to two players; solo leaves the ordinary default selected.
        // Keep identical frame timing, without RAM edits, cheats or imported saves.
        for (n, mask) in [
            (600, 0),
            (1, if player_count == 2 { 4 } else { 0 }),
            (30, 0),
            (1, 8),
        ] {
            for _ in 0..n {
                machine.set_controllers([mask, 0]);
                machine.run_frame()?;
                machine.take_audio();
            }
        }
        for _ in 0..1200 {
            let f = observe(&machine);
            if f.gameplay
                && f.players[..usize::from(player_count)]
                    .iter()
                    .all(|p| p.active && p.grounded && !p.dead)
            {
                break;
            }
            machine.set_controllers([0, 0]);
            machine.run_frame()?;
            machine.take_audio();
        }
        let f = observe(&machine);
        if !f.gameplay
            || f.two_players != (player_count == 2)
            || !f.players[..usize::from(player_count)]
                .iter()
                .all(|p| p.active && p.grounded)
            || (player_count == 1 && f.players[1].active)
        {
            return Err("Contra reset did not reach requested player mode in Jungle".into());
        }
        let initial = machine.checkpoint();
        let info = Info {
            player_count,
            frames: 0,
            max_progress: [0; 2],
            deaths: [0; 2],
            success: false,
            state: f,
        };
        let mut e = Self {
            machine,
            initial,
            info,
            horizon,
            finished: false,
        };
        e.reset(0)?;
        Ok(e)
    }
    pub fn reset(&mut self, seed: u64) -> Result<Step> {
        self.reset_timing((seed % 32) as u32)
    }
    /// Explicit timing variation, without the legacy seed modulo or memory edits.
    pub fn reset_timing(&mut self, warmup_frames: u32) -> Result<Step> {
        if warmup_frames > 240 {
            return Err("warmup must be 0..240 frames".into());
        }
        self.machine.restore_checkpoint(&self.initial)?;
        for _ in 0..warmup_frames {
            self.machine.set_controllers([0, 0]);
            self.machine.run_frame()?;
            self.machine.take_audio();
        }
        let state = observe(&self.machine);
        self.info = Info {
            player_count: self.info.player_count,
            frames: 0,
            max_progress: std::array::from_fn(|p| state.camera_x + state.players[p].x as u16),
            deaths: [0; 2],
            success: false,
            state,
        };
        self.finished = false;
        self.output([0.; 2], false, false)
    }
    pub fn step(&mut self, masks: [u8; 2]) -> Result<Step> {
        if self.finished {
            return Err("episode finished; call reset".into());
        }
        if self.info.player_count == 1 && masks[1] != 0 {
            return Err("solo Contra requires a neutral second controller".into());
        }
        if masks
            .iter()
            .any(|m| m & 12 != 0 || m & 48 == 48 || m & 192 == 192)
        {
            return Err(
                "episode actions cannot contain Start/Select or opposing directions".into(),
            );
        }
        let gameplay = observe(&self.machine).gameplay;
        crate::runner::advance(
            &mut self.machine,
            crate::runner::Input::Policy {
                buttons: masks,
                gameplay,
            },
        )?;
        let state = observe(&self.machine);
        self.info.frames += 1;
        // The boss flag starts the exit animation; a clear requires reaching Base 1.
        let success = state.level == 1;
        let mut rewards = [-0.001; 2];
        for (p, reward) in rewards.iter_mut().enumerate() {
            if p >= usize::from(self.info.player_count) {
                *reward = 0.;
                continue;
            }
            let dead = state.players[p].dead && !self.info.state.players[p].dead;
            if dead {
                self.info.deaths[p] += 1;
                *reward -= 1.;
            }
            if state.level == 0 && state.players[p].active && !state.players[p].dead {
                let progress = state.camera_x + state.players[p].x as u16;
                // High-water mark: walking backward and forward cannot farm reward.
                *reward += progress.saturating_sub(self.info.max_progress[p]) as f32 / 256.;
                self.info.max_progress[p] = self.info.max_progress[p].max(progress);
            }
            if success {
                *reward += 10.;
            }
        }
        let terminated = terminal(&state);
        let truncated = !terminated && self.info.frames >= self.horizon;
        self.info.success = success;
        self.info.state = state;
        self.finished = terminated || truncated;
        self.output(rewards, terminated, truncated)
    }
    pub fn current(&self) -> &Info {
        &self.info
    }
    fn output(&self, rewards: [f32; 2], terminated: bool, truncated: bool) -> Result<Step> {
        Ok(Step {
            observations: [
                self.info.state.observation(0)?,
                self.info.state.observation(1)?,
            ],
            rewards,
            terminated,
            truncated,
            info: self.info.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boss_animation_and_one_survivor_do_not_end_episode() {
        let machine =
            Machine::new(Cartridge::from_ines(&nes_core::diagnostic::playground_rom()).unwrap());
        let mut state = observe(&machine);
        state.supported = true;
        state.level = 0;
        state.boss_defeated = true;
        state.players[0].active = true;
        state.players[1].active = false;
        assert!(!terminal(&state));
        state.players[0].active = false;
        assert!(terminal(&state));
        state.players[0].active = true;
        state.level = 1;
        assert!(terminal(&state));
    }
}
