//! Mapper register state retains its schema-6 serialization shape.
use serde::{Deserialize, Serialize};
mod fme7;
mod mmc1;
mod mmc3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum MapperState {
    Nrom,
    Mmc5(Box<crate::mmc5::Mmc5>),
    Fme7 {
        command: u8,
        banks: [u8; 12],
        mirror: u8,
        control: u8,
        counter: u16,
        pending: bool,
    },
    Cnrom {
        chr: u8,
    },
    Axrom {
        bank: u8,
    },
    Mmc2 {
        prg: u8,
        chr: [u8; 4],
        latch_fe: [bool; 2],
        horizontal: bool,
    },
    Mmc3 {
        alternate: bool,
        select: u8,
        banks: [u8; 8],
        horizontal: bool,
        ram_protect: u8,
        irq_latch: u8,
        irq_counter: u8,
        irq_reload: bool,
        irq_enabled: bool,
        irq_pending: bool,
        a12: bool,
        low_since: u64,
    },
    Uxrom {
        bank: u8,
    },
    Mmc1 {
        shift: u8,
        control: u8,
        chr0: u8,
        chr1: u8,
        prg: u8,
        last_write: Option<u64>,
    },
    // Append variants to preserve serialized discriminants of existing saves.
    Rambo1(crate::rambo1::Rambo1),
    Camerica {
        bank: u8,
        single_screen: Option<bool>,
    },
}
