use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    OneScreenLower,
    OneScreenUpper,
    FourScreen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeError(pub String);
impl fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for CartridgeError {}

use mappers::MapperState;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CartridgeState {
    pub mapper: MapperState,
    pub prg_ram: Vec<u8>,
    pub chr_ram: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Cartridge {
    prg: Vec<u8>,
    chr: Vec<u8>,
    header_mirroring: Mirroring,
    digest: [u8; 32],
    identity: image::Identity,
    mapper_id: u8,
    battery: bool,
    pal: bool,
    mmc6: bool,
    zapper: bool,
    pub(crate) state: CartridgeState,
}

mod boards;
mod bus;
pub mod image;
mod loader;
mod mappers;

impl Cartridge {
    pub fn has_zapper(&self) -> bool {
        self.zapper
    }
    pub fn is_pal(&self) -> bool {
        self.pal
    }
    pub fn battery_ram(&self) -> Option<&[u8]> {
        self.battery.then_some(&self.state.prg_ram)
    }
    pub fn restore_battery_ram(&mut self, bytes: &[u8]) -> Result<(), CartridgeError> {
        if !self.battery || bytes.len() != self.state.prg_ram.len() {
            return Err(CartridgeError(
                "battery save does not match this cartridge's RAM size".into(),
            ));
        }
        self.state.prg_ram.copy_from_slice(bytes);
        Ok(())
    }
    pub fn identity(&self) -> &image::Identity {
        &self.identity
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn mapper_id(&self) -> u8 {
        self.mapper_id
    }

    pub(crate) fn valid_state(&self, state: &CartridgeState, cpu_cycle: u64) -> bool {
        if state.prg_ram.len() != self.state.prg_ram.len()
            || state.chr_ram.len() != self.state.chr_ram.len()
        {
            return false;
        }
        match (&self.state.mapper, &state.mapper) {
            (MapperState::Rambo1(_), MapperState::Rambo1(m)) => cpu_cycle
                .checked_mul(if self.pal { 16 } else { 3 })
                .is_some_and(|dots| m.valid(if self.pal { dots / 5 } else { dots })),
            (MapperState::Camerica { .. }, MapperState::Camerica { bank, .. }) => *bank < 16,
            (MapperState::Nrom, MapperState::Nrom) => true,
            (MapperState::Mmc5(_), MapperState::Mmc5(m)) => m.valid(),
            (
                MapperState::Fme7 { .. },
                MapperState::Fme7 {
                    command, mirror, ..
                },
            ) => *command < 16 && *mirror < 4,
            (MapperState::Cnrom { .. }, MapperState::Cnrom { chr }) => *chr < 4,
            (MapperState::Axrom { .. }, MapperState::Axrom { bank }) => *bank & !0x17 == 0,
            (MapperState::Mmc2 { .. }, MapperState::Mmc2 { prg, chr, .. }) => {
                *prg < 16 && chr.iter().all(|b| *b < 32)
            }
            (
                MapperState::Mmc3 {
                    alternate: original,
                    ..
                },
                MapperState::Mmc3 {
                    alternate,
                    low_since,
                    ..
                },
            ) => {
                original == alternate
                    && cpu_cycle
                        .checked_mul(if self.pal { 16 } else { 3 })
                        .map(|dots| if self.pal { dots / 5 } else { dots })
                        .is_some_and(|dots| *low_since <= dots)
            }
            (MapperState::Uxrom { .. }, MapperState::Uxrom { bank }) => *bank < 16,
            (
                MapperState::Mmc1 { .. },
                MapperState::Mmc1 {
                    shift,
                    control,
                    chr0,
                    chr1,
                    prg,
                    last_write,
                },
            ) => {
                (1..=31).contains(shift)
                    && [control, chr0, chr1, prg].iter().all(|v| **v < 32)
                    && last_write.is_none_or(|last| last < cpu_cycle)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod mmc6_tests {
    use super::*;
    #[test]
    fn mmc6_half_permissions_mirroring_and_gate() {
        let mut r = vec![0; 16 + 32768 + 8192];
        r[..4].copy_from_slice(b"NES\x1a");
        r[4] = 2;
        r[5] = 1;
        r[6] = 0x40;
        let mut c = Cartridge::from_ines(&r).unwrap();
        c.mmc6 = true;
        c.state.prg_ram = vec![0; 1024];
        c.cpu_write(0x8000, 0, 0);
        c.cpu_write(0xa001, 0xf0, 0);
        assert_eq!(c.cpu_read(0x7000), None);
        c.cpu_write(0x8000, 32, 0);
        c.cpu_write(0xa001, 0x30, 0);
        c.cpu_write(0x7000, 42, 0);
        c.cpu_write(0x7200, 99, 0);
        assert_eq!(c.cpu_read(0x7400), Some(42));
        assert_eq!(c.cpu_read(0x7200), Some(0));
        assert_eq!(c.cpu_read(0x6000), None);
        c.cpu_write(0xa001, 0xf0, 0);
        c.cpu_write(0x7e00, 99, 0);
        assert_eq!(c.cpu_read(0x7200), Some(99));
        c.cpu_write(0xa001, 0xa0, 0);
        c.cpu_write(0x7000, 11, 0);
        assert_eq!(c.cpu_read(0x7000), Some(42));
        c.cpu_write(0x8000, 0, 0);
        c.cpu_write(0x8000, 32, 0);
        assert_eq!(c.cpu_read(0x7000), None);
    }
}
