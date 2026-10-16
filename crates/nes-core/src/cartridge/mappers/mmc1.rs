use super::MapperState;

impl MapperState {
    pub(in crate::cartridge) fn write_mmc1(&mut self, address: u16, value: u8, cycle: u64) {
        let MapperState::Mmc1 {
            shift,
            control,
            chr0,
            chr1,
            prg,
            last_write,
        } = self
        else {
            unreachable!("mapper dispatch");
        };
        {
            if last_write.is_some_and(|last| cycle == last.wrapping_add(1)) {
                return;
            }
            *last_write = Some(cycle);
            if value & 0x80 != 0 {
                *shift = 0x10;
                *control |= 0x0c;
                return;
            }
            let complete = *shift & 1 != 0;
            *shift = (*shift >> 1) | ((value & 1) << 4);
            if complete {
                match (address >> 13) & 3 {
                    0 => *control = *shift,
                    1 => *chr0 = *shift,
                    2 => *chr1 = *shift,
                    _ => *prg = *shift,
                }
                *shift = 0x10;
            }
        }
    }
}
