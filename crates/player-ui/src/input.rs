use egui::{Key, Pos2, Rect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BUTTONS: [&str; 8] = ["A", "B", "Select", "Start", "Up", "Down", "Left", "Right"];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Device {
    Keyboard,
    Touch,
    Gamepad(usize),
}
impl Device {
    pub fn label(&self) -> String {
        match self {
            Self::Keyboard => "Keyboard".into(),
            Self::Touch => "Touch".into(),
            Self::Gamepad(n) => format!("Gamepad {}", n + 1),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub keys: [[Key; 8]; 2],
    pub devices: [Device; 2],
    pub volume: f32,
    pub muted: bool,
    pub touch: bool,
    pub touch_scale: f32,
    pub left_handed: bool,
    pub aspect: bool,
    pub integer_scale: bool,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
}
fn default_ui_scale() -> f32 {
    1.
}
impl Default for Preferences {
    fn default() -> Self {
        use Key::*;
        Self {
            keys: [
                [J, L, Space, Enter, W, S, A, D],
                [
                    Comma, Period, Num0, Backspace, ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
                ],
            ],
            devices: [Device::Keyboard, Device::Keyboard],
            volume: 0.6,
            muted: false,
            touch: false,
            touch_scale: 1.0,
            left_handed: false,
            aspect: true,
            integer_scale: false,
            ui_scale: 1.,
        }
    }
}
#[derive(Clone, Copy)]
pub enum Zone {
    Dpad,
    Buttons,
}
#[derive(Default)]
pub struct TouchPad {
    fingers: BTreeMap<u64, (Zone, Pos2)>,
}
#[derive(Clone, Copy)]
pub struct TouchLayout {
    pub dpad: Rect,
    pub a: Rect,
    pub b: Rect,
    pub ab: Rect,
}
impl TouchLayout {
    pub fn zone(&self, p: Pos2) -> Option<Zone> {
        if self.dpad.contains(p) {
            Some(Zone::Dpad)
        } else if [self.a, self.b, self.ab].iter().any(|r| r.contains(p)) {
            Some(Zone::Buttons)
        } else {
            None
        }
    }
    pub fn buttons(&self, zone: Zone, p: Pos2) -> u8 {
        match zone {
            Zone::Dpad => {
                if !self.dpad.expand(10.0).contains(p) {
                    return 0;
                }
                let d = (p - self.dpad.center()) / (self.dpad.width() / 2.0);
                let mut b = 0;
                if d.x > 0.28 {
                    b |= 128
                } else if d.x < -0.28 {
                    b |= 64
                }
                if d.y > 0.28 {
                    b |= 32
                } else if d.y < -0.28 {
                    b |= 16
                }
                b
            }
            Zone::Buttons => {
                if self.ab.contains(p) {
                    3
                } else if self.a.contains(p) {
                    1
                } else if self.b.contains(p) {
                    2
                } else {
                    0
                }
            }
        }
    }
}
impl TouchPad {
    pub fn begin(&mut self, id: u64, p: Pos2, layout: &TouchLayout) {
        if let Some(z) = layout.zone(p) {
            self.fingers.insert(id, (z, p));
        }
    }
    pub fn move_to(&mut self, id: u64, p: Pos2) {
        if let Some((_, pos)) = self.fingers.get_mut(&id) {
            *pos = p;
        }
    }
    pub fn end(&mut self, id: u64) {
        self.fingers.remove(&id);
    }
    pub fn clear(&mut self) {
        self.fingers.clear();
    }
    pub fn buttons(&self, layout: &TouchLayout) -> u8 {
        contra_agent::control::normalize(
            self.fingers
                .values()
                .fold(0, |v, (z, p)| v | layout.buttons(*z, *p)),
        )
    }
}
#[cfg(test)]
use session_engine::clock::FrameClock;
#[cfg(test)]
mod tests {
    use super::*;
    fn layout() -> TouchLayout {
        TouchLayout {
            dpad: Rect::from_min_max(Pos2::ZERO, Pos2::new(100., 100.)),
            a: Rect::from_min_max(Pos2::new(150., 0.), Pos2::new(200., 50.)),
            b: Rect::from_min_max(Pos2::new(150., 60.), Pos2::new(200., 110.)),
            ab: Rect::from_min_max(Pos2::new(210., 0.), Pos2::new(260., 50.)),
        }
    }
    #[test]
    fn touch_chords_have_independent_lifetimes_and_cannot_resurrect() {
        let l = layout();
        let mut t = TouchPad::default();
        t.begin(1, Pos2::new(90., 10.), &l);
        t.begin(2, Pos2::new(175., 85.), &l);
        t.begin(3, Pos2::new(230., 25.), &l);
        assert_eq!(t.buttons(&l), 147);
        t.end(3);
        assert_eq!(t.buttons(&l), 146);
        t.end(2);
        assert_eq!(t.buttons(&l), 144);
        t.clear();
        t.move_to(1, Pos2::new(90., 90.));
        assert_eq!(t.buttons(&l), 0);
    }
    #[test]
    fn clock_counts_elapsed_time_at_slow_presentations_and_bounds_backlog() {
        let mut c = FrameClock::default();
        c.frames(0., 60., true);
        let n: usize = (1..=500)
            .map(|i| c.frames(i as f64 * 0.02, 60., true))
            .sum();
        assert!((599..=601).contains(&n));
        assert!(c.frames(1000., 60., true) <= 4);
        assert_eq!(c.frames(1001., 60., false), 0);
        assert_eq!(c.frames(1002., 60., true), 1);
    }
}
