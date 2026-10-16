//! Portable NES state envelope v1. Legacy schema-6 JSON remains importable.
//! Header: eight-byte magic, LE header length, bounded JSON metadata, zlib payload.
use crate::machine::{Machine, MachineError};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
const MAGIC: &[u8; 8] = b"NESSTATE";
const LIMIT: usize = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    version: u32,
    system: String,
    core_schema: u32,
    producer: String,
    rom_sha256: String,
    payload_sha256: String,
    payload_bytes: usize,
}
fn invalid(e: impl std::fmt::Display) -> MachineError {
    MachineError::InvalidState(e.to_string())
}
pub fn export(machine: &Machine) -> Result<Vec<u8>, MachineError> {
    if machine.fault().is_some() {
        return Err(invalid(
            "Cannot export a faulted machine; restart or restore first",
        ));
    }
    let payload = machine.save_state();
    let header = serde_json::to_vec(&Header {
        version: 1,
        system: "nes".into(),
        core_schema: 6,
        producer: concat!("nes-core/", env!("CARGO_PKG_VERSION")).into(),
        rom_sha256: machine.cartridge_hash(),
        payload_sha256: format!("{:x}", Sha256::digest(&payload)),
        payload_bytes: payload.len(),
    })
    .map_err(invalid)?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&payload).map_err(invalid)?;
    let compressed = encoder.finish().map_err(invalid)?;
    let mut out = Vec::with_capacity(12 + header.len() + compressed.len());
    out.extend(MAGIC);
    out.extend((header.len() as u32).to_le_bytes());
    out.extend(header);
    out.extend(compressed);
    Ok(out)
}
pub fn restore(machine: &mut Machine, bytes: &[u8]) -> Result<(), MachineError> {
    if !bytes.starts_with(MAGIC) {
        return machine.load_state(bytes);
    }
    if bytes.len() < 12 || bytes.len() > LIMIT + 4096 {
        return Err(invalid("Invalid state file length"));
    }
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if len > 4096 || len > bytes.len() - 12 {
        return Err(invalid("Invalid state header length"));
    }
    let header: Header = serde_json::from_slice(&bytes[12..12 + len]).map_err(invalid)?;
    if header.version != 1
        || header.system != "nes"
        || header.core_schema != 6
        || header.rom_sha256 != machine.cartridge_hash()
        || header.payload_bytes > LIMIT
    {
        return Err(invalid("Unsupported state version, system, or cartridge"));
    }
    let mut decoder = ZlibDecoder::new(&bytes[12 + len..]);
    let mut payload = Vec::new();
    decoder
        .by_ref()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut payload)
        .map_err(invalid)?;
    if payload.len() != header.payload_bytes
        || decoder.total_in() as usize != bytes.len() - 12 - len
        || format!("{:x}", Sha256::digest(&payload)) != header.payload_sha256
    {
        return Err(invalid("State file checksum or length mismatch"));
    }
    machine.load_state(&payload)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cartridge::Cartridge, diagnostic::playground_rom};
    fn machine() -> Machine {
        Machine::new(Cartridge::from_ines(&playground_rom()).unwrap())
    }
    #[test]
    fn portable_and_legacy_import_preserve_exact_future_and_fail_transactionally() {
        let mut m = machine();
        for _ in 0..12 {
            m.run_frame().unwrap();
            m.take_audio();
        }
        let raw = m.save_state();
        let checkpoint = m.checkpoint();
        let file = export(&m).unwrap();
        assert!(file.len() < raw.len() / 2);
        m.run_frame().unwrap();
        let next = m.save_state();
        let audio = m.take_audio();
        for portable in [raw.clone(), file.clone()] {
            restore(&mut m, &portable).unwrap();
            m.run_frame().unwrap();
            assert_eq!(m.save_state(), next);
            assert_eq!(m.take_audio(), audio);
        }
        m.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(m.save_state(), raw);
        for corrupt in [
            file[..11].to_vec(),
            [file.as_slice(), b"trailing"].concat(),
            {
                let mut bad = file.clone();
                let n = bad.len();
                bad[n - 3] ^= 0x40;
                bad
            },
        ] {
            assert!(restore(&mut m, &corrupt).is_err());
            assert_eq!(m.save_state(), raw);
        }
        let mut wrong = file.clone();
        wrong[12 + 11] = b'9';
        assert!(restore(&mut m, &wrong).is_err());
        assert_eq!(m.save_state(), raw);
    }
}
