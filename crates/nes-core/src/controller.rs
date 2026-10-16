use serde::{Deserialize, Serialize};

/// Standard NES joypad: A, B, Select, Start, Up, Down, Left, Right.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Controller {
    buttons: u8,
    shift: u8,
    strobe: bool,
}

impl Controller {
    pub fn set_buttons(&mut self, buttons: u8) {
        self.buttons = buttons;
    }
    pub fn strobe(&mut self, high: bool) {
        if high || self.strobe {
            self.shift = self.buttons;
        }
        self.strobe = high;
    }
    pub fn read(&mut self) -> u8 {
        if self.strobe {
            self.buttons & 1
        } else {
            let value = self.shift & 1;
            self.shift = (self.shift >> 1) | 0x80;
            value
        }
    }
    pub fn peek(&self) -> u8 {
        if self.strobe {
            self.buttons & 1
        } else {
            self.shift & 1
        }
    }
}
