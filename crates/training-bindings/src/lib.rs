#[cfg(feature = "python")]
mod typed;
#[cfg(feature = "python")]
mod python {
    use crate::typed::{EpisodeBatch, StepData};
    use contra_agent::{Bundle, episode::Episode};
    use pyo3::{exceptions::PyValueError, prelude::*};
    fn error(e: impl std::fmt::Display) -> PyErr {
        PyValueError::new_err(e.to_string())
    }
    #[pyclass]
    struct Contra {
        inner: Episode,
    }
    #[pyclass]
    struct Checkpoint {
        inner: contra_agent::episode::Checkpoint,
    }
    #[pymethods]
    impl Contra {
        #[new]
        #[pyo3(signature=(rom,horizon=7200,single_player=false))]
        fn new(rom: &[u8], horizon: u32, single_player: bool) -> PyResult<Self> {
            Ok(Self {
                inner: Episode::new_with_players(rom, horizon, if single_player { 1 } else { 2 })
                    .map_err(error)?,
            })
        }
        fn reset(&mut self, seed: u64) -> PyResult<String> {
            serde_json::to_string(&self.inner.reset(seed).map_err(error)?).map_err(error)
        }
        fn reset_timing(&mut self, warmup_frames: u32) -> PyResult<String> {
            serde_json::to_string(&self.inner.reset_timing(warmup_frames).map_err(error)?)
                .map_err(error)
        }
        fn step(&mut self, players: [u8; 2]) -> PyResult<String> {
            serde_json::to_string(&self.inner.step(players).map_err(error)?).map_err(error)
        }
        fn reset_data(&mut self, py: Python<'_>, seed: u64) -> PyResult<StepData> {
            py.detach(|| self.inner.reset(seed).map(Into::into).map_err(error))
        }
        fn reset_timing_data(&mut self, py: Python<'_>, warmup_frames: u32) -> PyResult<StepData> {
            py.detach(|| {
                self.inner
                    .reset_timing(warmup_frames)
                    .map(Into::into)
                    .map_err(error)
            })
        }
        fn step_data(&mut self, py: Python<'_>, players: [u8; 2]) -> PyResult<StepData> {
            py.detach(|| self.inner.step(players).map(Into::into).map_err(error))
        }
        fn state(&self) -> PyResult<String> {
            serde_json::to_string(self.inner.current()).map_err(error)
        }
        fn checkpoint(&self) -> Checkpoint {
            Checkpoint {
                inner: self.inner.checkpoint(),
            }
        }
        fn restore(&mut self, checkpoint: &Checkpoint) -> PyResult<String> {
            serde_json::to_string(&self.inner.restore(&checkpoint.inner).map_err(error)?)
                .map_err(error)
        }
        fn snapshot(&self) -> Vec<u8> {
            self.inner.machine.save_state()
        }
        fn pixels(&self) -> Vec<u16> {
            self.inner.machine.ppu().framebuffer().to_vec()
        }
        fn policy_features(
            &self,
            schema: &str,
            slot: usize,
            human: u8,
            ownership: u8,
        ) -> PyResult<Vec<f32>> {
            contra_agent::registry::policy_features(
                &self.inner.machine,
                slot,
                schema,
                human,
                ownership,
            )
            .map_err(error)
        }
        fn ram(&self) -> Vec<u8> {
            (0..2048)
                .map(|a| self.inner.machine.peek(a).unwrap())
                .collect()
        }
    }
    /// Unrestricted local research session; never used to validate submitted weights.
    #[pyclass]
    struct ResearchMachine {
        inner: nes_core::machine::Machine,
    }
    #[pymethods]
    impl ResearchMachine {
        #[new]
        fn new(rom: &[u8]) -> PyResult<Self> {
            Ok(Self {
                inner: nes_core::machine::Machine::new(
                    nes_core::cartridge::Cartridge::from_ines(rom).map_err(error)?,
                ),
            })
        }
        fn step(&mut self, players: [u8; 2], frames: u32) -> PyResult<()> {
            if frames > 108_000 {
                return Err(error("frame budget exceeds limit"));
            }
            self.inner.set_controllers(players);
            for _ in 0..frames {
                self.inner.run_frame().map_err(error)?;
                self.inner.take_audio();
            }
            Ok(())
        }
        fn policy_features(
            &self,
            schema: &str,
            slot: usize,
            human: u8,
            ownership: u8,
        ) -> PyResult<Vec<f32>> {
            contra_agent::registry::policy_features(&self.inner, slot, schema, human, ownership)
                .map_err(error)
        }
        fn replay_provenance(&self) -> PyResult<String> {
            serde_json::to_string(&session_engine::replay::Provenance::current(&self.inner))
                .map_err(error)
        }
        fn observe(&self, task: &str) -> PyResult<String> {
            if task == "contra-jungle" {
                contra_agent::validate_rom(&self.inner).map_err(error)?;
                let state = contra_agent::observe(&self.inner);
                return serde_json::to_string(&serde_json::json!({
                    "gameplay": state.gameplay,
                    "supported": state.supported,
                    "success": state.level == 1,
                    "observations": [state.observation(0).map_err(error)?, state.observation(1).map_err(error)?],
                    "state": state,
                })).map_err(error);
            }
            let task = contra_agent::tasks::Task::parse(task).map_err(error)?;
            serde_json::to_string(&task.read(&self.inner).map_err(error)?).map_err(error)
        }
        fn ram(&self) -> Vec<u8> {
            (0..2048).map(|a| self.inner.peek(a).unwrap()).collect()
        }
        /// Bounded write observations for local field research; never patches RAM.
        fn watch_ram(
            &mut self,
            players: [u8; 2],
            frames: u32,
            addresses: Vec<u16>,
        ) -> PyResult<String> {
            if frames == 0
                || frames > 240
                || addresses.is_empty()
                || addresses.len() > 128
                || addresses.iter().any(|a| *a >= 2048)
            {
                return Err(error("watch needs 1..240 frames and 1..128 RAM addresses"));
            }
            let mut addresses = addresses;
            addresses.sort_unstable();
            addresses.dedup();
            let mut values: Vec<_> = addresses
                .iter()
                .map(|a| self.inner.peek(*a).unwrap())
                .collect();
            let mut events = Vec::new();
            let mut changes = 0usize;
            self.inner.set_controllers(players);
            for frame in 0..frames {
                let completed = self.inner.ppu().completed_frames();
                while self.inner.ppu().completed_frames() == completed {
                    let pc = self.inner.cpu.pc;
                    let x = self.inner.cpu.x;
                    let y = self.inner.cpu.y;
                    self.inner.step().map_err(error)?;
                    for (i, address) in addresses.iter().enumerate() {
                        let value = self.inner.peek(*address).unwrap();
                        if value != values[i] {
                            changes += 1;
                            if events.len() < 16_384 {
                                events.push(serde_json::json!({
                                    "frame": frame, "pc": pc, "x": x, "y": y,
                                    "address": address, "before": values[i], "after": value,
                                }));
                            }
                            values[i] = value;
                        }
                    }
                }
                self.inner.take_audio();
            }
            serde_json::to_string(&serde_json::json!({
                "frames": frames, "changes": changes,
                "truncated": changes > events.len(), "events": events,
            }))
            .map_err(error)
        }
        fn pixels(&self) -> Vec<u16> {
            self.inner.ppu().framebuffer().to_vec()
        }
        fn snapshot(&self) -> Vec<u8> {
            self.inner.save_state()
        }
        fn restore(&mut self, bytes: &[u8]) -> PyResult<()> {
            self.inner.load_state(bytes).map_err(error)
        }
    }
    #[pyclass]
    struct TaskEpisode {
        inner: contra_agent::tasks::Episode,
    }
    #[pymethods]
    impl TaskEpisode {
        fn reset_data(&mut self, py: Python<'_>, warmup: u32) -> PyResult<StepData> {
            py.detach(|| self.inner.reset(warmup).map(Into::into).map_err(error))
        }
        fn step_data(&mut self, py: Python<'_>, players: [u8; 2]) -> PyResult<StepData> {
            py.detach(|| self.inner.step(players).map(Into::into).map_err(error))
        }
        #[new]
        #[pyo3(signature=(rom,task,horizon=12000,start=None,single_player=false))]
        fn new(
            rom: &[u8],
            task: &str,
            horizon: u32,
            start: Option<&[u8]>,
            single_player: bool,
        ) -> PyResult<Self> {
            Ok(Self {
                inner: contra_agent::tasks::Episode::new_with_players(
                    rom,
                    contra_agent::tasks::Task::parse(task).map_err(error)?,
                    horizon,
                    start,
                    single_player,
                )
                .map_err(error)?,
            })
        }
        fn reset(&mut self, warmup: u32) -> PyResult<String> {
            serde_json::to_string(&self.inner.reset(warmup).map_err(error)?).map_err(error)
        }
        fn step(&mut self, players: [u8; 2]) -> PyResult<String> {
            serde_json::to_string(&self.inner.step(players).map_err(error)?).map_err(error)
        }
        fn snapshot(&self) -> Vec<u8> {
            self.inner.machine.save_state()
        }
        fn pixels(&self) -> Vec<u16> {
            self.inner.machine.ppu().framebuffer().to_vec()
        }
    }
    #[pyclass]
    struct Actor {
        inner: Bundle,
    }
    #[pymethods]
    impl Actor {
        #[new]
        fn new(manifest: &str, weights: &[u8]) -> PyResult<Self> {
            Ok(Self {
                inner: Bundle::load(manifest, weights).map_err(error)?,
            })
        }
        fn buttons_with_context(
            &self,
            observation: Vec<f32>,
            human: u8,
            ownership: u8,
        ) -> PyResult<u8> {
            self.inner
                .buttons_with_context(&observation, human, ownership)
                .map_err(error)
        }
        fn logits(&self, observation: Vec<f32>) -> PyResult<Vec<f32>> {
            Ok(self.inner.logits(&observation).map_err(error)?.to_vec())
        }
        fn buttons(&self, observation: Vec<f32>) -> PyResult<u8> {
            self.inner.buttons(&observation).map_err(error)
        }
    }
    #[pyfunction]
    fn observation_schema(game: &str) -> PyResult<String> {
        let task = contra_agent::registry::task(game).ok_or_else(|| error("unknown game task"))?;
        let schema = contra_agent::observations::schema(task.observation)
            .ok_or_else(|| error("missing observation schema"))?;
        Ok(schema.json.to_owned())
    }
    #[pyfunction]
    fn assistance_features(observation: Vec<f32>, human: u8, ownership: u8) -> PyResult<Vec<f32>> {
        contra_agent::control::assistance_features(&observation, human, ownership).map_err(error)
    }
    #[pyfunction]
    fn arbitrate(human: u8, proposal: u8, ownership: u8) -> PyResult<u8> {
        if !contra_agent::control::valid_ownership(ownership) {
            return Err(error("invalid ownership"));
        }
        Ok(contra_agent::control::arbitrate(human, proposal, ownership))
    }
    #[pymodule]
    fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("ICE_DEATH_METRIC", session_engine::tasks::ICE_DEATH_METRIC)?;
        m.add_function(wrap_pyfunction!(assistance_features, m)?)?;
        m.add_function(wrap_pyfunction!(arbitrate, m)?)?;
        m.add_function(wrap_pyfunction!(observation_schema, m)?)?;
        m.add_class::<StepData>()?;
        m.add_class::<EpisodeBatch>()?;
        m.add_class::<Contra>()?;
        m.add_class::<Actor>()?;
        m.add_class::<TaskEpisode>()?;
        m.add_class::<ResearchMachine>()?;
        Ok(())
    }
}
