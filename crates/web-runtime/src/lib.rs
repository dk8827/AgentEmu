use nes_video::display;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Policy {
    inner: agent_runtime::Policy,
}

#[wasm_bindgen]
impl Policy {
    #[wasm_bindgen(constructor)]
    pub fn new(weights: &[u8]) -> Result<Policy, JsError> {
        Ok(Self {
            inner: agent_runtime::Policy::load(weights)?,
        })
    }
    pub fn forward(&self, observation: &[f32]) -> Result<Vec<f32>, JsError> {
        Ok(self.inner.forward(observation)?.to_vec())
    }
    pub fn buttons(&self, observation: &[f32]) -> Result<u8, JsError> {
        Ok(agent_runtime::decode(&self.inner.forward(observation)?)?)
    }
}

#[cfg(target_arch = "wasm32")]
mod video;

/// Original NES core, with deterministic video, audio and snapshots.
#[wasm_bindgen]
pub struct NesMachine {
    inner: nes_core::machine::Machine,
    rom: Vec<u8>,
    agents: contra_agent::Agents,
    human: [u8; 2],
    audio: Vec<f32>,
    decision: Option<[contra_agent::control::Decision; 2]>,
}

#[wasm_bindgen]
impl NesMachine {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Result<NesMachine, JsError> {
        Ok(Self {
            inner: nes_core::machine::Machine::new(nes_core::cartridge::Cartridge::from_ines(rom)?),
            rom: rom.to_vec(),
            agents: contra_agent::Agents::default(),
            human: [0; 2],
            audio: Vec::new(),
            decision: None,
        })
    }
    pub fn diagnostic() -> NesMachine {
        Self::new(&nes_core::diagnostic::controller_rom()).expect("valid built-in diagnostic")
    }
    pub fn ppu_diagnostic() -> NesMachine {
        Self::new(&nes_core::diagnostic::ppu_rom()).expect("valid built-in PPU diagnostic")
    }
    pub fn background_diagnostic() -> NesMachine {
        Self::new(&nes_core::diagnostic::background_rom())
            .expect("valid built-in background diagnostic")
    }
    pub fn playground() -> NesMachine {
        Self::new(&nes_core::diagnostic::playground_rom()).expect("valid playground")
    }
    pub fn has_zapper(&self) -> bool {
        self.inner.has_zapper()
    }
    pub fn zapper(&mut self, x: i16, y: i16, trigger: bool) {
        self.inner.set_zapper(x, y, trigger);
    }
    pub fn frame_rate(&self) -> f64 {
        self.inner.frame_rate()
    }
    pub fn battery_ram(&self) -> Vec<u8> {
        self.inner.battery_ram().unwrap_or_default().to_vec()
    }
    pub fn restore_battery_ram(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        Ok(self.inner.restore_battery_ram(bytes)?)
    }
    pub fn cartridge_hash(&self) -> String {
        self.inner.cartridge_hash()
    }
    pub fn completed_frames(&self) -> u64 {
        self.inner.ppu().completed_frames()
    }
    pub fn set_mmc3_alternate(&mut self, enabled: bool) -> Result<(), JsError> {
        Ok(self.inner.set_mmc3_alternate(enabled)?)
    }
    pub fn audio_diagnostic() -> NesMachine {
        Self::new(&nes_core::diagnostic::audio_rom()).expect("valid built-in audio diagnostic")
    }
    pub fn audio_samples(&mut self) -> Vec<f32> {
        self.audio.extend(self.inner.take_audio());
        std::mem::take(&mut self.audio)
    }
    pub fn frame(&mut self) -> Result<u64, JsError> {
        let frame = session_engine::runner::advance(
            &mut self.inner,
            session_engine::runner::Input::Human {
                agents: &self.agents,
                buttons: self.human,
            },
        )
        .map_err(|e| JsError::new(&e))?;
        self.decision = frame.decisions;
        self.audio.extend(frame.audio);
        self.audio.truncate(96_000);
        Ok(frame.cycles)
    }
    pub fn agent_phases(&self) -> Result<String, JsError> {
        Ok(serde_json::to_string(
            &self
                .agents
                .phases(&self.inner)
                .map_err(|e| JsError::new(&e.to_string()))?,
        )?)
    }
    pub fn control_trace(&self) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.decision)?)
    }
    pub fn set_agent_ownership(&mut self, slot: usize, mask: u8) -> Result<(), JsError> {
        self.agents
            .set_ownership(slot, mask)
            .map_err(|e| JsError::new(&e.to_string()))?;
        self.human = [0; 2];
        self.decision = None;
        Ok(())
    }
    pub fn faulted(&self) -> bool {
        self.inner.fault().is_some()
    }
    pub fn framebuffer_codes(&self) -> Vec<u16> {
        self.inner.ppu().framebuffer().to_vec()
    }
    pub fn framebuffer_rgba(&self) -> Vec<u8> {
        display::rgba(self.inner.ppu().framebuffer())
    }
    pub fn controllers(&mut self, player1: u8, player2: u8) {
        self.human = [player1, player2];
        self.inner.set_controllers(self.human);
    }
    pub fn run(&mut self, instructions: u32) -> Result<(), JsError> {
        if instructions > 100_000 {
            return Err(JsError::new("at most 100,000 instructions per call"));
        }
        self.inner.run_instructions(instructions as usize)?;
        Ok(())
    }
    pub fn report(&self) -> String {
        self.inner.diagnostic_report()
    }
    pub fn snapshot(&self) -> Vec<u8> {
        self.inner.save_state()
    }
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        self.inner.load_state(bytes)?;
        self.audio.clear();
        self.decision = None;
        Ok(())
    }
}

