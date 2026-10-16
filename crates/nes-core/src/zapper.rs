//! Port-2 light gun. Light detection follows the rendered beam, not a whole-frame
//! hit test: a bright pixel within the sensor aperture holds detection briefly.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Zapper {
    pub aim: Option<[u16; 2]>,
    pub trigger: bool,
    pub light_until: u64,
}
impl Zapper {
    pub fn read(&self, dot: u64) -> u8 {
        (u8::from(self.trigger) << 4)
            | (u8::from(self.aim.is_none() || dot >= self.light_until) << 3)
    }
    pub fn sample(&mut self, x: u16, y: u16, color: u16, dot: u64) {
        if let Some([ax, ay]) = self.aim
            && ax.abs_diff(x) <= 4
            && ay.abs_diff(y) <= 4
        {
            let c = color & 63;
            // Bright gray/colored levels trigger the photodiode; black entries
            // never do. This is a deterministic sensor approximation.
            if (c >= 0x20 && c & 15 < 14) || (c == 0x10) {
                self.light_until = dot.saturating_add(26 * 341);
            }
        }
    }
    pub fn valid(&self, dot: u64) -> bool {
        self.aim.is_none_or(|[x, y]| x < 256 && y < 240)
            && self.light_until <= dot.saturating_add(26 * 341)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensor_requires_bright_pixels_at_aim_and_expires() {
        let mut z = Zapper {
            aim: Some([100, 100]),
            trigger: true,
            ..Default::default()
        };
        assert_eq!(z.read(0), 24);
        z.sample(0, 0, 0x30, 1);
        z.sample(100, 100, 0x0f, 2);
        assert_eq!(z.read(3), 24);
        z.sample(100, 100, 0x30, 4);
        assert_eq!(z.read(5), 16);
        assert_eq!(z.read(4 + 26 * 341), 24);
        z.aim = None;
        assert_eq!(z.read(5), 24);
    }
}
