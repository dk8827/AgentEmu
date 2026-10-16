//! Pinned US Life Force first-stage cooperative observations.
use crate::Result;
pub use crate::contracts::lifeforce::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
pub const RESET_FRAMES: u64 = 752;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub dead: bool,
    pub controllable: bool,
    pub x: u8,
    pub y: u8,
    pub state: u8,
    pub reserves: u8,
    pub weapon: u8,
    pub speed: u8,
    pub shield: u8,
    pub missiles: u8,
    pub options: u8,
    pub invincibility: u8,
    pub upgrade: u8,
    pub score: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: u8,
    pub kind: u8,
    pub x: u8,
    pub y: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub stage_cleared: bool,
    pub two_players: bool,
    pub paused: bool,
    pub level: u8,
    pub phase: u8,
    pub vertical: bool,
    pub players: [Player; 2],
    pub entities: Vec<Entity>,
    pub terrain: [Vec<bool>; 2],
}
fn blocked(r: &impl Fn(u16) -> u8, x: i16, y: i16) -> bool {
    if !(0..=255).contains(&x) || !(0..=239).contains(&y) {
        return true;
    }
    let sx = x as u16 + r(0xfd) as u16;
    let sy = (y as u16 + r(0xfc) as u16) % 240;
    let group = (sx & 255) >> 6;
    let page = (((sx >> 8) ^ r(0xff) as u16) & 1) << 7;
    let index = ((sy & 248) >> 1) | group | page;
    r(0x500 + index) & (128 >> ((sx >> 3) & 7)) != 0
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(
        |a| m.peek(a).expect("Life Force reads RAM only"),
        (m.ppu().completed_frames().saturating_sub(RESET_FRAMES) % 16) as u8,
    )
}
fn from_ram(r: impl Fn(u16) -> u8, phase: u8) -> Frame {
    let two_players = r(0x1d) == 1;
    let real_game = r(0x18) == 5 && r(0x1c) == 0;
    let paused = r(0x24) == 1;
    let level = r(0x30);
    let players = std::array::from_fn(|i| {
        let i = i as u16;
        let state = r(0x70 + i);
        let score = (0..3u16)
            .map(|j| {
                let v = r(0x7e4 + i * 4 + j);
                ((v >> 4) * 10 + (v & 15)) as u32 * 10 * 100u32.pow(j as u32)
            })
            .sum();
        Player {
            active: !matches!(state, 0 | 6),
            dead: matches!(state, 4..=6),
            controllable: state == 3,
            x: r(0x350 + i),
            y: r(0x32f + i),
            state,
            reserves: r(0x34 + i),
            weapon: r(0x76 + i),
            speed: r(0x80 + i),
            shield: r(0x82 + i),
            missiles: r(0x86 + i),
            options: r(0x8a + i),
            invincibility: r(0x84 + i),
            upgrade: r(0x78 + i),
            score,
        }
    });
    let terrain = std::array::from_fn(|i| {
        (-2..=2)
            .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| {
                blocked(
                    &r,
                    players[i].x as i16 + dx * 8,
                    players[i].y as i16 + dy * 8,
                )
            })
            .collect()
    });
    let supported = real_game && two_players && level == 0;
    Frame {
        supported,
        gameplay: supported && !paused,
        stage_cleared: real_game
            && two_players
            && level == 1
            && players.iter().any(|p| p.active && !p.dead),
        two_players,
        paused,
        level,
        phase,
        vertical: r(0x40) != 0,
        players,
        terrain,
        entities: (0..33u16)
            .filter(|i| !matches!(i, 10 | 11) && r(0x304 + i) & 127 != 0)
            .map(|i| Entity {
                slot: i as u8,
                kind: r(0x304 + i) & 127,
                x: r(0x346 + i),
                y: r(0x325 + i),
            })
            .collect(),
    }
}
impl Frame {
    pub fn progress(&self) -> u32 {
        self.players.iter().map(|p| p.score).sum::<u32>() / 100
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
            self.phase as f32 / 15.,
            b(self.phase < 2),
            b(self.vertical),
            self.entities.len() as f32 / 31.,
            b(self.players[1 - slot].active),
            b(self.phase.is_multiple_of(8)),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..24 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 255.,
                p.y as f32 / 239.,
                p.reserves as f32 / 8.,
                b(p.dead),
                b(p.controllable),
                p.state as f32 / 7.,
                p.weapon as f32 / 16.,
                p.speed as f32 / 8.,
                p.shield as f32 / 255.,
                p.missiles as f32 / 8.,
                p.options as f32 / 4.,
                p.invincibility as f32 / 255.,
                p.upgrade as f32 / 6.,
                p.score as f32 / 1000000.,
                0.,
            ]);
        }
        let me = &self.players[slot];
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|e| {
            (
                (e.x as i32 - me.x as i32).pow(2) + (e.y as i32 - me.y as i32).pow(2),
                e.slot,
            )
        });
        for (i, e) in entities.into_iter().take(24).enumerate() {
            o[40 + i * 5..45 + i * 5].copy_from_slice(&[
                1.,
                e.kind as f32 / 127.,
                (e.x as f32 - me.x as f32) / 255.,
                (e.y as f32 - me.y as f32) / 239.,
                e.slot as f32 / 32.,
            ]);
        }
        for i in 0..2 {
            for (cell, v) in self.terrain[if i == 0 { slot } else { 1 - slot }]
                .iter()
                .enumerate()
            {
                o[160 + i * 25 + cell] = b(*v);
            }
        }
        for v in &mut o {
            *v = v.clamp(-1., 1.);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("Life Force task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (120, [0, 0]),
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
    fn menus_demos_pause_and_last_life_are_distinct() {
        let mut r = [0; 2048];
        r[0x18] = 5;
        r[0x1d] = 1;
        r[0x70] = 3;
        r[0x71] = 6;
        let read = |r: &[u8; 2048]| from_ram(|a| r[a as usize], 0);
        assert!(read(&r).gameplay);
        assert!(read(&r).players[0].active);
        r[0x24] = 1;
        assert!(!read(&r).gameplay);
        r[0x24] = 0;
        r[0x1c] = 1;
        assert!(!read(&r).supported);
        r[0x30] = 1;
        assert!(!read(&r).stage_cleared);
        r[0x1c] = 0;
        assert!(read(&r).stage_cleared);
        assert!(!read(&r).gameplay);
        r[0x70] = 6;
        assert!(!read(&r).stage_cleared);
        assert!(read(&r).observation(2).is_err());
    }
}