#[wasm_bindgen]
impl NesMachine {
    pub fn load_agent(
        &mut self,
        slot: usize,
        manifest: &str,
        weights: &[u8],
    ) -> Result<(), JsError> {
        self.agents
            .load(&self.inner, slot, manifest, weights)
            .map_err(|e| JsError::new(&e.to_string()))
    }
    pub fn clear_agent(&mut self, slot: usize) -> Result<(), JsError> {
        self.agents
            .clear(slot)
            .map_err(|e| JsError::new(&e.to_string()))
    }
    pub fn contra_state(&self) -> Result<String, JsError> {
        contra_agent::validate_rom(&self.inner).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(serde_json::to_string(&contra_agent::observe(&self.inner))?)
    }
    pub fn contra_observation(&self, slot: usize) -> Result<Vec<f32>, JsError> {
        contra_agent::validate_rom(&self.inner).map_err(|e| JsError::new(&e.to_string()))?;
        contra_agent::observe(&self.inner)
            .observation(slot)
            .map_err(|e| JsError::new(&e.to_string()))
    }
    pub fn policy_features(
        &self,
        schema: &str,
        slot: usize,
        human: u8,
        ownership: u8,
    ) -> Result<Vec<f32>, JsError> {
        contra_agent::registry::policy_features(&self.inner, slot, schema, human, ownership)
            .map_err(|e| JsError::new(&e.to_string()))
    }
    pub fn task_state(&self, task: &str) -> Result<String, JsError> {
        let task =
            contra_agent::tasks::Task::parse(task).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(serde_json::to_string(
            &task
                .read(&self.inner)
                .map_err(|e| JsError::new(&e.to_string()))?,
        )?)
    }
    pub fn start_task(&mut self, task: &str) -> Result<(), JsError> {
        self.start_task_with_warmup(task, 0)
    }
    /// Reproduce a documented timing start with bounded, ordinary neutral input.
    /// Prepare before replacement so invalid timing leaves the current game intact.
    pub fn start_task_with_warmup(&mut self, task: &str, warmup: u32) -> Result<(), JsError> {
        if warmup > 240 {
            return Err(JsError::new("start timing must be 0..240 frames"));
        }
        let task =
            contra_agent::tasks::Task::parse(task).map_err(|e| JsError::new(&e.to_string()))?;
        let mut episode = contra_agent::tasks::Episode::new(&self.rom, task, 12000, None)
            .map_err(|e| JsError::new(&e.to_string()))?;
        episode
            .reset(warmup)
            .map_err(|e| JsError::new(&e.to_string()))?;
        self.inner = episode.machine;
        self.human = [0; 2];
        self.audio.clear();
        Ok(())
    }
    pub fn start_snow(&mut self) -> Result<(), JsError> {
        self.inner = contra_agent::tasks::Episode::new(
            &self.rom,
            contra_agent::tasks::Task::SnowFloor1,
            12000,
            None,
        )
        .map_err(|e| JsError::new(&e.to_string()))?
        .machine;
        self.human = [0; 2];
        self.audio.clear();
        Ok(())
    }
    pub fn start_contra(&mut self) -> Result<(), JsError> {
        self.inner = contra_agent::episode::Episode::new(&self.rom, 7200)
            .map_err(|e| JsError::new(&e.to_string()))?
            .machine;
        self.human = [0; 2];
        self.audio.clear();
        Ok(())
    }
    pub fn start_contra_solo(&mut self) -> Result<(), JsError> {
        self.inner = contra_agent::episode::Episode::new_with_players(&self.rom, 7200, 1)
            .map_err(|e| JsError::new(&e.to_string()))?
            .machine;
        self.human = [0; 2];
        self.audio.clear();
        Ok(())
    }
}
#[wasm_bindgen]
pub struct ContraPolicy {
    inner: contra_agent::Bundle,
}
#[wasm_bindgen]
impl ContraPolicy {
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: &str, weights: &[u8]) -> Result<ContraPolicy, JsError> {
        Ok(Self {
            inner: contra_agent::Bundle::load(manifest, weights)
                .map_err(|e| JsError::new(&e.to_string()))?,
        })
    }
    pub fn forward(&self, observation: &[f32]) -> Result<Vec<f32>, JsError> {
        Ok(self
            .inner
            .logits(observation)
            .map_err(|e| JsError::new(&e.to_string()))?
            .to_vec())
    }
    pub fn buttons(&self, observation: &[f32]) -> Result<u8, JsError> {
        self.inner
            .buttons(observation)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

/// Thin browser-worker bridge to the same headless runtime used by native.
#[wasm_bindgen]
pub struct SessionRuntime {
    inner: session_engine::realtime::Runtime,
    output: Option<session_engine::realtime::Output>,
}
#[wasm_bindgen]
impl SessionRuntime {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: Vec<u8>, state: &[u8], setup: &str) -> Result<Self, JsError> {
        Ok(Self {
            inner: session_engine::realtime::Runtime::new(rom, state, serde_json::from_str(setup)?)
                .map_err(|e| JsError::new(&e))?,
            output: None,
        })
    }
    pub fn input(&mut self, input: &str) -> Result<(), JsError> {
        self.inner.input(serde_json::from_str(input)?);
        Ok(())
    }
    pub fn update(&mut self, now: f64, queued: i32) -> bool {
        self.output = self.inner.update(
            now,
            if queued < 0 {
                None
            } else {
                Some(queued as usize)
            },
        );
        self.output.is_some()
    }
    pub fn view(&self) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.inner.view())?)
    }
    pub fn pixels(&self) -> Vec<u16> {
        self.inner.session.machine.ppu().framebuffer().to_vec()
    }
    pub fn audio(&mut self) -> Vec<f32> {
        self.output
            .as_mut()
            .map_or_else(Vec::new, |o| std::mem::take(&mut o.audio))
    }
    pub fn battery(&self) -> Vec<u8> {
        self.inner
            .session
            .machine
            .battery_ram()
            .map_or_else(Vec::new, |b| b.to_vec())
    }
    pub fn pause(&mut self) {
        self.inner.pause();
    }
    pub fn snapshot(&self) -> Vec<u8> {
        self.inner.snapshot()
    }
}
