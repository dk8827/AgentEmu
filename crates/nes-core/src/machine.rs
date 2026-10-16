//! CPU, cartridge, controllers and NTSC PPU register/timing foundation.
use crate::{
    apu::Apu,
    cartridge::{Cartridge, CartridgeState},
    controller::Controller,
    cpu::{Bus, Cpu, UnsupportedOpcode},
    ppu::Ppu,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum MachineError {
    Cpu(UnsupportedOpcode),
    InvalidState(String),
}
impl fmt::Display for MachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu(e) => e.fmt(f),
            Self::InvalidState(e) => write!(f, "invalid snapshot: {e}"),
        }
    }
}
impl Error for MachineError {}

pub struct Machine {
    fault: Option<UnsupportedOpcode>,
    pub cpu: Cpu,
    bus: MachineBus,
}
struct MachineBus {
    zapper: Option<crate::zapper::Zapper>,
    ram: Vec<u8>,
    cartridge: Cartridge,
    controllers: [Controller; 2],
    open_bus: u8,
    fault: Option<MachineError>,
    stall_cycles: u64,
    ppu: Ppu,
    apu: Apu,
    irq_pending: bool,
    irq_history: [bool; 2],
    nmi_pending: bool,
    nmi_scheduled: bool,
    nmi_history: [bool; 2],
    nmi_previous: bool,
    dma_page: Option<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zapper: Option<crate::zapper::Zapper>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fault: Option<UnsupportedOpcode>,
    version: u32,
    rom: [u8; 32],
    cpu: Cpu,
    ram: Vec<u8>,
    cartridge: CartridgeState,
    controllers: [Controller; 2],
    open_bus: u8,
    ppu: Ppu,
    apu: Apu,
    irq_pending: bool,
    irq_history: [bool; 2],
    nmi_pending: bool,
    nmi_scheduled: bool,
    nmi_history: [bool; 2],
    nmi_previous: bool,
    dma_page: Option<u8>,
}

/// Fast, opaque in-memory checkpoint. Portable saves use the versioned file envelope.
#[derive(Clone)]
pub struct Checkpoint(Snapshot);

