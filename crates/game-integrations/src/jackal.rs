//! Pinned US Jackal first-stage cooperative task.
use crate::Result;
pub use crate::contracts::jackal::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
pub const RESET_FRAMES: u64 = 1926;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub dead: bool,
    pub x: u8,
    pub y: u8,
    pub world_x: u16,
    pub north: u16,
    pub lives: u8,
    pub state: u8,
    pub weapon: u8,
    pub facing: u8,
    pub invincibility: u8,
    pub grenade_active: bool,
    pub grenade_held: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Enemy {
    pub slot: u8,
    pub kind: u8,
    pub state: u8,
    pub x: u8,
    pub y: u8,
    pub health: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub stage_cleared: bool,
    pub two_players: bool,
    pub paused: bool,
    pub level: u8,
    pub screen: u8,
    pub scroll: u8,
    pub camera_x: u8,
    pub phase: u8,
    pub boss_remaining: u8,
    pub players: [Player; 2],
    pub enemies: Vec<Enemy>,
    pub terrain: [Vec<u8>; 2],
}
fn collision(r: &impl Fn(u16) -> u8, x: i16, y: i16) -> u8 {
    if !(0..=255).contains(&x) || !(0..=239).contains(&y) {
        return 3;
    }
    let world_x = x as u16 + r(0x40) as u16;
    let base = if world_x > 255 { 0x400 } else { 0x300 };
    let row = ((y as u8).wrapping_sub(r(0x44) & 7) & 248)
        .wrapping_add(r(0x4f))
        .wrapping_add(8);
    let index = row.wrapping_add((world_x as u8) >> 5);
    (r(base + index as u16) >> (6 - 2 * ((world_x & 31) >> 3))) & 3
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(
        |a| m.peek(a).expect("Jackal reads RAM only"),
        (m.ppu().completed_frames().saturating_sub(RESET_FRAMES) % 16) as u8,
    )
}
fn from_ram(r: impl Fn(u16) -> u8, phase: u8) -> Frame {
    let two_players = r(0x22) == 1;
    let level = r(0x30);
    let screen = r(0x43);
    let scroll = r(0x44);
    let camera_x = r(0x40);
    let paused = r(0x25) != 0;
    let players = std::array::from_fn(|i| {
        let i = i as u16;
        let state = r(0x530 + i);
        let lives = r(0x31 + i);
        let x = r(0x5b0 + i);
        let y = r(0x570 + i);
        Player {
            active: lives > 0,
            dead: lives == 0 || matches!(state, 2..=4),
            x,
            y,
            world_x: camera_x as u16 + x as u16,
            north: (screen as u16 * 240 + scroll as u16 + 240).saturating_sub(y as u16),
            lives,
            state,
            weapon: r(0x50 + i),
            facing: r(0x64 + i),
            invincibility: r(0x52 + i),
            grenade_active: r(0x538 + i) != 0,
            grenade_held: r(0xf1 + i) & 128 != 0,
        }
    });
    let terrain = std::array::from_fn(|i| {
        let p = &players[i];
        (-2..=2)
            .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| collision(&r, p.x as i16 + dx * 8, p.y as i16 + dy * 8))
            .collect()
    });
    let playing = r(0x18) == 5;
    let supported = two_players && level == 0 && (playing || r(0x18) == 7);
    Frame {
        supported,
        gameplay: supported && playing && !paused,
        stage_cleared: two_players && level == 1 && playing && players.iter().any(|p| p.active),
        two_players,
        paused,
        level,
        screen,
        scroll,
        camera_x,
        phase,
        boss_remaining: r(0x3b),
        players,
        terrain,
        enemies: (0..16u16)
            .filter(|i| r(0x500 + i) & 127 != 0 && r(0x520 + i) != 0)
            .map(|i| Enemy {
                slot: i as u8,
                kind: r(0x500 + i) & 127,
                state: r(0x520 + i),
                x: r(0x5a0 + i),
                y: r(0x560 + i),
                health: r(0x740 + i) & 127,
            })
            .collect(),
    }
}
impl Frame {
    pub fn progress(&self) -> u32 {
        self.screen as u32 * 16
            + self.scroll as u32 / 16
            + if self.boss_remaining <= 4 {
                (4 - self.boss_remaining) as u32 * 16
            } else {
                0
            }
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
            self.screen as f32 / 16.,
            self.scroll as f32 / 240.,
            self.camera_x as f32 / 255.,
            b(self.boss_remaining <= 4),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..21 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 255.,
                p.y as f32 / 239.,
                p.world_x as f32 / 512.,
                p.north as f32 / 4096.,
                p.lives as f32 / 8.,
                p.state as f32 / 8.,
                p.weapon as f32 / 8.,
                p.facing as f32 / 8.,
                p.invincibility as f32 / 255.,
                b(p.grenade_active),
                b(p.grenade_held),
                b(p.dead),
            ]);
        }
        let me = &self.players[slot];
        for e in &self.enemies {
            let start = 40 + e.slot as usize * 6;
            o[start..start + 6].copy_from_slice(&[
                1.,
                e.kind as f32 / 127.,
                (e.x as f32 - me.x as f32) / 255.,
                (e.y as f32 - me.y as f32) / 239.,
                e.health as f32 / 127.,
                e.state as f32 / 255.,
            ]);
        }
        for i in 0..2 {
            for (cell, v) in self.terrain[if i == 0 { slot } else { 1 - slot }]
                .iter()
                .enumerate()
            {
                o[136 + i * 25 + cell] = *v as f32 / 3.;
            }
        }
        o[186] = if self.boss_remaining <= 4 {
            self.boss_remaining as f32 / 4.
        } else {
            1.
        };
        for v in &mut o {
            *v = v.clamp(-1., 1.);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("Jackal task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [4, 0]),
        (30, [0, 0]),
        (1, [4, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
        (1, [1, 1]),
        (120, [0, 0]),
        (1, [1, 1]),
        (120, [0, 0]),
        (1, [1, 1]),
        (120, [0, 0]),
        (300, [0, 0]),
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
    fn victory_animation_stays_in_episode_but_does_not_run_policy() {
        let mut r = [0; 2048];
        r[0x22] = 1;
        r[0x18] = 7;
        r[0x31] = 2;
        r[0x530] = 1;
        let f = from_ram(|a| r[a as usize], 15);
        assert!(f.supported);
        assert!(!f.gameplay);
        assert!(!f.stage_cleared);
        r[0x18] = 6;
        assert!(!from_ram(|a| r[a as usize], 0).supported);
    }
    #[test]
    fn observations_keep_ports_and_scroll_adjusted_collision_separate() {
        let mut r = [0; 2048];
        r[0x22] = 1;
        r[0x18] = 5;
        r[0x31] = 2;
        r[0x32] = 1;
        r[0x530] = 1;
        r[0x531] = 3;
        r[0x5b0] = 32;
        r[0x5b1] = 224;
        r[0x570] = 64;
        r[0x571] = 128;
        r[0x40] = 48;
        r[0x44] = 5;
        r[0x4f] = 240;
        // x=32,y=64 -> world x=80, wrapped row=48, index=50, shift=2.
        r[0x332] = 3 << 2;
        let f = from_ram(|a| r[a as usize], 0);
        assert_eq!(f.terrain[0][12], 3);
        assert!(f.players[1].dead);
        let a = f.observation(0).unwrap();
        let b = f.observation(1).unwrap();
        assert_eq!(a[9], b[25]);
        assert_eq!(b[9], a[25]);
        assert!(a.iter().all(|v| v.is_finite() && (-1. ..=1.).contains(v)));
    }
    #[test]
    fn level_two_with_a_survivor_is_required() {
        let mut r = [0; 2048];
        r[0x22] = 1;
        r[0x18] = 5;
        r[0x31] = 5;
        r[0x32] = 5;
        r[0x530] = 1;
        r[0x531] = 1;
        let read = |r: &[u8; 2048]| from_ram(|a| r[a as usize], 0);
        assert!(read(&r).gameplay);
        assert!(!read(&r).stage_cleared);
        r[0x25] = 1;
        assert!(!read(&r).gameplay);
        r[0x25] = 0;
        r[0x3b] = 0;
        assert!(!read(&r).stage_cleared);
        r[0x30] = 1;
        assert!(read(&r).stage_cleared);
        assert!(!read(&r).gameplay);
        r[0x22] = 0;
        assert!(!read(&r).stage_cleared);
        r[0x22] = 1;
        r[0x31] = 0;
        r[0x32] = 0;
        assert!(!read(&r).stage_cleared);
        assert!(read(&r).observation(2).is_err());
    }
}
