//! Cartridge CPU/PPU bus routing and ROM/RAM address translation.
//! Container parsing and board identification deliberately do not run here.
use super::{Cartridge, CartridgeError, MapperState, Mirroring};

impl Cartridge {
    pub fn mirroring(&self) -> Mirroring {
        if let MapperState::Camerica {
            single_screen: Some(upper),
            ..
        } = self.state.mapper
        {
            return if upper {
                Mirroring::OneScreenUpper
            } else {
                Mirroring::OneScreenLower
            };
        }
        if let MapperState::Rambo1(ref m) = self.state.mapper {
            return if m.horizontal {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
        if self.header_mirroring == Mirroring::FourScreen {
            return Mirroring::FourScreen;
        }
        if let MapperState::Mmc3 { horizontal, .. } | MapperState::Mmc2 { horizontal, .. } =
            self.state.mapper
        {
            return if horizontal {
                Mirroring::Horizontal
            } else {
                Mirroring::Vertical
            };
        }
        if let MapperState::Fme7 { mirror, .. } = self.state.mapper {
            return match mirror {
                0 => Mirroring::Vertical,
                1 => Mirroring::Horizontal,
                2 => Mirroring::OneScreenLower,
                _ => Mirroring::OneScreenUpper,
            };
        }
        if let MapperState::Axrom { bank } = self.state.mapper {
            return if bank & 16 == 0 {
                Mirroring::OneScreenLower
            } else {
                Mirroring::OneScreenUpper
            };
        }
        if let MapperState::Mmc1 { control, .. } = self.state.mapper {
            match control & 3 {
                0 => Mirroring::OneScreenLower,
                1 => Mirroring::OneScreenUpper,
                2 => Mirroring::Vertical,
                _ => Mirroring::Horizontal,
            }
        } else {
            self.header_mirroring
        }
    }

    fn ram_enabled(&self) -> bool {
        !self.state.prg_ram.is_empty()
            && !matches!(
                self.state.mapper,
                MapperState::Mmc2 { .. }
                    | MapperState::Cnrom { .. }
                    | MapperState::Axrom { .. }
                    | MapperState::Rambo1(_)
                    | MapperState::Camerica { .. }
            )
            && !matches!(self.state.mapper, MapperState::Mmc1 { prg, .. } if prg & 0x10 != 0)
            && !matches!(self.state.mapper,MapperState::Mmc3{ram_protect,..} if ram_protect&0x80==0)
    }
    fn prg_index(&self, address: u16) -> usize {
        if let MapperState::Rambo1(ref m) = self.state.mapper {
            return m.prg_bank(address, self.prg.len() / 8192) * 8192 + (address as usize & 8191);
        }
        if let MapperState::Mmc5(ref m) = self.state.mapper {
            return (m.prg_bank(address).1 * 8192 + (address as usize & 8191)) % self.prg.len();
        }
        if let MapperState::Fme7 { banks, .. } = self.state.mapper {
            let slot = (address as usize - 0x6000) / 8192;
            let bank = if slot == 4 {
                self.prg.len() / 8192 - 1
            } else {
                (banks[8 + slot] & 63) as usize
            };
            return (bank * 8192 + (address as usize & 8191)) % self.prg.len();
        }
        if let MapperState::Mmc2 { prg, .. } = self.state.mapper {
            let total = self.prg.len() / 8192;
            let slot = (address as usize - 0x8000) / 8192;
            let bank = if slot == 0 {
                prg as usize % total
            } else {
                total - 4 + slot
            };
            return bank * 8192 + (address as usize & 8191);
        }
        if let MapperState::Mmc3 { select, banks, .. } = self.state.mapper {
            let total = self.prg.len() / 8192;
            let bank = match (address - 0x8000) / 8192 {
                0 if select & 0x40 != 0 => total - 2,
                0 => banks[6] as usize & 63,
                1 => banks[7] as usize & 63,
                2 if select & 0x40 != 0 => banks[6] as usize & 63,
                2 => total - 2,
                _ => total - 1,
            };
            return bank % total * 8192 + (address as usize & 8191);
        }
        let banks = self.prg.len() / 16384;
        let offset = address as usize & 0x3fff;
        let bank = match self.state.mapper {
            MapperState::Mmc3 { .. }
            | MapperState::Mmc2 { .. }
            | MapperState::Mmc5(_)
            | MapperState::Rambo1(_)
            | MapperState::Fme7 { .. } => unreachable!(),
            MapperState::Axrom { bank } => (bank as usize & 7) * 2 + usize::from(address >= 0xc000),
            MapperState::Nrom | MapperState::Cnrom { .. } => (address as usize - 0x8000) / 16384,
            MapperState::Uxrom { bank } | MapperState::Camerica { bank, .. } => {
                if address < 0xc000 {
                    bank as usize
                } else {
                    banks - 1
                }
            }
            MapperState::Mmc1 {
                control, prg, chr0, ..
            } => {
                // SUROM uses CHR A16 as PRG A18. Fixed banks remain inside
                // the selected 256 KiB region, not the end of the full ROM.
                let outer = if banks > 16 { chr0 as usize & 16 } else { 0 };
                outer
                    + match (control >> 2) & 3 {
                        0 | 1 => (prg as usize & 0x0e) + usize::from(address >= 0xc000),
                        2 => {
                            if address < 0xc000 {
                                0
                            } else {
                                prg as usize & 0x0f
                            }
                        }
                        _ => {
                            if address < 0xc000 {
                                prg as usize & 0x0f
                            } else {
                                banks.min(16) - 1
                            }
                        }
                    }
            }
        };
        (bank % banks) * 16384 + offset
    }
    pub fn cpu_read(&self, address: u16) -> Option<u8> {
        if self.mmc6
            && (0x6000..0x8000).contains(&address)
            && let MapperState::Mmc3 {
                select,
                ram_protect,
                ..
            } = self.state.mapper
        {
            if address < 0x7000 || select & 32 == 0 || ram_protect & 0xa0 == 0 {
                return None;
            }
            let read = if address & 512 == 0 { 32 } else { 128 };
            return Some(if ram_protect & read != 0 {
                self.state.prg_ram[address as usize & 1023]
            } else {
                0
            });
        }
        if let MapperState::Mmc5(ref m) = self.state.mapper {
            if address < 0x6000 {
                return m.read(address);
            }
            let (rom, bank) = m.prg_bank(address);
            return Some(if rom {
                self.prg[self.prg_index(address)]
            } else {
                self.state.prg_ram
                    [(bank * 8192 + (address as usize & 8191)) % self.state.prg_ram.len()]
            });
        }
        if let MapperState::Fme7 { banks, .. } = self.state.mapper
            && (0x6000..0x8000).contains(&address)
        {
            return if banks[8] & 64 == 0 {
                Some(self.prg[self.prg_index(address)])
            } else if banks[8] & 128 != 0 {
                Some(self.state.prg_ram[address as usize & 8191])
            } else {
                None
            };
        }
        match address {
            0x6000..=0x7fff if self.ram_enabled() => {
                Some(self.state.prg_ram[address as usize - 0x6000])
            }
            0x8000..=0xffff => Some(self.prg[self.prg_index(address)]),
            _ => None,
        }
    }
    pub fn cpu_write(&mut self, address: u16, value: u8, cycle: u64) {
        if self.mmc6 && (0x6000..0x8000).contains(&address) {
            if let MapperState::Mmc3 {
                select,
                ram_protect,
                ..
            } = self.state.mapper
            {
                let mask = if address & 512 == 0 { 0x30 } else { 0xc0 };
                if address >= 0x7000 && select & 32 != 0 && ram_protect & mask == mask {
                    self.state.prg_ram[address as usize & 1023] = value;
                }
            }
            return;
        }
        if let MapperState::Mmc5(ref mut m) = self.state.mapper {
            if address >= 0x6000 {
                let (rom, bank) = m.prg_bank(address);
                if !rom && m.protect == [2, 1] {
                    let len = self.state.prg_ram.len();
                    self.state.prg_ram[(bank * 8192 + (address as usize & 8191)) % len] = value;
                }
            } else {
                m.write(address, value);
            }
            return;
        }
        if self
            .state
            .mapper
            .write_fme7(address, value, &mut self.state.prg_ram)
        {
            return;
        }
        if (0x6000..=0x7fff).contains(&address) {
            if self.ram_enabled()
                && !matches!(self.state.mapper,MapperState::Mmc3{ram_protect,..} if ram_protect&0x40!=0)
            {
                self.state.prg_ram[address as usize - 0x6000] = value;
            }
            return;
        }
        if address < 0x8000 {
            return;
        }
        let rom = self.prg[self.prg_index(address)];
        match &mut self.state.mapper {
            MapperState::Rambo1(m) => m.write(address, value),
            MapperState::Camerica {
                bank,
                single_screen,
            } => match address {
                0x9000..=0x9fff => *single_screen = Some(value & 16 != 0),
                0xc000..=0xffff => *bank = value & 15,
                _ => (),
            },
            MapperState::Mmc2 {
                prg,
                chr,
                horizontal,
                ..
            } => match address >> 12 {
                0xa => *prg = value & 15,
                0xb..=0xe => chr[(address >> 12) as usize - 0xb] = value & 31,
                0xf => *horizontal = value & 1 != 0,
                _ => (),
            },
            m @ MapperState::Mmc3 { .. } => m.write_mmc3(address, value, self.mmc6),
            MapperState::Mmc5(_) | MapperState::Fme7 { .. } => unreachable!(),
            MapperState::Nrom => (),
            MapperState::Cnrom { chr } => *chr = value & rom & 3,
            // iNES mapper 7 does not distinguish AMROM/ANROM. The standard
            // compatibility convention is no conflicts (NESdev AxROM).
            MapperState::Axrom { bank } => *bank = value & 0x17,
            // Standard discrete UxROM boards have bus conflicts.
            MapperState::Uxrom { bank } => *bank = value & rom & 0x0f,
            m @ MapperState::Mmc1 { .. } => m.write_mmc1(address, value, cycle),
        }
    }
    fn chr_index(&self, address: u16) -> usize {
        if let MapperState::Rambo1(ref m) = self.state.mapper {
            return (m.chr_bank(address) * 1024 + (address as usize & 1023)) % self.chr.len();
        }
        let addr = address as usize & 0x1fff;
        let size = if self.chr.is_empty() {
            self.state.chr_ram.len()
        } else {
            self.chr.len()
        };
        if let MapperState::Mmc5(ref m) = self.state.mapper {
            return m.chr_index(address, false, false, 0) % size;
        }
        if let MapperState::Fme7 { banks, .. } = self.state.mapper {
            return (banks[addr / 1024] as usize * 1024 + (addr & 1023)) % size;
        }
        if let MapperState::Cnrom { chr } = self.state.mapper {
            return (chr as usize * 8192 + addr) % size;
        }
        if let MapperState::Mmc2 { chr, latch_fe, .. } = self.state.mapper {
            let half = addr / 4096;
            let bank = chr[half * 2 + usize::from(latch_fe[half])];
            return (bank as usize * 4096 + (addr & 4095)) % size;
        }
        if let MapperState::Mmc3 { select, banks, .. } = self.state.mapper {
            let slot = (addr ^ if select & 0x80 != 0 { 0x1000 } else { 0 }) / 1024;
            let bank = match slot {
                0 => banks[0] & 0xfe,
                1 => banks[0] | 1,
                2 => banks[1] & 0xfe,
                3 => banks[1] | 1,
                _ => banks[slot - 2],
            };
            return (bank as usize * 1024 + (addr & 1023)) % size;
        }
        let index = if let MapperState::Mmc1 {
            control,
            chr0,
            chr1,
            ..
        } = self.state.mapper
        {
            if control & 0x10 != 0 {
                (if addr < 0x1000 { chr0 } else { chr1 }) as usize * 4096 + (addr & 0xfff)
            } else {
                (chr0 as usize & 0x1e) * 4096 + addr
            }
        } else {
            addr
        };
        index % size
    }
    pub fn chr_read(&self, address: u16) -> u8 {
        let index = self.chr_index(address);
        if self.chr.is_empty() {
            self.state.chr_ram[index]
        } else {
            self.chr[index]
        }
    }
    pub fn chr_write(&mut self, address: u16, value: u8) {
        if self.chr.is_empty() {
            let index = self.chr_index(address);
            self.state.chr_ram[index] = value;
        }
    }
    /// MMC2 latches change AFTER the triggering read; debugger peeks and writes
    /// must not switch CHR banks. Left-table triggers are single addresses.
    pub fn ppu_read_complete(&mut self, address: u16) {
        if let MapperState::Mmc5(ref mut m) = self.state.mapper {
            m.ppu_read(address);
        }
        if let MapperState::Mmc2 { latch_fe, .. } = &mut self.state.mapper {
            match address {
                0x0fd8 => latch_fe[0] = false,
                0x0fe8 => latch_fe[0] = true,
                0x1fd8..=0x1fdf => latch_fe[1] = false,
                0x1fe8..=0x1fef => latch_fe[1] = true,
                _ => (),
            }
        }
    }
    /// Observe the real PPU address bus, including dummy fetches and CPU VRAM access.
    /// Debugger peeks intentionally never call this method.
    pub fn observe_ppu(&mut self, address: u16, dot: u64) {
        if let MapperState::Rambo1(ref mut m) = self.state.mapper {
            m.observe_ppu(address, dot);
        }
        self.state.mapper.observe_mmc3(address, dot);
    }
    /// iNES does not distinguish MMC3 IRQ revisions; default is Sharp behavior.
    pub fn set_mmc3_alternate(&mut self, enabled: bool) -> Result<(), CartridgeError> {
        if let MapperState::Mmc3 { alternate, .. } = &mut self.state.mapper {
            *alternate = enabled;
            Ok(())
        } else {
            Err(CartridgeError(
                "MMC3 revision option requires mapper 4".into(),
            ))
        }
    }
    pub fn clock_cpu(&mut self) {
        match &mut self.state.mapper {
            MapperState::Rambo1(m) => m.clock_cpu(),
            MapperState::Mmc5(m) => m.clock(),
            MapperState::Fme7 {
                control,
                counter,
                pending,
                ..
            } if *control & 128 != 0 => {
                let old = *counter;
                *counter = counter.wrapping_sub(1);
                if old == 0 && *control & 1 != 0 {
                    *pending = true;
                }
            }
            _ => (),
        }
    }
    pub fn cpu_read_complete(&mut self, a: u16) {
        if let MapperState::Mmc5(ref mut m) = self.state.mapper {
            if a == 0x5204 {
                m.pending = false;
            }
            if a == 0xfffa || a == 0xfffb {
                m.reset_frame();
            }
        }
    }
    pub fn nametable_read(&self, a: u16, ram: &[u8]) -> Option<u8> {
        if let MapperState::Mmc5(ref m) = self.state.mapper {
            let i = a as usize & 1023;
            return Some(match (m.nt >> (((a >> 10) & 3) * 2)) & 3 {
                0 => ram[i],
                1 => ram[1024 + i],
                2 => {
                    if m.ex_mode < 2 {
                        m.exram[i]
                    } else {
                        0
                    }
                }
                _ => {
                    if i < 960 {
                        m.fill[0]
                    } else {
                        m.fill[1] * 0x55
                    }
                }
            });
        }
        None
    }
    pub fn nametable_write(&mut self, a: u16, v: u8, ram: &mut [u8]) -> bool {
        if let MapperState::Mmc5(ref mut m) = self.state.mapper {
            let i = a as usize & 1023;
            match (m.nt >> (((a >> 10) & 3) * 2)) & 3 {
                0 => ram[i] = v,
                1 => ram[1024 + i] = v,
                2 if m.ex_mode < 2 => m.exram[i] = v,
                _ => (),
            };
            return true;
        }
        false
    }
    pub fn render_chr(&self, a: u16, bg: bool, sprites16: bool, v: u16) -> u8 {
        if let MapperState::Mmc5(ref m) = self.state.mapper {
            return self.chr[m.chr_index(a, bg, sprites16, v) % self.chr.len()];
        }
        self.chr_read(a)
    }
    pub fn extended_attribute(&self, v: u16) -> Option<u8> {
        if let MapperState::Mmc5(ref m) = self.state.mapper
            && m.ex_mode == 1
        {
            Some(m.exram[(v & 1023) as usize] >> 6)
        } else {
            None
        }
    }
    pub fn irq(&self) -> bool {
        if let MapperState::Rambo1(ref m) = self.state.mapper {
            return m.pending;
        }
        if let MapperState::Mmc5(ref m) = self.state.mapper {
            return m.pending && m.irq_enable;
        }
        if let MapperState::Fme7 { pending, .. } = self.state.mapper {
            return pending;
        }
        matches!(
            self.state.mapper,
            MapperState::Mmc3 {
                irq_pending: true,
                ..
            }
        )
    }
}
