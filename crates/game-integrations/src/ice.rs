//! Ice Climber US mountain-one research observations. Read-only and ROM-pinned.
use crate::Result;
pub use crate::contracts::ice::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub lives: u8,
    pub x: u8,
    pub y: u8,
    pub floor: u8,
    pub motion: u8,
    pub horizontal_motion: u8,
    pub facing: u8,
    pub jump_held: bool,
    pub hammer_held: bool,
    pub dead: bool,
    pub grounded: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub x: u8,
    pub y: u8,
    pub floor: u8,
    pub state: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub demo: bool,
    pub mountain: u8,
    pub completed: u8,
    pub phase: u8,
    pub clock: u8,
    pub upper_visible_floor: u8,
    pub players: [Player; 2],
    /// Eight floors, bottom to top. 24 cells of 8 pixels, starting at X=32.
    /// High-bit values are holes. Other cartridge tile codes remain explicit.
    pub floor_tiles: Vec<Vec<u8>>,
    /// The ninth, unbreakable platform row. Kept separate so the legacy 256-field
    /// observation retains its eight-row terrain layout and four entity slots.
    pub upper_platform_tiles: Vec<u8>,
    pub entities: Vec<Entity>,
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(|a| m.peek(a).expect("Ice adapter reads RAM only"))
}
fn from_ram(r: impl Fn(u16) -> u8) -> Frame {
    let demo = r(0x53) != 0;
    let mountain = r(0x59);
    let phase = r(0x55);
    let players = [0u16, 1].map(|i| {
        let lives = r(0x20 + i);
        let motion = r(0x6a + i);
        let active = lives < 128 && (i == 0 || r(0x58) != 0);
        Player {
            active,
            lives: if active { lives } else { 0 },
            x: r(0x64 + i),
            y: r(0x66 + i),
            floor: r(0x5a + i),
            motion,
            horizontal_motion: r(0x68 + i),
            facing: r(0x62 + i),
            jump_held: r(0x14 + 2 * i) & 128 != 0,
            hammer_held: r(0x14 + 2 * i) & 64 != 0,
            dead: !active || motion == 7,
            grounded: active && matches!(motion, 1 | 3 | 5),
        }
    });
    Frame {
        supported: mountain == 0 && !demo,
        gameplay: mountain == 0 && !demo && phase == 1 && players.iter().any(|p| p.active),
        demo,
        mountain,
        completed: r(0x56),
        phase,
        clock: r(0x50),
        upper_visible_floor: r(0x90),
        players,
        floor_tiles: (0..8)
            .map(|row| (0..24).map(|col| r(0x400 + row * 24 + col)).collect())
            .collect(),
        upper_platform_tiles: (0..24).map(|col| r(0x4c0 + col)).collect(),
        entities: (0..11u16)
            .filter(|i| r(0x91 + i) != 0 && r(0xa7 + i) < 240)
            .map(|i| Entity {
                slot: i as usize,
                x: r(0x9c + i),
                y: r(0xa7 + i),
                floor: r(0xb2 + i),
                state: r(0x91 + i),
            })
            .collect(),
    }
}
impl Frame {
    pub fn completed_task(&self) -> bool {
        !self.demo && self.completed > 0 && self.mountain == 1
    }
    pub fn observation(&self, slot: usize) -> Result<Vec<f32>> {
        if slot > 1 {
            return Err("player must be 0 or 1".into());
        }
        let b = |v: bool| v as u8 as f32;
        let mut o = vec![0.; 256];
        o[..8].copy_from_slice(&[
            slot as f32,
            b(self.gameplay),
            self.clock as f32 / 255.,
            b(self.clock & 2 != 0),
            self.mountain as f32 / 255.,
            self.phase as f32 / 255.,
            self.upper_visible_floor as f32 / 255.,
            b(self.players[1 - slot].active),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..22 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 255.,
                p.y as f32 / 255.,
                p.lives as f32 / 127.,
                b(p.dead),
                b(p.grounded),
                p.floor as f32 / 255.,
                p.motion as f32 / 255.,
                p.horizontal_motion as f32 / 255.,
                p.facing as f32 / 255.,
                b(matches!(p.motion, 2 | 4 | 6 | 8)),
                b(p.motion == 5),
                b(p.jump_held),
                b(p.hammer_held),
            ]);
        }
        for (index, code) in self.floor_tiles.iter().flatten().enumerate() {
            o[40 + index] = if code & 128 != 0 {
                -1.
            } else {
                *code as f32 / 127.
            };
        }
        let p = &self.players[slot];
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|e| {
            (
                (e.x as i32 - p.x as i32).abs() + (e.y as i32 - p.y as i32).abs(),
                e.slot,
            )
        });
        for (i, e) in entities.iter().take(4).enumerate() {
            o[232 + i * 6..238 + i * 6].copy_from_slice(&[
                1.,
                (e.x as f32 - p.x as f32) / 256.,
                (e.y as f32 - p.y as f32) / 256.,
                e.floor as f32 / 255.,
                e.state as f32 / 255.,
                0.,
            ]);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("Ice task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [4, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
    ] {
        crate::registry::run_reset(machine, masks, n)?;
    }

    Ok(())
}

