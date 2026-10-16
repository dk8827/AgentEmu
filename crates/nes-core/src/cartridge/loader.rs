use super::*;
impl Cartridge {
    /// Load explicitly supported iNES and NES 2.0 board configurations.
    /// Unknown submappers and RAM/ROM layouts are rejected rather than guessed.
    pub fn from_ines(original: &[u8]) -> Result<Self, CartridgeError> {
        let parsed = image::parse(original)?;
        let header = parsed.header;
        let bytes = original;
        let flags7 = header[7];
        let fail = |message: &str| CartridgeError(message.into());
        let mut mapper_id = (header[6] >> 4) | (flags7 & 0xf0);
        let (prg_banks, chr_banks) = (header[4] as usize, header[5] as usize);
        let mut mapper = match mapper_id {
            64 if prg_banks.is_power_of_two()
                && (2..=16).contains(&prg_banks)
                && chr_banks.is_power_of_two()
                && chr_banks <= 32 =>
            {
                MapperState::Rambo1(crate::rambo1::Rambo1::new(header[6] & 1 == 0))
            }
            71 if prg_banks.is_power_of_two()
                && (2..=16).contains(&prg_banks)
                && chr_banks == 0 =>
            {
                MapperState::Camerica {
                    bank: 0,
                    single_screen: None,
                }
            }
            0 if (1..=2).contains(&prg_banks) && chr_banks <= 1 => MapperState::Nrom,
            5 if prg_banks.is_power_of_two()
                && (2..=32).contains(&prg_banks)
                && chr_banks.is_power_of_two()
                && chr_banks <= 128 =>
            {
                MapperState::Mmc5(Box::default())
            }
            69 if prg_banks.is_power_of_two()
                && (2..=32).contains(&prg_banks)
                && chr_banks.is_power_of_two()
                && chr_banks <= 32 =>
            {
                MapperState::Fme7 {
                    command: 0,
                    banks: [0; 12],
                    mirror: 0,
                    control: 0,
                    counter: 0,
                    pending: false,
                }
            }
            3 if (1..=2).contains(&prg_banks) && chr_banks.is_power_of_two() && chr_banks <= 4 => {
                MapperState::Cnrom { chr: 0 }
            }
            7 if prg_banks.is_power_of_two() && (2..=16).contains(&prg_banks) && chr_banks == 0 => {
                MapperState::Axrom { bank: 0 }
            }
            9 if prg_banks.is_power_of_two()
                && (2..=8).contains(&prg_banks)
                && chr_banks.is_power_of_two()
                && chr_banks <= 16 =>
            {
                MapperState::Mmc2 {
                    prg: 0,
                    chr: [0; 4],
                    latch_fe: [true; 2],
                    horizontal: header[6] & 1 == 0,
                }
            }
            4 if prg_banks.is_power_of_two()
                && (2..=32).contains(&prg_banks)
                && (chr_banks == 0 || (chr_banks.is_power_of_two() && chr_banks <= 32)) =>
            {
                MapperState::Mmc3 {
                    alternate: false,
                    select: 0,
                    banks: [0; 8],
                    horizontal: header[6] & 1 == 0,
                    ram_protect: 0x80,
                    irq_latch: 0,
                    irq_counter: 0,
                    irq_reload: false,
                    irq_enabled: false,
                    irq_pending: false,
                    a12: false,
                    low_since: 0,
                }
            }
            2 if prg_banks.is_power_of_two() && (2..=16).contains(&prg_banks) && chr_banks == 0 => {
                MapperState::Uxrom { bank: 0 }
            }
            1 if prg_banks.is_power_of_two()
                && (prg_banks <= 16 || (prg_banks == 32 && chr_banks == 0))
                && (chr_banks == 0 || (chr_banks.is_power_of_two() && chr_banks <= 16)) =>
            {
                MapperState::Mmc1 {
                    shift: 0x10,
                    control: 0x0c,
                    chr0: 0,
                    chr1: 0,
                    prg: 0,
                    last_write: None,
                }
            }
            _ => return Err(fail("unsupported mapper or board size")),
        };
        let offset = 16 + if header[6] & 4 != 0 { 512 } else { 0 };
        let prg_len = prg_banks * 16384;
        let chr_len = chr_banks * 8192;
        if bytes.len() != offset + prg_len + chr_len {
            return Err(fail("truncated image or unexpected trailing data"));
        }
        let prg = bytes[offset..offset + prg_len].to_vec();
        let chr = bytes[offset + prg_len..].to_vec();
        let prg_hash = format!("{:x}", Sha256::digest(&prg));
        let profile = boards::lookup(&prg_hash);
        let four_screen = header[6] & 8 != 0 && !(mapper_id == 1 && profile.ignore_four_screen);
        if four_screen && mapper_id != 4 {
            return Err(fail(
                "four-screen mapping is supported only for MMC3 boards",
            ));
        }
        // Early Micro Machines dumps predate mapper 71 and label the board
        // UNROM. Correct only this known PRG identity: real UNROM still has
        // bus conflicts. Keep the original file digest for saves/catalogs.
        if mapper_id == 2 && profile.mapper == Some(71) {
            mapper_id = 71;
            mapper = MapperState::Camerica {
                bank: 0,
                single_screen: None,
            };
        }
        let mmc6 = mapper_id == 4 && profile.mmc6;
        if mmc6 && parsed.version == image::HeaderVersion::Nes2 {
            return Err(fail("NES 2.0 MMC6 RAM layout is not implemented"));
        }
        let pal = (header[9] & 1 != 0) || profile.pal;
        let mut prg_ram = vec![
            0;
            if mmc6 {
                1024
            } else {
                parsed.prg_ram.unwrap_or(8192)
            }
        ];
        if header[6] & 4 != 0 && !mmc6 {
            prg_ram[0x1000..0x1200].copy_from_slice(&bytes[16..528]);
        }
        Ok(Self {
            prg,
            chr,
            header_mirroring: if four_screen {
                Mirroring::FourScreen
            } else if header[6] & 1 != 0 {
                Mirroring::Vertical
            } else {
                Mirroring::Horizontal
            },
            digest: Sha256::digest(original).into(),
            identity: image::Identity {
                file_sha256: format!("{:x}", Sha256::digest(original)),
                prg_sha256: prg_hash,
                chr_sha256: format!("{:x}", Sha256::digest(&original[offset + prg_len..])),
                header: parsed.version,
                submapper: parsed.submapper,
            },
            mapper_id,
            battery: header[6] & 2 != 0 || mmc6,
            pal,
            mmc6,
            zapper: profile.zapper,
            state: CartridgeState {
                mapper,
                prg_ram,
                chr_ram: if chr_len == 0 {
                    vec![0; 8192]
                } else {
                    Vec::new()
                },
            },
        })
    }
}
