//! NTSC 2A03 audio hardware. Timers run independently of presentation.
//! See docs/APU.md for clock conventions, sampling and remaining timing limits.
use serde::{Deserialize, Serialize};

const LENGTH: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];
const NOISE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];
const DMC: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];
pub const SAMPLE_RATE: u32 = 48_000;
const MASTER_CLOCK: u32 = 21_477_272;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    control: u8,
    start: bool,
    divider: u8,
    decay: u8,
}
impl Envelope {
    fn tick(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.control & 15;
        } else if self.divider > 0 {
            self.divider -= 1;
        } else {
            self.divider = self.control & 15;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.control & 0x20 != 0 {
                self.decay = 15;
            }
        }
    }
    fn volume(&self) -> u8 {
        if self.control & 0x10 != 0 {
            self.control & 15
        } else {
            self.decay
        }
    }
    fn valid(&self) -> bool {
        self.divider < 16 && self.decay < 16
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pulse {
    envelope: Envelope,
    length: u8,
    period: u16,
    timer: u16,
    phase: u8,
    sweep: u8,
    sweep_divider: u8,
    sweep_reload: bool,
}
impl Pulse {
    fn target(&self, first: bool) -> i32 {
        let delta = (self.period >> (self.sweep & 7)) as i32;
        if self.sweep & 8 != 0 {
            (self.period as i32 - delta - i32::from(first)).max(0)
        } else {
            self.period as i32 + delta
        }
    }
    fn muted(&self, first: bool) -> bool {
        self.period < 8 || self.target(first) > 0x7ff
    }
    fn half(&mut self, first: bool) {
        if self.length > 0 && self.envelope.control & 0x20 == 0 {
            self.length -= 1;
        }
        if self.sweep_divider == 0
            && self.sweep & 0x80 != 0
            && self.sweep & 7 != 0
            && !self.muted(first)
        {
            self.period = self.target(first) as u16;
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = (self.sweep >> 4) & 7;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }
    fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.phase = (self.phase + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }
    fn output(&self, first: bool) -> u8 {
        const DUTY: [u8; 4] = [0b00000010, 0b00000110, 0b00011110, 0b11111001];
        if self.length == 0
            || self.muted(first)
            || DUTY[(self.envelope.control >> 6) as usize] & (1 << self.phase) == 0
        {
            0
        } else {
            self.envelope.volume()
        }
    }
    fn valid(&self) -> bool {
        self.envelope.valid()
            && self.period < 2048
            && self.timer < 2048
            && self.phase < 8
            && self.sweep_divider < 8
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Triangle {
    control: u8,
    length: u8,
    linear: u8,
    reload: bool,
    period: u16,
    timer: u16,
    phase: u8,
}
impl Triangle {
    fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            if self.length > 0 && self.linear > 0 {
                self.phase = (self.phase + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }
    fn quarter(&mut self) {
        if self.reload {
            self.linear = self.control & 0x7f;
        } else if self.linear > 0 {
            self.linear -= 1;
        }
        if self.control & 0x80 == 0 {
            self.reload = false;
        }
    }
    fn output(&self) -> u8 {
        if self.phase < 16 {
            15 - self.phase
        } else {
            self.phase - 16
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Noise {
    envelope: Envelope,
    length: u8,
    mode_period: u8,
    timer: u16,
    shift: u16,
}
impl Default for Noise {
    fn default() -> Self {
        Self {
            envelope: Default::default(),
            length: 0,
            mode_period: 0,
            timer: 0,
            shift: 1,
        }
    }
}
impl Noise {
    fn tick(&mut self, pal: bool) {
        if self.timer == 0 {
            self.timer = (if pal {
                [
                    4, 8, 14, 30, 60, 88, 118, 148, 188, 236, 354, 472, 708, 944, 1890, 3778,
                ]
            } else {
                NOISE
            })[(self.mode_period & 15) as usize]
                - 1;
            let bit = if self.mode_period & 0x80 != 0 { 6 } else { 1 };
            let feedback = (self.shift ^ (self.shift >> bit)) & 1;
            self.shift = (self.shift >> 1) | (feedback << 14);
        } else {
            self.timer -= 1;
        }
    }
    fn output(&self) -> u8 {
        if self.length == 0 || self.shift & 1 != 0 {
            0
        } else {
            self.envelope.volume()
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dmc {
    control: u8,
    output: u8,
    address_reg: u8,
    length_reg: u8,
    address: u16,
    remaining: u16,
    timer: u16,
    bits: u8,
    shift: u8,
    silent: bool,
    buffer: Option<u8>,
    irq: bool,
    dma_delay: u8,
}
impl Default for Dmc {
    fn default() -> Self {
        Self {
            control: 0,
            output: 0,
            address_reg: 0,
            length_reg: 0,
            address: 0xc000,
            remaining: 0,
            timer: 427,
            bits: 8,
            shift: 0,
            silent: true,
            buffer: None,
            irq: false,
            dma_delay: 0,
        }
    }
}
impl Dmc {
    fn restart(&mut self) {
        self.address = 0xc000 | (self.address_reg as u16) << 6;
        self.remaining = (self.length_reg as u16) * 16 + 1;
    }
    fn tick(&mut self, pal: bool) {
        if self.dma_delay > 0 {
            self.dma_delay -= 1;
        }
        if self.timer > 0 {
            self.timer -= 1;
            return;
        }
        self.timer = (if pal {
            [
                398, 354, 316, 298, 276, 236, 210, 198, 176, 148, 132, 118, 98, 78, 66, 50,
            ]
        } else {
            DMC
        })[(self.control & 15) as usize]
            - 1;
        if !self.silent {
            if self.shift & 1 != 0 {
                if self.output <= 125 {
                    self.output += 2;
                }
            } else if self.output >= 2 {
                self.output -= 2;
            }
        }
        self.shift >>= 1;
        self.bits -= 1;
        if self.bits == 0 {
            self.bits = 8;
            if let Some(value) = self.buffer.take() {
                self.shift = value;
                self.silent = false;
            } else {
                self.silent = true;
            }
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Apu {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pal: bool,
    pulse: [Pulse; 2],
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,
    enabled: u8,
    cycles: u64,
    frame_cycle: u32,
    mode5: bool,
    irq_inhibit: bool,
    frame_irq: bool,
    reset_delay: u8,
    sample_phase: u32,
    sample_sum: i64,
    sample_count: u16,
    previous_sample: i64,
    high_pass: i64,
    low_pass: i64,
    #[serde(skip)]
    samples: Vec<f32>,
}
impl Apu {
    pub fn with_region(pal: bool) -> Self {
        Self {
            pal,
            ..Self::default()
        }
    }
    pub fn is_pal(&self) -> bool {
        self.pal
    }
    pub fn cycles(&self) -> u64 {
        self.cycles
    }
    pub fn irq(&self) -> bool {
        self.frame_irq || self.dmc.irq
    }
    pub fn status(&self) -> u8 {
        u8::from(self.pulse[0].length > 0)
            | (u8::from(self.pulse[1].length > 0) << 1)
            | (u8::from(self.triangle.length > 0) << 2)
            | (u8::from(self.noise.length > 0) << 3)
            | (u8::from(self.dmc.remaining > 0) << 4)
            | (u8::from(self.frame_irq) << 6)
            | (u8::from(self.dmc.irq) << 7)
    }
    pub fn read_status(&mut self) -> u8 {
        let value = self.status();
        self.frame_irq = false;
        value
    }
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0x4000..=0x4007 => {
                let index = ((address - 0x4000) / 4) as usize;
                let p = &mut self.pulse[index];
                match address & 3 {
                    0 => p.envelope.control = value,
                    1 => {
                        p.sweep = value;
                        p.sweep_reload = true;
                    }
                    2 => p.period = (p.period & 0x700) | value as u16,
                    _ => {
                        p.period = (p.period & 255) | ((value as u16 & 7) << 8);
                        if self.enabled & (1 << index) != 0 {
                            p.length = LENGTH[(value >> 3) as usize];
                        }
                        p.phase = 0;
                        p.envelope.start = true;
                    }
                }
            }
            0x4008 => self.triangle.control = value,
            0x400a => self.triangle.period = (self.triangle.period & 0x700) | value as u16,
            0x400b => {
                self.triangle.period = (self.triangle.period & 255) | ((value as u16 & 7) << 8);
                if self.enabled & 4 != 0 {
                    self.triangle.length = LENGTH[(value >> 3) as usize];
                }
                self.triangle.reload = true;
            }
            0x400c => self.noise.envelope.control = value,
            0x400e => self.noise.mode_period = value,
            0x400f => {
                if self.enabled & 8 != 0 {
                    self.noise.length = LENGTH[(value >> 3) as usize];
                }
                self.noise.envelope.start = true;
            }
            0x4010 => {
                self.dmc.control = value;
                if value & 0x80 == 0 {
                    self.dmc.irq = false;
                }
            }
            0x4011 => self.dmc.output = value & 127,
            0x4012 => self.dmc.address_reg = value,
            0x4013 => self.dmc.length_reg = value,
            0x4015 => {
                self.enabled = value & 31;
                for i in 0..2 {
                    if value & (1 << i) == 0 {
                        self.pulse[i].length = 0;
                    }
                }
                if value & 4 == 0 {
                    self.triangle.length = 0;
                }
                if value & 8 == 0 {
                    self.noise.length = 0;
                }
                self.dmc.irq = false;
                if value & 16 == 0 {
                    self.dmc.remaining = 0;
                } else if self.dmc.remaining == 0 {
                    self.dmc.restart();
                    self.dmc.dma_delay = if self.cycles & 1 == 0 { 3 } else { 2 };
                }
            }
            0x4017 => {
                self.mode5 = value & 0x80 != 0;
                self.irq_inhibit = value & 0x40 != 0;
                if self.irq_inhibit {
                    self.frame_irq = false;
                }
                self.reset_delay = if self.cycles & 1 == 0 { 3 } else { 4 };
            }
            _ => (),
        }
    }
    fn quarter(&mut self) {
        for p in &mut self.pulse {
            p.envelope.tick();
        }
        self.noise.envelope.tick();
        self.triangle.quarter();
    }
    fn half(&mut self) {
        self.pulse[0].half(true);
        self.pulse[1].half(false);
        if self.triangle.length > 0 && self.triangle.control & 0x80 == 0 {
            self.triangle.length -= 1;
        }
        if self.noise.length > 0 && self.noise.envelope.control & 0x20 == 0 {
            self.noise.length -= 1;
        }
    }
    pub fn tick(&mut self) {
        self.cycles = self.cycles.wrapping_add(1);
        self.frame_cycle += 1;
        if self.reset_delay > 0 {
            self.reset_delay -= 1;
            if self.reset_delay == 0 {
                self.frame_cycle = 0;
                if self.mode5 {
                    self.quarter();
                    self.half();
                }
            }
        }
        let [q1, h1, q2, h2, h3] = if self.pal {
            [8313, 16627, 24939, 33253, 41565]
        } else {
            [7457, 14913, 22371, 29829, 37281]
        };
        match self.frame_cycle {
            n if n == q1 || n == q2 => self.quarter(),
            n if n == h1 => {
                self.quarter();
                self.half();
            }
            n if n == h2 && !self.mode5 => {
                self.quarter();
                self.half();
            }
            n if n == h3 && self.mode5 => {
                self.quarter();
                self.half();
            }
            _ => (),
        }
        if !self.mode5 && (h2 - 1..=h2 + 1).contains(&self.frame_cycle) && !self.irq_inhibit {
            self.frame_irq = true;
        }
        if self.frame_cycle == if self.mode5 { h3 + 1 } else { h2 + 1 } {
            self.frame_cycle = 0;
        }
        self.triangle.tick();
        self.noise.tick(self.pal);
        self.dmc.tick(self.pal);
        if self.cycles & 1 == 0 {
            for p in &mut self.pulse {
                p.tick();
            }
        }
        // Integer nonlinear mix and box integration keep native/WASM samples stable.
        let pulse = self.pulse[0].output(true) as i64 + self.pulse[1].output(false) as i64;
        let tnd = 3 * self.triangle.output() as i64
            + 2 * self.noise.output() as i64
            + self.dmc.output as i64;
        self.sample_sum += 9588 * pulse * 65536 / (100 * (8128 + 100 * pulse))
            + 15979 * tnd * 65536 / (100 * (24329 + 100 * tnd));
        self.sample_count += 1;
        let master = if self.pal { 26_601_712 } else { MASTER_CLOCK };
        self.sample_phase += SAMPLE_RATE * if self.pal { 16 } else { 12 };
        if self.sample_phase >= master {
            self.sample_phase -= master;
            let sample = self.sample_sum / self.sample_count as i64;
            self.high_pass = sample - self.previous_sample + self.high_pass * 65000 / 65536;
            self.previous_sample = sample;
            self.low_pass += (self.high_pass - self.low_pass) / 2;
            if self.samples.len() < SAMPLE_RATE as usize * 2 {
                self.samples.push(self.low_pass as f32 / 65536.0);
            }
            self.sample_sum = 0;
            self.sample_count = 0;
        }
    }
    /// A pending memory request halts the CPU on a read cycle, not on writes.
    pub fn dmc_request(&self) -> Option<u16> {
        if self.dmc.remaining > 0 && self.dmc.buffer.is_none() && self.dmc.dma_delay == 0 {
            Some(self.dmc.address)
        } else {
            None
        }
    }
    pub fn complete_dmc(&mut self, value: u8) {
        self.dmc.buffer = Some(value);
        self.dmc.address = if self.dmc.address == 0xffff {
            0x8000
        } else {
            self.dmc.address + 1
        };
        self.dmc.remaining -= 1;
        if self.dmc.remaining == 0 {
            if self.dmc.control & 0x40 != 0 {
                self.dmc.restart();
            } else if self.dmc.control & 0x80 != 0 {
                self.dmc.irq = true;
            }
        }
    }
    /// Host output queue is bounded and deliberately excluded from hardware snapshots.
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }
    pub(crate) fn valid_state(&self, cycles: u64) -> bool {
        self.cycles == cycles
            && self.pulse.iter().all(Pulse::valid)
            && self.triangle.period < 2048
            && self.triangle.timer < 2048
            && self.triangle.phase < 32
            && self.triangle.linear < 128
            && self.noise.envelope.valid()
            && self.noise.shift > 0
            && self.noise.shift < 32768
            && self.noise.timer < 4068
            && self.dmc.output < 128
            && self.dmc.address >= 0x8000
            && self.dmc.remaining <= 4081
            && self.dmc.timer < 428
            && (1..=8).contains(&self.dmc.bits)
            && self.dmc.dma_delay <= 3
            && self.enabled < 32
            && self.frame_cycle < if self.pal { 41566 } else { 37282 }
            && self.reset_delay <= 5
            && self.sample_phase < if self.pal { 26_601_712 } else { MASTER_CLOCK }
            && self.sample_count < 38
            && self.sample_sum.unsigned_abs() < 38 * 65536
            && self.previous_sample.unsigned_abs() < 65536
            && self.high_pass.unsigned_abs() < 131072
            && self.low_pass.unsigned_abs() < 131072
    }
}
