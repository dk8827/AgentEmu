//! Tengen RAMBO-1 (mapper 64). Register layout: NESdev RAMBO-1;
//! IRQ reload/delay behavior cross-checked against Mesen2's Rambo1.h.
//! This is a separate board, not an MMC3 revision.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Rambo1 {
    select: u8,
    banks: [u8; 16],
    pub horizontal: bool,
    latch: u8,
    counter: u8,
    reload: bool,
    cycle_mode: bool,
    divider: u8,
    force_clock: bool,
    enabled: bool,
    pub pending: bool,
    delay: u8,
    a12: bool,
    low_since: u64,
}

impl Rambo1 {
    pub fn new(horizontal: bool) -> Self {
        Self {
            horizontal,
            ..Self::default()
        }
    }
    pub fn prg_bank(&self, address: u16, total: usize) -> usize {
        let order = if self.select & 0x40 == 0 {
            [6, 7, 15]
        } else {
            [15, 6, 7]
        };
        let slot = (address as usize - 0x8000) / 8192;
        if slot == 3 {
            total - 1
        } else {
            self.banks[order[slot]] as usize % total
        }
    }
    pub fn chr_bank(&self, address: u16) -> usize {
        let slot = (address as usize / 1024) ^ if self.select & 0x80 != 0 { 4 } else { 0 };
        let bank = if slot >= 4 {
            self.banks[slot - 2]
        } else if self.select & 0x20 != 0 {
            self.banks[[0, 8, 1, 9][slot]]
        } else {
            (self.banks[slot / 2] & !1) | (slot as u8 & 1)
        };
        bank as usize
    }
    pub fn write(&mut self, address: u16, value: u8) {
        match address & 0xe001 {
            0x8000 => self.select = value,
            0x8001 => self.banks[(self.select & 15) as usize] = value,
            0xa000 => self.horizontal = value & 1 != 0,
            0xc000 => self.latch = value,
            0xc001 => {
                self.force_clock |= self.cycle_mode && value & 1 == 0;
                self.cycle_mode = value & 1 != 0;
                if self.cycle_mode {
                    self.divider = 0;
                }
                self.reload = true;
            }
            0xe000 => {
                self.enabled = false;
                self.pending = false;
                self.delay = 0;
            }
            0xe001 => self.enabled = true,
            _ => (), // No PRG RAM or MMC3 $A001 register.
        }
    }
    fn clock_counter(&mut self, delay: u8) {
        if self.reload {
            self.counter = self.latch.wrapping_add(if self.latch > 1 { 2 } else { 1 });
            self.reload = false;
        } else if self.counter == 0 {
            self.counter = self.latch.wrapping_add(1);
        }
        self.counter = self.counter.wrapping_sub(1);
        if self.counter == 0 && self.enabled {
            self.delay = delay;
        }
    }
    pub fn clock_cpu(&mut self) {
        if self.delay > 0 {
            self.delay -= 1;
            if self.delay == 0 {
                self.pending = true;
            }
        }
        if self.cycle_mode || self.force_clock {
            self.divider = (self.divider + 1) & 3;
            if self.divider == 0 {
                self.clock_counter(1);
                self.force_clock = false;
            }
        }
    }
    pub fn observe_ppu(&mut self, address: u16, dot: u64) {
        let high = address & 0x1000 != 0;
        if !high && self.a12 {
            self.low_since = dot;
        }
        // RAMBO-1 filters short A12 pulses more aggressively than MMC3.
        if high && !self.a12 && !self.cycle_mode && dot.saturating_sub(self.low_since) >= 30 {
            self.clock_counter(2);
        }
        self.a12 = high;
    }
    pub fn valid(&self, dots: u64) -> bool {
        self.divider < 4 && self.delay <= 2 && self.low_since <= dots
    }
}
