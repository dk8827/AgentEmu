//! NTSC 2C02 registers, memory, and background fetch/rendering pipeline.
//!
//! Coordinates name the next dot to execute. CPU accesses occur before that dot.
//! CPU sampling is coordinated by Machine. Bus decay is deterministic;
//! silicon-specific corruption and sub-dot races remain approximations. See docs/PPU.md for the timing model and limitations.
use crate::cartridge::{Cartridge, Mirroring};
use serde::{Deserialize, Serialize};

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

/// One completed background/dummy fetch, for diagnostics and future mapper wiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpuFetch {
    pub dot: u64,
    pub address: u16,
    pub value: u8,
}

pub const DOTS_PER_FRAME: u64 = 341 * 262;
/// Deterministic 0.6-second approximation; physical decay varies by chip/temperature.
pub const IO_DECAY_DOTS: u64 = DOTS_PER_FRAME * 36;
/// Deterministic approximation of the hardware's initial write-inhibit period.
pub const WRITE_INHIBIT_DOTS: u64 = 29_658 * 3;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sprite {
    low: u8,
    high: u8,
    x: u8,
    attributes: u8,
    zero: bool,
    valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ppu {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pal: bool,
    nametables: Vec<u8>,
    palette: [u8; 32],
    oam: Vec<u8>,
    ctrl: u8,
    mask: u8,
    next_mask: u8,
    mask_delay: u8,
    status: u8,
    oam_addr: u8,
    v: u16,
    t: u16,
    fine_x: u8,
    second_write: bool,
    read_buffer: u8,
    io_latch: u8,
    io_refreshed: [u64; 8],
    total_dots: u64,
    suppress_vblank: bool,
    frame: u64,
    scanline: u16,
    dot: u16,
    skipped_dots: u64,
    completed_frames: u64,
    front: Vec<u16>,
    back: Vec<u16>,
    pattern_low: u16,
    pattern_high: u16,
    attribute_low: u16,
    attribute_high: u16,
    tile: u8,
    tile_attribute: u8,
    tile_low: u8,
    fetch_address: u16,
    secondary: [u8; 32],
    secondary_ids: [u8; 8],
    secondary_count: u8,
    eval_n: u8,
    eval_m: u8,
    eval_done: bool,
    overflow_copy: u8,
    oam_bus: u8,
    sprites: [Sprite; 8],
}
impl Default for Ppu {
    fn default() -> Self {
        Self {
            pal: false,
            nametables: vec![0; 4096],
            palette: [0; 32],
            oam: vec![0; 256],
            ctrl: 0,
            mask: 0,
            next_mask: 0,
            mask_delay: 0,
            status: 0,
            oam_addr: 0,
            v: 0,
            t: 0,
            fine_x: 0,
            second_write: false,
            read_buffer: 0,
            io_latch: 0,
            io_refreshed: [0; 8],
            total_dots: 0,
            suppress_vblank: false,
            frame: 0,
            scanline: 0,
            dot: 0,
            skipped_dots: 0,
            completed_frames: 0,
            front: vec![0; WIDTH * HEIGHT],
            back: vec![0; WIDTH * HEIGHT],
            pattern_low: 0,
            pattern_high: 0,
            attribute_low: 0,
            attribute_high: 0,
            tile: 0,
            tile_attribute: 0,
            tile_low: 0,
            fetch_address: 0,
            secondary: [0xff; 32],
            secondary_ids: [0xff; 8],
            secondary_count: 0,
            eval_n: 0,
            eval_m: 0,
            eval_done: false,
            overflow_copy: 0,
            oam_bus: 0xff,
            sprites: [Sprite::default(); 8],
        }
    }
}
impl Ppu {
    pub fn with_region(pal: bool) -> Self {
        Self {
            pal,
            ..Self::default()
        }
    }
    pub fn beam_pixel(&self) -> Option<(u16, u16, u16)> {
        if self.scanline < 240 && (2..=257).contains(&self.dot) {
            let x = self.dot - 2;
            Some((
                x,
                self.scanline,
                self.back[self.scanline as usize * 256 + x as usize],
            ))
        } else {
            None
        }
    }
    pub fn is_pal(&self) -> bool {
        self.pal
    }
    fn pre_render(&self) -> u16 {
        if self.pal { 311 } else { 261 }
    }
    pub fn total_dots(&self) -> u64 {
        self.total_dots
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
    pub fn scanline(&self) -> u16 {
        self.scanline
    }
    pub fn dot(&self) -> u16 {
        self.dot
    }
    /// Asserted level, not a CPU-sampled interrupt or an edge latch.
    pub fn nmi_line(&self) -> bool {
        self.ctrl & self.status & 0x80 != 0
    }
    /// Completed frame: six-bit palette color plus PPUMASK emphasis in bits 6..8.
    /// This immutable front buffer remains stable while the next frame is drawn.
    pub fn framebuffer(&self) -> &[u16] {
        &self.front
    }
    pub fn completed_frames(&self) -> u64 {
        self.completed_frames
    }
    fn rendering(&self) -> bool {
        self.mask & 0x18 != 0
    }
    fn render_line(&self) -> bool {
        self.scanline < 240 || self.scanline == self.pre_render()
    }
    pub fn data_address(&self) -> u16 {
        self.v & 0x3fff
    }
    pub fn bus_address(&self) -> u16 {
        if self.rendering() && self.render_line() {
            self.fetch_address
        } else {
            self.v & 0x3fff
        }
    }
    pub fn tick(&mut self, cartridge: &Cartridge) -> Option<PpuFetch> {
        let mut fetch = None;
        if self.mask_delay != 0 {
            self.mask_delay -= 1;
            if self.mask_delay == 0 {
                self.mask = self.next_mask;
            }
        }
        match (self.scanline, self.dot) {
            (241, 1) => {
                std::mem::swap(&mut self.front, &mut self.back);
                self.completed_frames = self.completed_frames.wrapping_add(1);
                if !self.suppress_vblank {
                    self.status |= 0x80;
                }
                self.suppress_vblank = false;
            }
            (line, 1) if line == self.pre_render() => self.status &= !0xe0,
            _ => (),
        }
        if self.scanline < 240 && (1..=256).contains(&self.dot) {
            self.output_pixel();
        }
        if self.rendering() && self.render_line() {
            if self.scanline < 240 {
                self.evaluate_sprites();
            }
            if (257..=320).contains(&self.dot) {
                self.oam_addr = 0;
                fetch = self.fetch_sprite(cartridge);
            }
            if (1..=256).contains(&self.dot) || (321..=336).contains(&self.dot) {
                // Output uses the old high bits. Shifting then loading at dot 8
                // leaves both prefetched tiles aligned for dot 1 of the next line.
                self.pattern_low <<= 1;
                self.pattern_high <<= 1;
                self.attribute_low <<= 1;
                self.attribute_high <<= 1;
                match (self.dot - 1) & 7 {
                    0 => self.fetch_address = 0x2000 | (self.v & 0x0fff),
                    2 => {
                        self.fetch_address = 0x23c0
                            | (self.v & 0x0c00)
                            | ((self.v >> 4) & 0x38)
                            | ((self.v >> 2) & 7)
                    }
                    4 => {
                        self.fetch_address = ((self.ctrl as u16 & 0x10) << 8)
                            | ((self.tile as u16) << 4)
                            | ((self.v >> 12) & 7)
                    }
                    6 => self.fetch_address += 8,
                    phase => {
                        let value = if self.fetch_address < 0x2000 {
                            cartridge.render_chr(
                                self.fetch_address,
                                true,
                                self.ctrl & 32 != 0,
                                self.v,
                            )
                        } else {
                            self.peek_memory(cartridge, self.fetch_address)
                        };
                        fetch = Some(PpuFetch {
                            dot: self.total_dots,
                            address: self.fetch_address,
                            value,
                        });
                        match phase {
                            1 => self.tile = value,
                            3 => {
                                let shift = ((self.v >> 4) & 4) | (self.v & 2);
                                self.tile_attribute = cartridge
                                    .extended_attribute(self.v)
                                    .unwrap_or((value >> shift) & 3);
                            }
                            5 => self.tile_low = value,
                            7 => {
                                self.pattern_low =
                                    (self.pattern_low & 0xff00) | self.tile_low as u16;
                                self.pattern_high = (self.pattern_high & 0xff00) | value as u16;
                                self.attribute_low = (self.attribute_low & 0xff00)
                                    | if self.tile_attribute & 1 != 0 {
                                        0xff
                                    } else {
                                        0
                                    };
                                self.attribute_high = (self.attribute_high & 0xff00)
                                    | if self.tile_attribute & 2 != 0 {
                                        0xff
                                    } else {
                                        0
                                    };
                                self.increment_x();
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
            if self.dot == 256 {
                self.increment_y();
            }
            if self.dot == 257 {
                self.v = (self.v & !0x041f) | (self.t & 0x041f);
            }
            if self.scanline == self.pre_render() && (280..=304).contains(&self.dot) {
                self.v = (self.v & !0x7be0) | (self.t & 0x7be0);
            }
            if self.dot == 337 || self.dot == 339 {
                self.fetch_address = 0x2000 | (self.v & 0x0fff);
            } else if self.dot == 338 || self.dot == 340 {
                fetch = Some(PpuFetch {
                    dot: self.total_dots,
                    address: self.fetch_address,
                    value: self.peek_memory(cartridge, self.fetch_address),
                });
            }
        }
        self.total_dots = self.total_dots.wrapping_add(1);
        if !self.pal
            && self.rendering()
            && self.frame & 1 != 0
            && self.scanline == self.pre_render()
            && self.dot == 339
        {
            self.dot = 0;
            self.scanline = 0;
            self.frame = self.frame.wrapping_add(1);
            self.skipped_dots = self.skipped_dots.wrapping_add(1);
        } else if self.dot == 340 {
            self.dot = 0;
            if self.scanline == self.pre_render() {
                self.scanline = 0;
                self.frame = self.frame.wrapping_add(1);
            } else {
                self.scanline += 1;
            }
        } else {
            self.dot += 1;
        }
        fetch
    }
    fn output_pixel(&mut self) {
        let x = self.dot as usize - 1;
        let bit = 0x8000 >> self.fine_x;
        let pixel = if self.mask & 8 != 0 && (x >= 8 || self.mask & 2 != 0) {
            u8::from(self.pattern_low & bit != 0) | (u8::from(self.pattern_high & bit != 0) << 1)
        } else {
            0
        };
        let palette = u8::from(self.attribute_low & bit != 0)
            | (u8::from(self.attribute_high & bit != 0) << 1);
        let mut index = if !self.rendering() && self.v & 0x3fff >= 0x3f00 {
            Self::palette_index(self.v) // forced-blank palette override
        } else if pixel == 0 {
            0
        } else {
            (palette * 4 + pixel) as usize
        };
        let mut selected = None;
        if self.rendering() {
            for sprite in &mut self.sprites {
                if sprite.x != 0 {
                    sprite.x -= 1;
                    continue;
                }
                let color = (sprite.low >> 7) | ((sprite.high >> 6) & 2);
                sprite.low <<= 1;
                sprite.high <<= 1;
                if sprite.valid && color != 0 && selected.is_none() {
                    selected = Some((color, sprite.attributes, sprite.zero));
                }
            }
        }
        if self.mask & 0x10 != 0
            && (x >= 8 || self.mask & 4 != 0)
            && let Some((color, attributes, zero)) = selected
        {
            if zero && pixel != 0 && x != 255 {
                self.status |= 0x40;
            }
            if pixel == 0 || attributes & 0x20 == 0 {
                index = 0x10 | ((attributes as usize & 3) << 2) | color as usize;
            }
        }
        let color = self.palette[index] & if self.mask & 1 != 0 { 0x30 } else { 0x3f };
        self.back[self.scanline as usize * WIDTH + x] =
            color as u16 | ((self.mask as u16 & 0xe0) << 1);
    }
    fn sprite_height(&self) -> u8 {
        if self.ctrl & 0x20 != 0 { 16 } else { 8 }
    }
    fn evaluate_sprites(&mut self) {
        if self.dot == 1 {
            self.secondary_count = 0;
            self.secondary_ids.fill(0xff);
            self.eval_n = self.oam_addr >> 2;
            self.eval_m = self.oam_addr & 3;
            self.eval_done = false;
            self.overflow_copy = 0;
        }
        if (1..=64).contains(&self.dot) {
            self.oam_bus = 0xff;
            if self.dot & 1 == 0 {
                self.secondary[self.dot as usize / 2 - 1] = 0xff;
            }
        } else if (65..=256).contains(&self.dot) {
            if self.dot & 1 != 0 {
                self.oam_bus = self.oam[(self.eval_n as usize * 4 + self.eval_m as usize) & 255];
                return;
            }
            if self.eval_done {
                self.eval_n = (self.eval_n + 1) & 63;
                return;
            }
            let in_range =
                self.scanline.wrapping_sub(self.oam_bus as u16) < self.sprite_height() as u16;
            if self.secondary_count < 8 {
                let destination = self.secondary_count as usize * 4 + self.eval_m as usize;
                self.secondary[destination] = self.oam_bus;
                if self.eval_m == 0 {
                    if in_range {
                        self.secondary_ids[self.secondary_count as usize] = self.eval_n;
                        self.eval_m = 1;
                    } else {
                        self.next_sprite();
                    }
                } else {
                    self.eval_m += 1;
                    if self.eval_m == 4 {
                        self.eval_m = 0;
                        self.secondary_count += 1;
                        self.next_sprite();
                    }
                }
            } else if self.overflow_copy != 0 {
                self.eval_m = (self.eval_m + 1) & 3;
                if self.eval_m == 0 {
                    self.next_sprite();
                }
                self.overflow_copy -= 1;
                if self.overflow_copy == 0 {
                    self.eval_m = 0;
                    self.eval_done = true;
                }
            } else if in_range {
                self.status |= 0x20;
                self.overflow_copy = 3;
                self.eval_m = (self.eval_m + 1) & 3;
                if self.eval_m == 0 {
                    self.next_sprite();
                }
            } else {
                // Hardware's diagonal overflow scan compares tile/attribute/X
                // bytes as Y after the first out-of-range ninth sprite.
                self.eval_m = (self.eval_m + 1) & 3;
                self.next_sprite();
            }
            self.oam_addr = self.eval_n * 4 + self.eval_m;
        } else if self.dot >= 321 || self.dot == 0 {
            self.oam_bus = self.secondary[0];
        }
    }
    fn next_sprite(&mut self) {
        self.eval_n = (self.eval_n + 1) & 63;
        if self.eval_n == 0 {
            self.eval_done = true;
        }
    }
    fn sprite_address(&self, slot: usize) -> u16 {
        let base = slot * 4;
        let tile = self.secondary[base + 1] as u16;
        let height = self.sprite_height();
        let mut row = (self.scanline as u8).wrapping_sub(self.secondary[base]) & (height - 1);
        if self.secondary[base + 2] & 0x80 != 0 {
            row ^= height - 1;
        }
        if height == 16 {
            ((tile & 1) << 12) | ((tile & 0xfe) << 4) | ((row as u16 & 8) << 1) | (row as u16 & 7)
        } else {
            ((self.ctrl as u16 & 8) << 9) | (tile << 4) | row as u16
        }
    }
    fn fetch_sprite(&mut self, cartridge: &Cartridge) -> Option<PpuFetch> {
        let slot = (self.dot as usize - 257) / 8;
        let phase = (self.dot - 257) & 7;
        let base = slot * 4;
        self.oam_bus = self.secondary[base + (phase as usize).min(3)];
        match phase {
            0 | 2 => self.fetch_address = 0x2000 | (self.v & 0x0fff),
            4 => self.fetch_address = self.sprite_address(slot),
            6 => self.fetch_address += 8,
            _ => {
                let value = if self.fetch_address < 0x2000 {
                    cartridge.render_chr(self.fetch_address, false, self.ctrl & 32 != 0, self.v)
                } else {
                    self.peek_memory(cartridge, self.fetch_address)
                };
                let bits = if self.secondary[base + 2] & 0x40 != 0 {
                    value.reverse_bits()
                } else {
                    value
                };
                if phase == 5 {
                    self.sprites[slot] = Sprite {
                        low: bits,
                        high: 0,
                        x: self.secondary[base + 3],
                        attributes: self.secondary[base + 2],
                        zero: self.secondary_ids[slot] == 0,
                        valid: slot < self.secondary_count as usize && self.scanline < 240,
                    };
                } else if phase == 7 {
                    self.sprites[slot].high = bits;
                }
                return Some(PpuFetch {
                    dot: self.total_dots,
                    address: self.fetch_address,
                    value,
                });
            }
        }
        None
    }
    fn increment_x(&mut self) {
        if self.v & 31 == 31 {
            self.v = (self.v & !31) ^ 0x0400;
        } else {
            self.v += 1;
        }
    }
    fn increment_y(&mut self) {
        if self.v & 0x7000 != 0x7000 {
            self.v += 0x1000;
            return;
        }
        self.v &= !0x7000;
        let y = (self.v >> 5) & 31;
        let next = match y {
            29 => {
                self.v ^= 0x0800;
                0
            }
            31 => 0,
            _ => y + 1,
        };
        self.v = (self.v & !0x03e0) | (next << 5);
    }
    fn nametable_index(&self, cartridge: &Cartridge, address: u16) -> usize {
        let offset = (address as usize - 0x2000) & 0xfff;
        let table = offset / 1024;
        let page = match cartridge.mirroring() {
            Mirroring::FourScreen => table,
            Mirroring::Horizontal => table / 2,
            Mirroring::Vertical => table & 1,
            Mirroring::OneScreenLower => 0,
            Mirroring::OneScreenUpper => 1,
        };
        page * 1024 + (offset & 1023)
    }
    fn palette_index(address: u16) -> usize {
        let index = address as usize & 31;
        if index & 0x13 == 0x10 {
            index - 16
        } else {
            index
        }
    }
    /// Side-effect-free physical PPU memory view, independent of CPU read buffering.
    pub fn peek_memory(&self, cartridge: &Cartridge, address: u16) -> u8 {
        let address = address & 0x3fff;
        match address {
            0..=0x1fff => cartridge.chr_read(address),
            0x2000..=0x3eff => cartridge
                .nametable_read(address, &self.nametables)
                .unwrap_or_else(|| self.nametables[self.nametable_index(cartridge, address)]),
            _ => self.palette[Self::palette_index(address)],
        }
    }
    fn write_memory(&mut self, cartridge: &mut Cartridge, address: u16, value: u8) {
        let address = address & 0x3fff;
        match address {
            0..=0x1fff => cartridge.chr_write(address, value),
            0x2000..=0x3eff => {
                if !cartridge.nametable_write(address, value, &mut self.nametables) {
                    let index = self.nametable_index(cartridge, address);
                    self.nametables[index] = value;
                }
            }
            _ => self.palette[Self::palette_index(address)] = value & 0x3f,
        }
    }
    fn increment_address(&mut self) {
        if self.rendering() && self.render_line() {
            self.increment_x();
            self.increment_y();
        } else {
            self.v = (self.v + if self.ctrl & 4 != 0 { 32 } else { 1 }) & 0x7fff;
        }
    }
    fn io_value(&self) -> u8 {
        let mut value = self.io_latch;
        for (bit, last) in self.io_refreshed.iter().enumerate() {
            if self.total_dots.saturating_sub(*last) >= IO_DECAY_DOTS {
                value &= !(1 << bit);
            }
        }
        value
    }
    fn refresh_io(&mut self, value: u8, mask: u8) {
        self.io_latch = (self.io_latch & !mask) | (value & mask);
        for (bit, last) in self.io_refreshed.iter_mut().enumerate() {
            if mask & (1 << bit) != 0 {
                *last = self.total_dots;
            }
        }
    }
    /// Value that a CPU read would return, without clearing status or advancing v.
    pub fn peek_register(&self, cartridge: &Cartridge, address: u16) -> u8 {
        match address & 7 {
            2 => (self.status & 0xe0) | (self.io_value() & 0x1f),
            4 if self.rendering() && self.render_line() => self.oam_bus,
            4 => self.oam[self.oam_addr as usize],
            7 if self.v & 0x3fff >= 0x3f00 => {
                let mask = if self.mask & 1 != 0 { 0x30 } else { 0x3f };
                (self.peek_memory(cartridge, self.v) & mask) | (self.io_value() & 0xc0)
            }
            7 => self.read_buffer,
            _ => self.io_value(),
        }
    }
    pub fn read_register(&mut self, cartridge: &Cartridge, address: u16) -> u8 {
        let value = self.peek_register(cartridge, address);
        let driven = match address & 7 {
            2 => 0xe0,
            4 => 0xff,
            7 if self.v & 0x3fff >= 0x3f00 => 0x3f,
            7 => 0xff,
            _ => 0,
        };
        match address & 7 {
            2 => {
                // Reads immediately before the vblank-set dot suppress this frame.
                if self.scanline() == 241 && self.dot() == 1 {
                    self.suppress_vblank = true;
                }
                self.status &= !0x80;
                self.second_write = false;
            }
            7 => {
                let addr = self.v & 0x3fff;
                self.read_buffer =
                    self.peek_memory(cartridge, if addr >= 0x3f00 { addr - 0x1000 } else { addr });
                self.increment_address();
            }
            _ => (),
        }
        self.refresh_io(value, driven);
        value
    }
    pub fn write_register(
        &mut self,
        cartridge: &mut Cartridge,
        address: u16,
        value: u8,
    ) -> Result<(), std::convert::Infallible> {
        self.refresh_io(value, 0xff);
        let register = address & 7;
        if self.total_dots
            < if self.pal {
                106_024
            } else {
                WRITE_INHIBIT_DOTS
            }
            && matches!(register, 0 | 1 | 5 | 6)
        {
            return Ok(());
        }
        match register {
            0 => {
                self.ctrl = value;
                self.t = (self.t & !0x0c00) | ((value as u16 & 3) << 10);
            }
            1 => {
                self.mask = (self.mask & 0x18) | (value & !0x18);
                self.next_mask = value;
                self.mask_delay = 2;
            }
            3 => self.oam_addr = value,
            4 if self.rendering() && self.render_line() => {
                self.oam_addr = self.oam_addr.wrapping_add(4);
            }
            4 => {
                // Attribute bits 2..4 do not exist in primary OAM.
                self.oam[self.oam_addr as usize] =
                    value & if self.oam_addr & 3 == 2 { 0xe3 } else { 0xff };
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                if self.second_write {
                    self.t = (self.t & !0x73e0)
                        | ((value as u16 & 7) << 12)
                        | ((value as u16 & 0xf8) << 2);
                } else {
                    self.t = (self.t & !0x001f) | (value as u16 >> 3);
                    self.fine_x = value & 7;
                }
                self.second_write = !self.second_write;
            }
            6 => {
                if self.second_write {
                    self.t = (self.t & 0x7f00) | value as u16;
                    self.v = self.t;
                } else {
                    self.t = (self.t & 0xff) | ((value as u16 & 0x3f) << 8);
                }
                self.second_write = !self.second_write;
            }
            7 => {
                self.write_memory(cartridge, self.v, value);
                self.increment_address();
            }
            _ => (), // PPUSTATUS writes only refresh the I/O latch.
        }
        Ok(())
    }
    pub(crate) fn valid_state(&self, cpu_cycles: u64) -> bool {
        let coordinate_clock = self
            .frame
            .checked_mul(if self.pal { 341 * 312 } else { DOTS_PER_FRAME })
            .and_then(|n| n.checked_add(self.scanline as u64 * 341 + self.dot as u64))
            .and_then(|n| n.checked_sub(self.skipped_dots));
        self.scanline <= self.pre_render()
            && self.dot < 341
            && self.skipped_dots <= self.frame / 2
            && coordinate_clock == Some(self.total_dots)
            && self.completed_frames
                == self.frame
                    + u64::from(self.scanline > 241 || (self.scanline == 241 && self.dot > 1))
            && self.front.len() == WIDTH * HEIGHT
            && self.back.len() == WIDTH * HEIGHT
            && self
                .front
                .iter()
                .chain(&self.back)
                .all(|pixel| *pixel < 512)
            && self.tile_attribute < 4
            && self.fetch_address < 0x4000
            && self.nametables.len() == 4096
            && self.oam.len() == 256
            && self.palette.iter().all(|v| *v < 64)
            && self.oam.iter().skip(2).step_by(4).all(|v| v & 0x1c == 0)
            && self.v < 0x8000
            && self.t < 0x8000
            && self.fine_x < 8
            && self.mask_delay <= 3
            && self.secondary_count <= 8
            && self.secondary_ids.iter().all(|id| *id < 64 || *id == 255)
            && self.eval_n < 64
            && self.eval_m < 4
            && self.overflow_copy <= 3
            && self
                .io_refreshed
                .iter()
                .all(|last| *last <= self.total_dots)
            && self.status & 0x1f == 0
            && cpu_cycles
                .checked_mul(if self.pal { 16 } else { 3 })
                .map(|n| if self.pal { n / 5 } else { n })
                == Some(self.total_dots)
            && (!self.suppress_vblank || (self.scanline() == 241 && self.dot() <= 1))
    }
}
