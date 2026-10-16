//! Revision-pinned Snow Bros. first-floor adapter, derived from controlled local play.
use crate::Result;
pub use crate::contracts::snow::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub x: u8,
    pub y: u8,
    pub vx: f32,
    pub vy: f32,
    pub grounded: bool,
    pub dead: bool,
    pub reserve_lives: u8,
    pub invincible: bool,
    pub facing_left: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub kind: u8,
    pub x: u8,
    pub y: u8,
    pub vx: f32,
    pub vy: f32,
    pub grounded: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub floor: u8,
    pub layout: u8,
    pub phase: u8,
    pub remaining_enemies: u8,
    pub players: [Player; 2],
    pub entities: Vec<Entity>,
    pub terrain: [Vec<u8>; 2],
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(|a| m.peek(a).expect("Snow adapter reads only RAM"))
}
fn from_ram(r: impl Fn(u16) -> u8) -> Frame {
    let supported = r(0x73) == 0 && r(0x74) == 0;
    // The cartridge enters mode 5 while it is still constructing the floor.
    // Lives and player-count fields are already set, but both bodies can still
    // be absent (positions are zero). Do not infer from that loading state.
    let bodies_present = (r(0x450) | r(0x451)) & 0x80 != 0;
    let gameplay =
        supported && r(0x70) == 5 && r(0x6f) & 0x78 == 0 && r(0x27) & 0x80 == 0 && bodies_present;
    let players = [0, 1].map(|p| Player {
        active: r(0x76 + p) & 0x80 == 0 && (p == 0 || r(0x72) != 0),
        x: r(0x476 + p),
        y: r(0x463 + p),
        vx: r(0x534 + p) as i8 as f32 / 16.,
        vy: r(0x521 + p) as i8 as f32 / 16.,
        grounded: r(0x354 + p) & 0x80 != 0,
        dead: r(0x369 + p) & 0x60 != 0 || r(0x76 + p) & 0x80 != 0,
        reserve_lives: r(0x76 + p) & 0x7f,
        invincible: r(0x3d2 + p) != 0,
        facing_left: r(0x4fb + p) & 0x40 != 0,
    });
    let entities = (2..19u16)
        .filter(|i| r(0x450 + i) & 0x80 != 0)
        .map(|i| Entity {
            slot: i as usize,
            kind: r(0x32a + i),
            x: r(0x476 + i),
            y: r(0x463 + i),
            vx: r(0x534 + i) as i8 as f32 / 16.,
            vy: r(0x521 + i) as i8 as f32 / 16.,
            grounded: r(0x354 + i) & 0x80 != 0,
        })
        .collect();
    let terrain = std::array::from_fn(|p| {
        (0..5)
            .flat_map(|row| (0..8).map(move |col| (col, row)))
            .map(|(col, row)| {
                let x = players[p].x as i16 + (col - 2) * 16;
                let y = players[p].y as i16 + (row - 2) * 16;
                if !gameplay || !(0..256).contains(&x) || !(0..240).contains(&y) {
                    return 255;
                }
                r(0x5fc + (y as u16 / 16) * 16 + x as u16 / 16)
            })
            .collect()
    });
    Frame {
        supported,
        gameplay,
        floor: r(0x73),
        layout: r(0x74),
        phase: r(0x50) & 7,
        remaining_enemies: r(0x85),
        players,
        entities,
        terrain,
    }
}
impl Frame {
    pub fn observation(&self, player: usize) -> Result<Vec<f32>> {
        if player > 1 {
            return Err("player must be 0 or 1".into());
        }
        let b = |v: bool| v as u8 as f32;
        let mut o = vec![
            player as f32,
            b(self.gameplay),
            self.phase as f32 / 7.,
            (self.phase & 1) as f32,
            self.floor as f32 / 50.,
            self.remaining_enemies as f32 / 8.,
            b(self.players[1].active),
            0.,
        ];
        for p in [&self.players[player], &self.players[1 - player]] {
            o.extend([
                b(p.active),
                p.x as f32 / 255.,
                p.y as f32 / 240.,
                p.vx / 8.,
                p.vy / 8.,
                b(p.grounded),
                b(p.dead),
                p.reserve_lives as f32 / 10.,
                b(p.invincible),
                b(p.facing_left),
                0.,
                0.,
                0.,
                0.,
                0.,
                0.,
            ]);
        }
        let me = &self.players[player];
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
                    e.kind as f32 / 63.,
                    b(e.grounded),
                ]);
            } else {
                o.extend([0.; 7])
            }
        }
        for &code in &self.terrain[player] {
            o.extend([
                b(code != 255),
                b(code == 1),
                b(code == 2),
                b(code > 2 && code != 255),
            ])
        }
        debug_assert_eq!(o.len(), 256);
        for x in &mut o {
            *x = x.clamp(-1., 1.)
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("Snow task uses a normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [8, 8]),
        (120, [0, 0]),
        (1, [32, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
        (1, [8, 8]),
    ] {
        crate::registry::run_reset(machine, masks, n)?
    }
    for _ in 0..1200 {
        let f = observe(machine);
        if f.gameplay
            && f.remaining_enemies == 4
            && f.players.iter().all(|p| p.active && p.grounded)
            && machine.peek(0x450).unwrap() & 0x80 != 0
            && machine.peek(0x451).unwrap() & 0x80 != 0
        {
            break;
        }
        crate::registry::run_reset(machine, [0, 0], 1)?
    }

    Ok(())
}

pub(crate) fn policy_observation(
    m: &Machine,
    slot: usize,
) -> Result<crate::registry::PolicyObservation> {
    let r = |a| m.peek(a).unwrap_or(0);
    let f = observe(m);
    let phase = crate::control::Phase::classify(r(0x70) < 5, f.supported, f.gameplay);
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
    fn loading_floor_is_not_policy_gameplay_until_a_body_exists() {
        let mut ram = [0; 2048];
        ram[0x70] = 5;
        ram[0x72] = 1;
        ram[0x76] = 2;
        ram[0x77] = 2;
        let loading = from_ram(|a| ram[a as usize]);
        assert!(loading.supported);
        assert!(!loading.gameplay);
        assert_eq!(loading.observation(0).unwrap()[1], 0.);
        ram[0x451] = 0x80;
        assert!(from_ram(|a| ram[a as usize]).gameplay);
        ram[0x27] = 0x80;
        assert!(!from_ram(|a| ram[a as usize]).gameplay);
    }

    #[test]
    fn independent_ports_lives_and_collision_grid() {
        let mut r = [0; 2048];
        r[0x70] = 5;
        r[0x450] = 0x80;
        r[0x72] = 1;
        r[0x476] = 104;
        r[0x477] = 152;
        r[0x463] = 197;
        r[0x464] = 197;
        r[0x354] = 0x80;
        r[0x355] = 0x80;
        r[0x76] = 2;
        r[0x77] = 1;
        r[0x5fc + 13 * 16 + 6] = 1;
        let f = from_ram(|a| r[a as usize]);
        assert!(f.gameplay);
        let a = f.observation(0).unwrap();
        let b = f.observation(1).unwrap();
        assert_eq!(a.len(), 256);
        assert_eq!(a[9], b[25]);
        assert_eq!(a[15], 0.2);
        assert_eq!(b[15], 0.1);
        assert!(f.terrain[0].contains(&1));
        r[0x76] = 0x80;
        r[0x73] = 1;
        let f = from_ram(|a| r[a as usize]);
        assert!(!f.gameplay);
        assert!(!f.players[0].active);
        assert!(f.players[0].dead);
        assert!(f.observation(2).is_err());
    }
}
