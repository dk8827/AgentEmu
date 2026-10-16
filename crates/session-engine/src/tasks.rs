use game_integrations::{
    Result,
    tasks::{State, Task},
};
use nes_core::{cartridge::Cartridge, machine::Machine};
use serde::Serialize;
#[derive(Serialize)]
pub struct Step {
    pub observations: [Vec<f32>; 2],
    pub reward: f32,
    pub terminated: bool,
    pub truncated: bool,
    pub info: Info,
}
#[derive(Serialize)]
pub struct Info {
    pub frames: u32,
    pub success: bool,
    pub max_progress: u32,
    pub deaths: [u32; 2],
    pub state: serde_json::Value,
}
pub struct Episode {
    pub machine: Machine,
    task: Task,
    initial: nes_core::machine::Checkpoint,
    horizon: u32,
    frames: u32,
    progress: u32,
    deaths: [u32; 2],
    dead: [bool; 2],
    finished: bool,
    single_player: bool,
}
fn run(m: &mut Machine, masks: [u8; 2], frames: u32) -> Result<()> {
    m.set_controllers(masks);
    for _ in 0..frames {
        m.run_frame()?;
        m.take_audio();
    }
    Ok(())
}
impl Episode {
    pub fn new(rom: &[u8], task: Task, horizon: u32, start: Option<&[u8]>) -> Result<Self> {
        Self::new_with_players(rom, task, horizon, start, false)
    }
    pub fn new_with_players(
        rom: &[u8],
        task: Task,
        horizon: u32,
        start: Option<&[u8]>,
        single_player: bool,
    ) -> Result<Self> {
        if single_player && start.is_some() {
            return Err("solo tasks use their verified menu reset".into());
        }
        if !(1..=108000).contains(&horizon) {
            return Err("horizon must be 1..108000".into());
        }
        let mut machine = Machine::new(Cartridge::from_ines(rom)?);
        if single_player {
            task.initialize_solo(&mut machine)?;
        } else {
            task.initialize(&mut machine, start)?;
        }
        let initial = machine.checkpoint();
        let mut episode = Self {
            machine,
            task,
            initial,
            horizon,
            frames: 0,
            progress: 0,
            deaths: [0; 2],
            dead: [false; 2],
            finished: false,
            single_player,
        };
        episode.reset(0)?;
        Ok(episode)
    }
    pub fn reset(&mut self, warmup: u32) -> Result<Step> {
        if warmup > 240 {
            return Err("warmup must be 0..240".into());
        }
        self.machine.restore_checkpoint(&self.initial)?;
        run(&mut self.machine, [0, 0], warmup)?;
        let s = self.task.read(&self.machine)?;
        if !s.supported || s.active == [false, false] {
            return Err("warmup left the supported task".into());
        }
        self.frames = 0;
        self.progress = s.progress;
        self.deaths = [0; 2];
        self.dead = s.dead;
        self.finished = false;
        Ok(self.output(s, 0., false, false))
    }
    pub fn step(&mut self, masks: [u8; 2]) -> Result<Step> {
        if self.finished {
            return Err("episode finished; reset required".into());
        }
        if self.single_player && masks[1] != 0 {
            return Err("solo task requires neutral P2 input".into());
        }
        if masks
            .iter()
            .any(|m| m & 12 != 0 || m & 48 == 48 || m & 192 == 192)
        {
            return Err("invalid training buttons".into());
        }
        let before = self.task.read(&self.machine)?;
        // DD2 can respawn directly after falling off a platform, without entering
        // its normal death animation. Count cartridge life losses for this task.
        let life_base = match self.task {
            Task::JackalStage1 => 0x31,
            Task::IceMountain1 => 0x20,
            _ => 0x432,
        };
        let life_counts =
            |m: &Machine| [m.peek(life_base).unwrap(), m.peek(life_base + 1).unwrap()];
        let lives_before = matches!(
            self.task,
            Task::DoubleDragonMission1 | Task::JackalStage1 | Task::IceMountain1
        )
        .then(|| life_counts(&self.machine));
        crate::runner::advance(
            &mut self.machine,
            crate::runner::Input::Policy {
                buttons: masks,
                gameplay: before.gameplay,
            },
        )?;
        self.frames += 1;
        let s = self.task.read(&self.machine)?;
        let mut reward = -0.001;
        let lives_after = life_counts(&self.machine);
        for p in 0..if self.single_player { 1 } else { 2 } {
            let losses = if self.task == Task::IceMountain1 {
                // Scroll deaths may skip the death animation. Count the cartridge
                // counter's wrap to $FF once, then ignore inactive-slot underflow.
                if before.gameplay {
                    ice_life_losses(lives_before.unwrap()[p], lives_after[p])
                } else {
                    0
                }
            } else if let Some(lives) = lives_before {
                lives[p].saturating_sub(lives_after[p]) as u32
            } else {
                (s.dead[p] && !self.dead[p]) as u32
            };
            self.deaths[p] += losses;
            reward -= losses as f32;
        }
        self.dead = s.dead;
        if s.supported {
            reward += s.progress.saturating_sub(self.progress) as f32;
            self.progress = self.progress.max(s.progress)
        }
        if s.success {
            reward += 10.
        }
        let terminated = s.success || !s.supported || s.active == [false, false];
        let truncated = !terminated && self.frames >= self.horizon;
        self.finished = terminated || truncated;
        Ok(self.output(s, reward, terminated, truncated))
    }
    fn output(&self, s: State, reward: f32, terminated: bool, truncated: bool) -> Step {
        Step {
            observations: s.observations,
            reward,
            terminated,
            truncated,
            info: Info {
                frames: self.frames,
                success: s.success,
                max_progress: self.progress,
                deaths: self.deaths,
                state: s.state,
            },
        }
    }
}

/// Metric used by the Ice Climber task reward and evaluation reports.
pub const ICE_DEATH_METRIC: &str = "cartridge-life-losses-v2";

fn ice_life_losses(before: u8, after: u8) -> u32 {
    if before >= 128 {
        0
    } else if after >= 128 {
        u32::from(before) + 1
    } else {
        u32::from(before.saturating_sub(after))
    }
}

#[cfg(test)]
mod tests {
    use super::ice_life_losses;

    #[test]
    fn ice_counts_scroll_losses_and_final_life_without_counting_inactive_underflow() {
        let trace = [3, 2, 1, 0, 255, 254, 253, 252];
        let losses: u32 = trace.windows(2).map(|p| ice_life_losses(p[0], p[1])).sum();
        assert_eq!(losses, 4);
        assert_eq!(ice_life_losses(1, 2), 0); // A bonus life is not a death.
        assert_eq!(ice_life_losses(2, 2), 0);
        assert_eq!(ice_life_losses(255, 3), 0); // A fresh game, not a loss.
    }
}