impl Machine {
    pub fn new(cartridge: Cartridge) -> Self {
        let pal = cartridge.is_pal();
        let mut machine = Self {
            fault: None,
            cpu: Cpu::default(),
            bus: MachineBus {
                zapper: cartridge.has_zapper().then(Default::default),
                ram: vec![0; 2048],
                cartridge,
                controllers: Default::default(),
                open_bus: 0,
                fault: None,
                ppu: Ppu::with_region(pal),
                apu: Apu::with_region(pal),
                irq_pending: false,
                irq_history: [false; 2],
                stall_cycles: 0,
                nmi_pending: false,
                nmi_scheduled: false,
                nmi_history: [false; 2],
                nmi_previous: false,
                dma_page: None,
            },
        };
        machine.cpu.reset(&mut machine.bus);
        machine
    }
    pub fn has_zapper(&self) -> bool {
        self.bus.zapper.is_some()
    }
    pub fn set_zapper(&mut self, x: i16, y: i16, trigger: bool) {
        if let Some(z) = &mut self.bus.zapper {
            z.aim = if (0..256).contains(&x) && (0..240).contains(&y) {
                Some([x as u16, y as u16])
            } else {
                None
            };
            z.trigger = trigger;
        }
    }
    pub fn frame_rate(&self) -> f64 {
        if self.bus.cartridge.is_pal() {
            26_601_712.0 / 5.0 / (341.0 * 312.0)
        } else {
            60.098813897
        }
    }
    pub fn battery_ram(&self) -> Option<&[u8]> {
        self.bus.cartridge.battery_ram()
    }
    pub fn restore_battery_ram(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), crate::cartridge::CartridgeError> {
        self.bus.cartridge.restore_battery_ram(bytes)
    }
    pub fn cartridge_identity(&self) -> &crate::cartridge::image::Identity {
        self.bus.cartridge.identity()
    }
    pub fn mapper_id(&self) -> u8 {
        self.bus.cartridge.mapper_id()
    }
    pub fn cartridge_hash(&self) -> String {
        self.bus
            .cartridge
            .digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    pub fn set_mmc3_alternate(
        &mut self,
        enabled: bool,
    ) -> Result<(), crate::cartridge::CartridgeError> {
        self.bus.cartridge.set_mmc3_alternate(enabled)
    }
    pub fn set_controllers(&mut self, players: [u8; 2]) {
        for (port, buttons) in self.bus.controllers.iter_mut().zip(players) {
            port.set_buttons(buttons);
        }
    }
    pub fn fault(&self) -> Option<UnsupportedOpcode> {
        self.fault
    }
    pub fn step(&mut self) -> Result<u64, MachineError> {
        if let Some(error) = self.fault {
            return Err(MachineError::Cpu(error));
        }
        let start = self.cpu.cycles;
        if std::mem::take(&mut self.bus.nmi_scheduled) {
            self.bus.nmi_pending = false;
            self.bus.nmi_history = [false; 2];
            self.bus.irq_pending = false;
            self.cpu.interrupt(&mut self.bus, true);
        } else if std::mem::take(&mut self.bus.irq_pending) {
            self.cpu.interrupt(&mut self.bus, false);
        } else {
            if let Err(error) = self.cpu.step(&mut self.bus) {
                self.fault = Some(error);
                return Err(MachineError::Cpu(error));
            }
        }
        if let Some(page) = self.bus.dma_page.take() {
            // Once OAM has halted the CPU, DMC setup can overlap OAM
            // reads AND writes. Only the sample get steals the shared bus.
            let mut halt = true;
            let mut offset = 0_u16;
            let mut oam_value = None;
            let mut dmc: Option<(u16, u8)> = None;
            while offset < 256 || dmc.is_some() {
                if dmc.is_none()
                    && let Some(address) = self.bus.apu.dmc_request()
                {
                    dmc = Some((address, 2)); // halt, then dummy, then aligned get
                }
                let get = self.cpu.cycles & 1 != 0;
                if halt {
                    self.bus.read_cycle(self.cpu.pc);
                    halt = false;
                } else if let Some((address, 0)) = dmc
                    && get
                {
                    let sample = self.bus.read_cycle(address);
                    self.bus.apu.complete_dmc(sample);
                    dmc = None;
                } else if offset < 256 && get && oam_value.is_none() {
                    oam_value = Some(self.bus.read_cycle((page as u16) << 8 | offset));
                } else if !get && let Some(value) = oam_value.take() {
                    self.bus.write(0x2004, value, self.cpu.cycles);
                    offset += 1;
                } else {
                    self.bus.read_cycle(self.cpu.pc);
                }
                if let Some((_, delay)) = &mut dmc {
                    *delay = delay.saturating_sub(1);
                }
                self.cpu.cycles = self.cpu.cycles.wrapping_add(1);
            }
        }
        if let Some(error) = self.bus.fault.take() {
            return Err(error);
        }
        Ok(self.cpu.cycles.wrapping_sub(start))
    }
    /// Advance to publication of the next complete video frame (at vblank).
    pub fn run_frame(&mut self) -> Result<u64, MachineError> {
        let completed = self.bus.ppu.completed_frames();
        let start = self.cpu.cycles;
        while self.bus.ppu.completed_frames() == completed {
            self.step()?;
        }
        Ok(self.cpu.cycles.wrapping_sub(start))
    }
    pub fn take_audio(&mut self) -> Vec<f32> {
        self.bus.apu.take_samples()
    }
    pub fn apu(&self) -> &Apu {
        &self.bus.apu
    }
    pub fn ppu(&self) -> &Ppu {
        &self.bus.ppu
    }
    pub fn peek_ppu(&self, address: u16) -> u8 {
        self.bus.ppu.peek_memory(&self.bus.cartridge, address)
    }

    /// Bounded stepping; an NMI entry consumes one step, as does an instruction.
    pub fn run_instructions(&mut self, count: usize) -> Result<(), MachineError> {
        for _ in 0..count {
            self.step()?;
        }
        Ok(())
    }
    /// Side-effect-free access for diagnostics/adapters. Unimplemented I/O is None.
    pub fn peek(&self, address: u16) -> Option<u8> {
        match address {
            0..=0x1fff => Some(self.bus.ram[address as usize & 0x7ff]),
            0x2000..=0x3fff => Some(self.bus.ppu.peek_register(&self.bus.cartridge, address)),
            0x4015 => Some(self.bus.apu.status() | (self.bus.open_bus & 0x20)),
            0x4017 if self.bus.zapper.is_some() => Some(
                (self.bus.open_bus & 0xe0)
                    | self
                        .bus
                        .zapper
                        .as_ref()
                        .unwrap()
                        .read(self.bus.ppu.total_dots()),
            ),
            0x4016..=0x4017 => {
                Some((self.bus.open_bus & 0xe0) | self.bus.controllers[address as usize & 1].peek())
            }
            0x4020..=0xffff => self.bus.cartridge.cpu_read(address),
            _ => None,
        }
    }
    pub fn checkpoint(&self) -> Checkpoint {
        let mut apu = self.bus.apu.clone();
        apu.take_samples(); // Host output is excluded from both checkpoint formats.
        Checkpoint(Snapshot {
            zapper: self.bus.zapper.clone(),
            fault: self.fault,
            version: 6,
            rom: self.bus.cartridge.digest(),
            cpu: self.cpu,
            ram: self.bus.ram.clone(),
            cartridge: self.bus.cartridge.state.clone(),
            controllers: self.bus.controllers.clone(),
            open_bus: self.bus.open_bus,
            ppu: self.bus.ppu.clone(),
            apu,
            irq_pending: self.bus.irq_pending,
            irq_history: self.bus.irq_history,
            nmi_pending: self.bus.nmi_pending,
            nmi_scheduled: self.bus.nmi_scheduled,
            nmi_history: self.bus.nmi_history,
            nmi_previous: self.bus.nmi_previous,
            dma_page: self.bus.dma_page,
        })
    }
    /// Legacy canonical representation retained for existing replays and hashes.
    pub fn save_state(&self) -> Vec<u8> {
        serde_json::to_vec(&self.checkpoint().0).expect("serializable machine state")
    }
    pub fn restore_checkpoint(&mut self, checkpoint: &Checkpoint) -> Result<(), MachineError> {
        self.restore_snapshot(checkpoint.0.clone(), false)
    }
    pub fn diagnostic_report(&self) -> String {
        let digest = Sha256::digest(self.save_state());
        let digest: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut frame_hasher = Sha256::new();
        for pixel in self.bus.ppu.framebuffer() {
            frame_hasher.update(pixel.to_le_bytes());
        }
        let frame_digest: String = frame_hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        serde_json::json!({"cpu":self.cpu,"ram":self.bus.ram[..34],"state_sha256":digest,
            "ppu":{"frame":self.bus.ppu.frame(),"scanline":self.bus.ppu.scanline(),
                "dot":self.bus.ppu.dot(),"total_dots":self.bus.ppu.total_dots(),
                "nmi_line":self.bus.ppu.nmi_line(),"completed_frames":self.bus.ppu.completed_frames(),
                "framebuffer_sha256":frame_digest}})
        .to_string()
    }
    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), MachineError> {
        let invalid = |message: &str| MachineError::InvalidState(message.into());
        if bytes.len() > 1024 * 1024 {
            return Err(invalid("too large"));
        }
        let state: Snapshot =
            serde_json::from_slice(bytes).map_err(|e| MachineError::InvalidState(e.to_string()))?;
        self.restore_snapshot(state, false)
    }
    /// Move a stopped worker's state to another host. A fault remains latched;
    /// this is not a recovery operation and never makes a faulted state runnable.
    pub fn adopt_transport_state(&mut self, bytes: &[u8]) -> Result<(), MachineError> {
        if bytes.len() > 1024 * 1024 {
            return Err(MachineError::InvalidState("too large".into()));
        }
        let state =
            serde_json::from_slice(bytes).map_err(|e| MachineError::InvalidState(e.to_string()))?;
        self.restore_snapshot(state, true)
    }
    fn restore_snapshot(&mut self, state: Snapshot, allow_fault: bool) -> Result<(), MachineError> {
        let invalid = |message: &str| MachineError::InvalidState(message.into());
        if state.zapper.is_some() != self.bus.zapper.is_some()
            || state
                .zapper
                .as_ref()
                .is_some_and(|z| !z.valid(state.ppu.total_dots()))
            || (state.fault.is_some() && !allow_fault)
            || state.version != 6
            || state.ppu.is_pal() != self.bus.cartridge.is_pal()
            || state.apu.is_pal() != self.bus.cartridge.is_pal()
            || !state.ppu.valid_state(state.cpu.cycles)
            || !state.apu.valid_state(state.cpu.cycles)
            || state.dma_page.is_some()
            || state.rom != self.bus.cartridge.digest()
            || state.ram.len() != 2048
            || !self
                .bus
                .cartridge
                .valid_state(&state.cartridge, state.cpu.cycles)
        {
            return Err(invalid("version, cartridge or memory layout mismatch"));
        }
        // Validate everything before mutating the live machine.
        self.fault = state.fault;
        self.cpu = state.cpu;
        self.bus.zapper = state.zapper;
        self.bus.ram = state.ram;
        self.bus.cartridge.state = state.cartridge;
        self.bus.controllers = state.controllers;
        self.bus.open_bus = state.open_bus;
        self.bus.ppu = state.ppu;
        self.bus.apu = state.apu;
        self.bus.irq_pending = state.irq_pending;
        self.bus.irq_history = state.irq_history;
        self.bus.stall_cycles = 0;
        self.bus.nmi_pending = state.nmi_pending;
        self.bus.nmi_scheduled = state.nmi_scheduled;
        self.bus.nmi_history = state.nmi_history;
        self.bus.nmi_previous = state.nmi_previous;
        self.bus.dma_page = state.dma_page;
        self.bus.fault = None;
        Ok(())
    }
}

