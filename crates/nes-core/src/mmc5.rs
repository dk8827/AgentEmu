//! MMC5 banking, ExRAM, nametable routing, multiplier and PPU-read IRQ detector.
//! Register behavior follows NESdev's MMC5 hardware reference. Expansion audio
//! and vertical split are not used by the US Castlevania III / Gemfire targets.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mmc5 {
    pub prg_mode: u8,
    pub chr_mode: u8,
    pub protect: [u8; 2],
    pub ex_mode: u8,
    pub nt: u8,
    pub fill: [u8; 2],
    pub prg: [u8; 5],
    pub chr: [u16; 12],
    pub high: u8,
    pub last_bg: bool,
    pub exram: Vec<u8>,
    pub multiply: [u8; 2],
    pub irq_target: u8,
    pub irq_enable: bool,
    pub pending: bool,
    pub in_frame: bool,
    pub line: u8,
    pub last_address: u16,
    pub matches: u8,
    pub idle: u8,
    pub read: bool,
}
impl Default for Mmc5 {
    fn default() -> Self {
        Self {
            prg_mode: 3,
            chr_mode: 3,
            protect: [1, 2],
            ex_mode: 3,
            nt: 0,
            fill: [0; 2],
            prg: [0, 0x80, 0x80, 0x80, 0xff],
            chr: [0; 12],
            high: 0,
            last_bg: false,
            exram: vec![0; 1024],
            multiply: [0; 2],
            irq_target: 0,
            irq_enable: false,
            pending: false,
            in_frame: false,
            line: 0,
            last_address: 0xffff,
            matches: 0,
            idle: 0,
            read: false,
        }
    }
}
impl Mmc5 {
    pub fn prg_bank(&self, address: u16) -> (bool, usize) {
        if address < 0x8000 {
            return (false, (self.prg[0] & 7) as usize);
        }
        let slot = (address as usize - 0x8000) / 8192;
        let (reg, mask, offset) = match self.prg_mode {
            0 => (4, 3, slot),
            1 => (if slot < 2 { 2 } else { 4 }, 1, slot & 1),
            2 if slot < 2 => (2, 1, slot & 1),
            _ => (slot + 1, 0, 0),
        };
        let rom = reg == 4 || self.prg[reg] & 128 != 0;
        (
            rom,
            ((self.prg[reg] as usize & if rom { 127 } else { 7 }) & !mask) + offset,
        )
    }
    pub fn write(&mut self, a: u16, v: u8) -> bool {
        match a {
            0x5100 => self.prg_mode = v & 3,
            0x5101 => self.chr_mode = v & 3,
            0x5102..=0x5103 => self.protect[(a - 0x5102) as usize] = v & 3,
            0x5104 => self.ex_mode = v & 3,
            0x5105 => self.nt = v,
            0x5106 => self.fill[0] = v,
            0x5107 => self.fill[1] = v & 3,
            0x5113..=0x5117 => self.prg[(a - 0x5113) as usize] = v,
            0x5120..=0x512b => {
                self.chr[(a - 0x5120) as usize] = v as u16 | ((self.high as u16) << 8);
                self.last_bg = a >= 0x5128;
            }
            0x5130 => self.high = v & 3,
            0x5203 => self.irq_target = v,
            0x5204 => self.irq_enable = v & 128 != 0,
            0x5205..=0x5206 => self.multiply[(a - 0x5205) as usize] = v,
            0x5c00..=0x5fff => {
                if self.ex_mode != 3 {
                    self.exram[(a & 1023) as usize] = if self.ex_mode < 2 && !self.in_frame {
                        0
                    } else {
                        v
                    };
                }
            }
            _ => return false,
        }
        true
    }
    pub fn read(&self, a: u16) -> Option<u8> {
        match a {
            0x5204 => Some((u8::from(self.pending) << 7) | (u8::from(self.in_frame) << 6)),
            0x5205 => Some((self.multiply[0] as u16 * self.multiply[1] as u16) as u8),
            0x5206 => Some(((self.multiply[0] as u16 * self.multiply[1] as u16) >> 8) as u8),
            0x5c00..=0x5fff if self.ex_mode >= 2 => Some(self.exram[(a & 1023) as usize]),
            _ => None,
        }
    }
    pub fn chr_index(&self, a: u16, bg: bool, sprites16: bool, v: u16) -> usize {
        if bg && self.ex_mode == 1 {
            let bank =
                (self.exram[(v & 1023) as usize] & 63) as usize | ((self.high as usize) << 6);
            return bank * 4096 + (a as usize & 4095);
        }
        let bg = if sprites16 { bg } else { self.last_bg };
        let unit = 8192usize >> self.chr_mode;
        let slot = (a as usize / unit + 1) * (unit / 1024) - 1;
        let reg = if bg { 8 + (slot & 3) } else { slot };
        self.chr[reg] as usize * unit + (a as usize & (unit - 1))
    }
    pub fn ppu_read(&mut self, a: u16) {
        self.read = true;
        if self.matches == 2 {
            self.matches = 0;
            if !self.in_frame {
                self.in_frame = true;
                self.line = 0;
                self.pending = false;
            } else {
                self.line = self.line.wrapping_add(1);
                if self.line == self.irq_target && self.irq_target != 0 {
                    self.pending = true;
                }
            }
        }
        if (0x2000..0x3000).contains(&a) && a == self.last_address {
            self.matches = self.matches.saturating_add(1).min(2);
        } else {
            self.matches = 0;
        }
        self.last_address = a;
    }
    pub fn clock(&mut self) {
        if self.read {
            self.idle = 0;
        } else {
            self.idle = self.idle.saturating_add(1).min(3);
            if self.idle == 3 {
                self.in_frame = false;
                self.matches = 0;
                self.last_address = 0xffff;
            }
        }
        self.read = false;
    }
    pub fn reset_frame(&mut self) {
        self.in_frame = false;
        self.pending = false;
        self.line = 0;
        self.matches = 0;
        self.last_address = 0xffff;
    }
    pub fn valid(&self) -> bool {
        self.prg_mode < 4
            && self.chr_mode < 4
            && self.protect.iter().all(|v| *v < 4)
            && self.ex_mode < 4
            && self.fill[1] < 4
            && self.high < 4
            && self.chr.iter().all(|v| *v < 1024)
            && self.exram.len() == 1024
            && self.matches <= 2
            && self.idle <= 3
    }
}