pub(crate) fn policy_observation(
    m: &Machine,
    slot: usize,
) -> Result<crate::registry::PolicyObservation> {
    let r = |a| m.peek(a).unwrap_or(0);
    let f = observe(m);
    let phase =
        crate::control::Phase::classify(r(0x55) == 0 && r(0x59) == 0, f.supported, f.gameplay);
    let observation = if phase == crate::control::Phase::SupportedGameplay {
        Some(f.observation(slot)?)
    } else {
        None
    };
    Ok((phase, observation))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upper_platform_row_is_inspectable_without_changing_legacy_model_inputs() {
        let mut ram = [0u8; 2048];
        ram[0x55] = 1;
        ram[0x58] = 1;
        let before = from_ram(|a| ram[a as usize]);
        ram[0x4c3] = 255;
        ram[0x4c4] = 255;
        let after = from_ram(|a| ram[a as usize]);
        assert_eq!(after.floor_tiles.len(), 8);
        assert_eq!(after.upper_platform_tiles.len(), 24);
        assert_eq!(&after.upper_platform_tiles[3..5], &[255, 255]);
        for slot in 0..2 {
            assert_eq!(
                before.observation(slot).unwrap(),
                after.observation(slot).unwrap()
            );
            assert!(
                after.observation(slot).unwrap()[232..]
                    .iter()
                    .all(|v| *v == 0.)
            );
        }
    }
    #[test]
    fn lives_zero_is_last_life_and_high_bit_is_out() {
        let mut ram = [0u8; 2048];
        ram[0x55] = 1;
        ram[0x58] = 1;
        ram[0x6a] = 1;
        ram[0x6b] = 7;
        let f = from_ram(|a| ram[a as usize]);
        assert!(f.players[0].active);
        assert!(f.players[1].dead);
        ram[0x21] = 255;
        let f = from_ram(|a| ram[a as usize]);
        assert!(!f.players[1].active);
        assert_eq!(f.players[1].lives, 0);
    }
    #[test]
    fn menus_demo_bonus_and_second_mountain_do_not_run_policy() {
        let mut ram = [0u8; 2048];
        ram[0x55] = 1;
        assert!(from_ram(|a| ram[a as usize]).gameplay);
        for phase in [0, 2, 3, 4] {
            ram[0x55] = phase;
            assert!(!from_ram(|a| ram[a as usize]).gameplay);
        }
        ram[0x55] = 1;
        ram[0x53] = 1;
        assert!(!from_ram(|a| ram[a as usize]).supported);
        ram[0x53] = 0;
        ram[0x59] = 1;
        assert!(!from_ram(|a| ram[a as usize]).supported);
    }
    #[test]
    fn completion_requires_real_mountain_progress_not_demo_or_menu_selection() {
        let mut ram = [0u8; 2048];
        ram[0x59] = 1;
        assert!(!from_ram(|a| ram[a as usize]).completed_task());
        ram[0x56] = 1;
        assert!(from_ram(|a| ram[a as usize]).completed_task());
        ram[0x53] = 1;
        assert!(!from_ram(|a| ram[a as usize]).completed_task());
    }
    #[test]
    fn observations_keep_players_separate_and_bound_features() {
        let mut ram = [255u8; 2048];
        ram[0x64] = 50;
        ram[0x65] = 180;
        let f = from_ram(|a| ram[a as usize]);
        let a = f.observation(0).unwrap();
        let b = f.observation(1).unwrap();
        assert_eq!(a.len(), 256);
        assert_eq!(a[9], b[25]);
        assert_eq!(a[25], b[9]);
        assert_eq!(a[40], -1.);
        assert!(a.iter().all(|x| x.is_finite() && (-1. ..=1.).contains(x)));
        assert!(f.observation(2).is_err());
    }
}
