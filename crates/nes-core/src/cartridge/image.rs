//! Container parsing only. Board implementation and corrections are separate.
//! NES 2.0 field reference: NESdev format and Mesen2 Core/NES/NesHeader.cpp.
use super::CartridgeError;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeaderVersion {
    Ines,
    Nes2,
}
#[derive(Debug, Clone)]
pub struct Identity {
    pub file_sha256: String,
    pub prg_sha256: String,
    pub chr_sha256: String,
    pub header: HeaderVersion,
    pub submapper: u8,
}
pub(super) struct Parsed {
    pub header: [u8; 16],
    pub version: HeaderVersion,
    pub submapper: u8,
    pub prg_ram: Option<usize>,
}
pub(super) fn parse(bytes: &[u8]) -> Result<Parsed, CartridgeError> {
    let bad = |s: &str| CartridgeError(s.into());
    if bytes.len() < 16 || &bytes[..4] != b"NES\x1a" {
        return Err(bad("not an iNES image"));
    }
    let mut h: [u8; 16] = bytes[..16].try_into().unwrap();
    if &h[7..] == b"DiskDude!" {
        h[7..].fill(0);
    }
    if h[7] & 3 != 0 {
        return Err(bad("unsupported console variant"));
    }
    let version = match h[7] & 12 {
        0 => HeaderVersion::Ines,
        8 => HeaderVersion::Nes2,
        _ => return Err(bad("unrecognized header format")),
    };
    let mut prg_ram = None;
    let submapper = if version == HeaderVersion::Nes2 {
        h[8] >> 4
    } else {
        0
    };
    if version == HeaderVersion::Nes2 {
        // Accept only configurations implemented by our existing boards. Never
        // discard high mapper/size bits or silently guess a submapper's hardware.
        if h[8] != 0 || h[9] != 0 {
            return Err(bad(
                "NES 2.0 submapper or extended ROM size is not implemented",
            ));
        }
        if h[12] & 3 == 3 || h[13] != 0 || h[14] != 0 || h[15] != 0 {
            return Err(bad(
                "NES 2.0 timing, miscellaneous ROM or peripheral variant is not implemented",
            ));
        }
        let ram = match h[10] {
            0 => 0,
            7 | 0x70 => 8192,
            _ => return Err(bad("NES 2.0 PRG RAM layout is not implemented")),
        };
        if (h[10] == 0x70) != (h[6] & 2 != 0) {
            return Err(bad("inconsistent NES 2.0 battery RAM flags"));
        }
        let mapper = (h[6] >> 4) | (h[7] & 0xf0);
        if matches!(mapper, 5 | 69) && ram != 8192 {
            return Err(bad("this board needs its implemented 8 KiB RAM layout"));
        }
        if h[6] & 4 != 0 && ram < 8192 {
            return Err(bad("trainer requires 8 KiB PRG RAM"));
        }
        if h[11] != if h[5] == 0 { 7 } else { 0 } {
            return Err(bad("NES 2.0 CHR RAM layout is not implemented"));
        }
        prg_ram = Some(ram);
        let pal = h[12] & 3 == 1; // dual-region images default to NTSC
        h[7] &= !12;
        h[8] = u8::from(ram != 0);
        h[9] = u8::from(pal);
    } else if h[8] > 1 {
        return Err(bad("PRG RAM larger than 8 KiB is not implemented"));
    }
    let expected =
        16 + usize::from(h[6] & 4 != 0) * 512 + h[4] as usize * 16384 + h[5] as usize * 8192;
    if bytes.len() != expected {
        return Err(bad("truncated image or unexpected trailing data"));
    }
    Ok(Parsed {
        header: h,
        version,
        submapper,
        prg_ram,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cartridge::Cartridge, diagnostic::controller_rom};
    #[test]
    fn equivalent_containers_keep_distinct_file_identity_and_same_program_identity() {
        let legacy = controller_rom();
        let mut nes2 = legacy.clone();
        nes2[7] |= 8;
        nes2[10] = 7;
        nes2[11] = 7;
        let a = Cartridge::from_ines(&legacy).unwrap();
        let b = Cartridge::from_ines(&nes2).unwrap();
        assert_ne!(a.identity().file_sha256, b.identity().file_sha256);
        assert_eq!(a.identity().prg_sha256, b.identity().prg_sha256);
        assert_eq!(b.identity().header, HeaderVersion::Nes2);
        for index in [8, 9, 11, 13, 14, 15] {
            let mut invalid = nes2.clone();
            invalid[index] = 0xff;
            assert!(Cartridge::from_ines(&invalid).is_err());
        }
        nes2[10] = 0;
        let c = Cartridge::from_ines(&nes2).unwrap();
        assert_eq!(c.cpu_read(0x6000), None);
    }
}
