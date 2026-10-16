use crate::Result;
pub use crate::contracts::adapter::{ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
pub const INPUTS: usize = 256;

pub fn validate_rom(m: &Machine) -> Result<()> {
    if m.cartridge_hash() != ROM_SHA256 {
        return Err("Contra agent requires the pinned US ROM revision".into());
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub x: u8,
    pub y: u8,
    /// Residual screen-X motion ($98). The game clears this when movement is
    /// transferred to camera scrolling; it is not net world-X velocity.
    pub vx: i8,
    pub vy: f32,
    pub grounded: bool,
    pub dead: bool,
    pub reserve_lives: u8,
    pub invincible: bool,
    pub water: bool,
    pub weapon: u8,
    pub rapid: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub kind: u8,
    pub x: u8,
    pub y: u8,
    pub vx: f32,
    pub vy: f32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub level: u8,
    pub camera_x: u16,
    pub phase: u8,
    pub two_players: bool,
    pub boss_defeated: bool,
    pub players: [Player; 2],
    pub entities: Vec<Entity>,
    /// Two player-centered 8x5 row-major grids. 0 empty, 1 floor, 2 water, 3 solid, 4 unknown.
    pub terrain: [Vec<u8>; 2],
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(|a| m.peek(a).expect("adapter reads only RAM"))
}
fn from_ram(r: impl Fn(u16) -> u8) -> Frame {
    let level = r(0x30);
    let supported = level == 0 && r(0x40) == 0 && r(0x41) == 0;
    let gameplay = supported && r(0x18) == 5 && r(0x2c) == 4 && r(0x1c) == 0 && r(0x25) == 0;
    let players = [0, 1].map(|i| Player {
        active: r(0x38 + i) == 0 && (i == 0 || r(0x22) == 1),
        x: r(0x334 + i),
        y: r(0x31a + i),
        vx: r(0x98 + i) as i8,
        vy: r(0xc6 + i) as i8 as f32 + r(0xc4 + i) as f32 / 256.,
        grounded: r(0xa0 + i) & 1 == 0 && r(0xa4 + i) == 0 && r(0x90 + i) == 1,
        dead: r(0xb4 + i) & 1 != 0 || r(0x90 + i) == 2,
        reserve_lives: r(0x32 + i),
        invincible: r(0xae + i) != 0 || r(0xb0 + i) != 0,
        water: r(0xb2 + i) & 4 != 0,
        weapon: r(0xaa + i) & 15,
        rapid: r(0xaa + i) & 16 != 0,
    });
    let entities = (0..16u16)
        .filter(|i| r(0x4b8 + i) != 0 && r(0x30a + i) != 0)
        .map(|i| Entity {
            slot: i as usize,
            kind: r(0x528 + i),
            x: r(0x33e + i),
            y: r(0x324 + i),
            vx: r(0x508 + i) as i8 as f32 + r(0x518 + i) as f32 / 256.,
            vy: r(0x4e8 + i) as i8 as f32 + r(0x4f8 + i) as f32 / 256.,
        })
        .collect();
    let terrain = std::array::from_fn(|p| {
        (0..5)
            .flat_map(|row| (0..8).map(move |col| (col, row)))
            .map(|(col, row)| {
                let x = players[p].x as i16 + (col - 2) * 16;
                let y = players[p].y as i16 + (row - 2) * 16;
                if !gameplay || !(0..256).contains(&x) || !(0..224).contains(&y) {
                    return 4;
                }
                collision(&r, x as u8, y as u8)
            })
            .collect()
    });
    Frame {
        supported,
        gameplay,
        level,
        camera_x: r(0x64) as u16 * 256 + r(0x65) as u16,
        phase: r(0x1a) & 7,
        two_players: r(0x22) == 1,
        boss_defeated: r(0x3b) != 0,
        players,
        entities,
        terrain,
    }
}
/// Decode the game's 16px collision lattice, including horizontal nametable wrap.
fn collision(r: &impl Fn(u16) -> u8, x: u8, y: u8) -> u8 {
    let horizontal = x as u16 + r(0xfd) as u16;
    let table = (r(0xff) & 1) ^ (horizontal >> 8) as u8;
    let vertical = (y as u16 + r(0xfc) as u16) % 240;
    let offset = table as u16 * 64 + (vertical / 16) * 4 + (horizontal % 256) / 64;
    let shift = 6 - ((horizontal % 64) / 16) * 2;
    (r(0x680 + offset) >> shift) & 3
}
impl Frame {
    pub fn observation(&self, player: usize) -> Result<Vec<f32>> {
        if player > 1 {
            return Err("player must be 0 or 1".into());
        }
        let me = &self.players[player];
        let mut o = vec![
            player as f32,
            self.gameplay as u8 as f32,
            self.phase as f32 / 7.,
            (self.phase & 1) as f32,
            self.camera_x as f32 / 4096.,
            (self.camera_x as f32 + me.x as f32) / 4096.,
            self.boss_defeated as u8 as f32,
            self.two_players as u8 as f32,
        ];
        for p in [&self.players[player], &self.players[1 - player]] {
            o.extend([
                p.active as u8 as f32,
                p.x as f32 / 255.,
                p.y as f32 / 240.,
                p.vx as f32 / 4.,
                p.vy / 8.,
                p.grounded as u8 as f32,
                p.dead as u8 as f32,
                p.reserve_lives as f32 / 10.,
                p.invincible as u8 as f32,
                p.water as u8 as f32,
            ]);
            o.extend((0..5).map(|w| (p.weapon == w) as u8 as f32));
            o.push(p.rapid as u8 as f32);
        }
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|e| {
            (
                (e.x as i32 - me.x as i32).pow(2) + (e.y as i32 - me.y as i32).pow(2),
                e.slot,
            )
        });
        for i in 0..8 {
            if let Some(e) = entities.get(i) {
                o.extend([
                    1.,
                    (e.x as f32 - me.x as f32) / 256.,
                    (e.y as f32 - me.y as f32) / 240.,
                    e.vx / 8.,
                    e.vy / 8.,
                    matches!(e.kind, 1 | 0x0b | 0x0f) as u8 as f32,
                    (e.kind == 0) as u8 as f32,
                ]);
            } else {
                o.extend([0.; 7]);
            }
        }
        for &code in &self.terrain[player] {
            o.extend([
                (code != 4) as u8 as f32,
                (code == 1) as u8 as f32,
                (code == 2) as u8 as f32,
                (code == 3) as u8 as f32,
            ]);
        }
        debug_assert_eq!(o.len(), INPUTS);
        for x in &mut o {
            *x = x.clamp(-1., 1.);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
pub(crate) fn policy_observation(
    m: &Machine,
    slot: usize,
) -> Result<crate::registry::PolicyObservation> {
    let r = |a| m.peek(a).unwrap_or(0);
    let f = observe(m);
    let phase =
        crate::control::Phase::classify(r(0x18) < 5 || r(0x1c) != 0, f.supported, f.gameplay);
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
    fn geometry_wrap_masks_and_relative_players() {
        let mut r = [0u8; 2048];
        r[0x18] = 5;
        r[0x2c] = 4;
        r[0x22] = 1;
        r[0x90] = 1;
        r[0x91] = 1;
        r[0x334] = 32;
        r[0x335] = 96;
        r[0x31a] = 80;
        r[0x31b] = 80;
        r[0x680..0x6c0].fill(0b00011011);
        r[0x6c0..0x700].fill(0b11100100);
        assert_eq!(collision(&|a| r[a as usize], 16, 0), 1);
        r[0xfd] = 250;
        assert_eq!(collision(&|a| r[a as usize], 16, 0), 3);
        let f = from_ram(|a| r[a as usize]);
        assert!(f.gameplay);
        let a = f.observation(0).unwrap();
        let b = f.observation(1).unwrap();
        assert_eq!(a.len(), 256);
        assert_eq!(a[9], b[25]);
        assert_eq!(a[25], b[9]);
        assert!(a.iter().all(|v| v.is_finite() && v.abs() <= 1.));
        assert!(f.observation(2).is_err());
        r[0x1c] = 1;
        assert!(!from_ram(|a| r[a as usize]).gameplay);
        r[0x30] = 1;
        assert!(!from_ram(|a| r[a as usize]).supported);
    }
    #[test]
    fn rejects_other_roms_without_mutating_machine() {
        let m = Machine::new(
            nes_core::cartridge::Cartridge::from_ines(&nes_core::diagnostic::playground_rom())
                .unwrap(),
        );
        let before = m.save_state();
        assert!(validate_rom(&m).is_err());
        observe(&m);
        assert_eq!(before, m.save_state());
    }
}
