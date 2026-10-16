use super::MapperState;

impl MapperState {
    pub(in crate::cartridge) fn write_mmc3(&mut self, address: u16, value: u8, mmc6: bool) {
        let MapperState::Mmc3 {
            select,
            banks,
            horizontal,
            ram_protect,
            irq_latch,
            irq_counter,
            irq_reload,
            irq_enabled,
            irq_pending,
            ..
        } = self
        else {
            unreachable!("mapper dispatch");
        };
        match address & 0xe001 {
            0x8000 => {
                *select = value;
                if mmc6 && value & 32 == 0 {
                    *ram_protect = 0;
                }
            }
            0x8001 => banks[(*select & 7) as usize] = value,
            0xa000 => *horizontal = value & 1 != 0,
            0xa001 => {
                if !mmc6 || *select & 32 != 0 {
                    *ram_protect = value;
                }
            }
            0xc000 => *irq_latch = value,
            0xc001 => {
                *irq_counter = 0;
                *irq_reload = true;
            }
            0xe000 => {
                *irq_enabled = false;
                *irq_pending = false;
            }
            0xe001 => *irq_enabled = true,
            _ => unreachable!(),
        }
    }
    pub(in crate::cartridge) fn observe_mmc3(&mut self, address: u16, dot: u64) {
        if let MapperState::Mmc3 {
            alternate,
            a12,
            low_since,
            irq_latch,
            irq_counter,
            irq_reload,
            irq_enabled,
            irq_pending,
            ..
        } = self
        {
            let high = address & 0x1000 != 0;
            if !high && *a12 {
                *low_since = dot;
            }
            if high && !*a12 && dot.saturating_sub(*low_since) > 9 {
                let trigger = !*alternate || *irq_counter != 0 || *irq_reload;
                if *irq_counter == 0 || *irq_reload {
                    *irq_counter = *irq_latch;
                } else {
                    *irq_counter -= 1;
                }
                *irq_reload = false;
                if *irq_counter == 0 && *irq_enabled && trigger {
                    *irq_pending = true;
                }
            }
            *a12 = high;
        }
    }
}
