//! NTSC 2C02 RGB lookup, including all emphasis combinations.
//! Provenance and reproducible generation: ../palettes/README.md.
const COLORS: &[u8; 512 * 3] = include_bytes!("../palettes/2c02.pal");

pub fn rgba(codes: &[u16]) -> Vec<u8> {
    let mut result = Vec::with_capacity(codes.len() * 4);
    for &code in codes {
        let start = usize::from(code & 0x1ff) * 3;
        result.extend_from_slice(&COLORS[start..start + 3]);
        result.push(255);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ntsc_black_white_and_emphasis_use_the_same_lookup() {
        let codes: Vec<_> = (0..512).collect();
        let output = rgba(&codes);
        assert_eq!(output.len(), 512 * 4);
        assert!(output.chunks_exact(4).all(|c| c[3] == 255));
        assert_eq!(rgba(&[0x0f, 0x20]), [0, 0, 0, 255, 255, 255, 255, 255]);
        assert_ne!(rgba(&[0x21]), rgba(&[0x61]));
        assert_eq!(rgba(&[0x1ff]), rgba(&[0xffff]));
    }
}
