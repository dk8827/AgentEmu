//! Current game-read controller history and remembered aim; no host-side memory.
//! All fields are already covered by the ordinary machine snapshot.
use nes_core::machine::Machine;

pub fn encode(
    base: &[f32],
    history: [u8; 2],
    aim: [u8; 2],
    with_aim: bool,
) -> crate::Result<Vec<f32>> {
    if base.len() != 256 || base.iter().any(|x| !x.is_finite()) {
        return Err("expected 256 finite base features".into());
    }
    let mut result = base.to_vec();
    // Cartridge order: A, B, Select, Start, Up, Down, Left, Right (MSB first).
    // Own player first, then teammate. These are game-read buttons, not proposals.
    for mask in history {
        result.extend((0..8).rev().map(|bit| ((mask >> bit) & 1) as f32));
    }
    if with_aim {
        for code in aim {
            // All eleven valid aim codes, plus an explicit unknown category.
            result.extend((0..12).map(|i| u8::from(i == code.min(11)) as f32));
        }
    }
    Ok(result)
}
pub fn extend(m: &Machine, slot: usize, base: &[f32], features: &str) -> crate::Result<Vec<f32>> {
    if slot > 1 || m.cartridge_hash() != crate::adapter::ROM_SHA256 {
        return Err("Contra temporal features require exact ROM and valid player".into());
    }
    if !["history", "history-aim"].contains(&features) {
        return Err("unknown temporal feature contract".into());
    }
    let ports = [slot as u16, (1 - slot) as u16];
    let history = ports.map(|p| m.peek(0xf1 + p).expect("internal RAM"));
    let aim = ports.map(|p| m.peek(0xc2 + p).expect("internal RAM"));
    encode(base, history, aim, features == "history-aim")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn button_order_and_all_aim_codes_are_distinct() {
        for code in 0..=255 {
            let x = encode(&[0.; 256], [0x40, 0x01], [code, 10], true).unwrap();
            assert_eq!(x.len(), 296);
            assert_eq!(x[257], 1.); // own game-read B
            assert_eq!(x[271], 1.); // teammate game-read Right
            assert_eq!(x[272..284].iter().sum::<f32>(), 1.);
            assert_eq!(x[272 + code.min(11) as usize], 1.);
            assert_eq!(x[294], 1.); // valid code ten must not share unknown
        }
        assert_eq!(
            encode(&[0.; 256], [0; 2], [0; 2], false).unwrap().len(),
            272
        );
        assert!(encode(&[0.; 255], [0; 2], [0; 2], true).is_err());
        assert!(encode(&[f32::NAN; 256], [0; 2], [0; 2], false).is_err());
    }
}