impl MachineBus {
    fn clock(&mut self) {
        // The CPU edge detector samples once per CPU cycle, after one PPU
        // dots. The instruction polls the previous cycle's detector output.
        self.tick_ppu();
        self.sample_nmi();
        self.irq_history = [self.irq_history[1], self.apu.irq() || self.cartridge.irq()];
        self.tick_ppu();
        self.tick_ppu();
        self.nmi_history = [self.nmi_history[1], self.nmi_pending];
        if self.cartridge.is_pal() && (self.apu.cycles() + 1).is_multiple_of(5) {
            self.tick_ppu();
        }
        self.apu.tick();
        self.cartridge.clock_cpu();
    }
    fn tick_ppu(&mut self) {
        if let Some(fetch) = self.ppu.tick(&self.cartridge) {
            self.cartridge.ppu_read_complete(fetch.address);
        }
        if let Some(z) = &mut self.zapper
            && let Some((x, y, c)) = self.ppu.beam_pixel()
        {
            z.sample(x, y, c, self.ppu.total_dots());
        }
        self.cartridge
            .observe_ppu(self.ppu.bus_address(), self.ppu.total_dots());
    }
    fn sample_nmi(&mut self) {
        let line = self.ppu.nmi_line();
        self.nmi_pending |= line && !self.nmi_previous;
        self.nmi_previous = line;
    }
}
impl MachineBus {
    fn dmc_idle_read(&mut self, address: u16) {
        // On an NES-001, joypad /OE stays asserted across consecutive
        // halted reads. It clocks once at the halt, then again when the CPU
        // resumes after the sample get; dummy/alignment cycles add no edges.
        if matches!(address, 0x4016..=0x4017) {
            self.clock();
        } else {
            self.read_cycle(address);
        }
    }
    fn read_cycle(&mut self, address: u16) -> u8 {
        let value = match address {
            0..=0x1fff => self.ram[address as usize & 0x7ff],
            0x4017 if self.zapper.is_some() => {
                (self.open_bus & 0xe0) | self.zapper.as_ref().unwrap().read(self.ppu.total_dots())
            }
            0x4016..=0x4017 => {
                (self.open_bus & 0xe0) | self.controllers[address as usize & 1].read()
            }
            0x2000..=0x3fff => {
                let vram = self.ppu.data_address();
                let value = self.ppu.read_register(&self.cartridge, address);
                if address & 7 == 7 {
                    self.cartridge.ppu_read_complete(if vram >= 0x3f00 {
                        vram - 0x1000
                    } else {
                        vram
                    });
                }
                value
            }
            0x4014 => self.open_bus, // OAMDMA is write-only
            0x4015 => self.apu.read_status() | (self.open_bus & 0x20),
            0x4000..=0x4013 => self.open_bus,
            0x4020..=0xffff => self.cartridge.cpu_read(address).unwrap_or(self.open_bus),
            _ => self.open_bus,
        };
        self.cartridge.cpu_read_complete(address);
        self.open_bus = value;
        self.clock();
        value
    }
}
impl Bus for MachineBus {
    fn take_stall_cycles(&mut self) -> u64 {
        std::mem::take(&mut self.stall_cycles)
    }
    fn poll_interrupts(&mut self, disabled: bool) {
        self.nmi_scheduled |= self.nmi_history[0] && self.nmi_pending;
        self.irq_pending |= self.irq_history[0] && !disabled;
    }
    fn take_nmi(&mut self) -> bool {
        if self.nmi_history[0] && self.nmi_pending {
            self.nmi_pending = false;
            self.nmi_scheduled = false;
            self.nmi_history = [false; 2];
            true
        } else {
            false
        }
    }
    fn read(&mut self, address: u16, cycle: u64) -> u8 {
        if let Some(sample_address) = self.apu.dmc_request() {
            let mut elapsed = 0;
            // Halt, dummy, optional alignment, and sample get on an odd cycle.
            if self.cartridge.is_pal() {
                self.clock();
            } else {
                self.read_cycle(address);
            }
            elapsed += 1;
            self.dmc_idle_read(address);
            elapsed += 1;
            if (cycle + elapsed) & 1 == 0 {
                self.dmc_idle_read(address);
                elapsed += 1;
            }
            let sample = self.read_cycle(sample_address);
            elapsed += 1;
            self.apu.complete_dmc(sample);
            self.stall_cycles += elapsed;
        }
        self.read_cycle(address)
    }
    fn write(&mut self, address: u16, value: u8, cycle: u64) {
        self.open_bus = value;
        match address {
            0..=0x1fff => self.ram[address as usize & 0x7ff] = value,
            0x4016 => self
                .controllers
                .iter_mut()
                .for_each(|p| p.strobe(value & 1 != 0)),
            0x2000..=0x3fff => {
                self.ppu
                    .write_register(&mut self.cartridge, address, value)
                    .unwrap();
            }
            0x4014 => self.dma_page = Some(value),
            0x4000..=0x4017 => self.apu.write(address, value),
            0x4020..=0xffff => self.cartridge.cpu_write(address, value, cycle),
            _ => (),
        }
        self.clock();
    }
}
