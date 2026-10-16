use super::MapperState;
impl MapperState {
    pub(in crate::cartridge) fn write_fme7(
        &mut self,
        address: u16,
        value: u8,
        ram: &mut [u8],
    ) -> bool {
        if let MapperState::Fme7 {
            command,
            banks,
            mirror,
            control,
            counter,
            pending,
        } = self
        {
            match address {
                0x6000..=0x7fff if banks[8] & 0xc0 == 0xc0 => ram[address as usize & 8191] = value,
                0x8000..=0x9fff => *command = value & 15,
                0xa000..=0xbfff => match *command {
                    0..=11 => banks[*command as usize] = value,
                    12 => *mirror = value & 3,
                    13 => {
                        *control = value;
                        *pending = false;
                    }
                    14 => *counter = (*counter & 0xff00) | value as u16,
                    15 => *counter = (*counter & 255) | ((value as u16) << 8),
                    _ => unreachable!(),
                },
                _ => (),
            }
            return true;
        }
        false
    }
}
