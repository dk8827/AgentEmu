//! US Chip 'n Dale, normal two-player Zone 0 through the robot's fifth hit.
use crate::Result;
pub use crate::contracts::chip::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
pub const RESET_FRAMES: u64 = 2496;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub dead: bool,
    pub x: u16,
    pub y: u16,
    pub action: u8,
    pub health: u8,
    pub reserve_lives: u8,
    pub invincibility: u8,
    pub carrying: bool,
    pub carried_slot: u8,
    pub jump_held: bool,
    pub throw_held: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Object {
    pub slot: u8,
    pub present: bool,
    pub kind: u8,
    pub x: u16,
    pub y: u16,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub zone_cleared: bool,
    pub two_players: bool,
    pub paused: bool,
    pub area: u8,
    pub phase: u16,
    pub camera_y: u16,
    pub robot_hits: u8,
    pub robot_hit_cooldown: u8,
    pub players: [Player; 2],
    pub objects: [Object; 14],
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(
        |a| m.peek(a).expect("Chip adapter reads RAM only"),
        (m.ppu().completed_frames().saturating_sub(RESET_FRAMES) % 240) as u16,
    )
}
fn from_ram(r: impl Fn(u16) -> u8, phase: u16) -> Frame {
    let word = |lo, hi, i| r(lo + i) as u16 | (r(hi + i) as u16) << 8;
    // HUD life digits are installed by gameplay initialization. Sprite visibility
    // cannot gate life: both characters can be off-screen while still alive.
    let started = [0x5b4, 0x5e4]
        .into_iter()
        .any(|a| r(a) & 0xf0 == 0x80 && r(a + 2) & 0xf0 == 0x80);
    let players = std::array::from_fn(|slot| {
        let i = slot as u16;
        let action = r(0x54 + i);
        let active = started && action != 12;
        let hearts = if slot == 0 {
            [0x208, 0x20c, 0x210]
        } else {
            [0x21c, 0x220, 0x224]
        };
        let life = if slot == 0 { 0x5b4 } else { 0x5e4 };
        Player {
            active,
            dead: matches!(action, 10 | 12),
            x: word(0x4f0, 0x500, i),
            y: word(0x510, 0x520, i),
            action,
            health: if active {
                hearts.into_iter().filter(|a| r(*a) != 0xf8).count() as u8
            } else {
                0
            },
            reserve_lives: (r(life) & 15) * 10 + (r(life + 2) & 15),
            invincibility: r(0x5e + i),
            carrying: r(0x56 + i) & 128 != 0,
            carried_slot: r(0x56 + i) & 15,
            jump_held: r(0x16 + i) & 128 != 0,
            throw_held: r(0x16 + i) & 64 != 0,
        }
    });
    let area = r(0x6e);
    let two_players = r(0x50) == 1;
    let supported = two_players && started && matches!(area, 0 | 1 | 2 | 24);
    let paused = r(0x98) != 0;
    let robot_hits = r(0xee);
    // Entering the arena or reusing area zero is not a completion signal.
    // The fifth registered hit defeats this exact robot; the fixture also
    // verifies its subsequent Zone 0 clear message with ordinary inputs.
    let zone_cleared = supported
        && area == 24
        && robot_hits == 5
        && r(0xef) == 14
        && r(0x42e) == 114
        && players.iter().any(|p| p.active && !p.dead);
    Frame {
        supported,
        gameplay: supported && !paused && !zone_cleared && players.iter().any(|p| p.active),
        zone_cleared,
        two_players,
        paused,
        area,
        phase,
        camera_y: word(0xfa, 0xfb, 0),
        robot_hits,
        robot_hit_cooldown: r(0xed),
        players,
        objects: std::array::from_fn(|slot| {
            let i = slot as u16 + 2;
            Object {
                slot: i as u8,
                present: r(0x400 + i) & 128 != 0,
                kind: r(0x420 + i),
                x: word(0x4f0, 0x500, i),
                y: word(0x510, 0x520, i),
            }
        }),
    }
}
impl Frame {
    pub fn progress(&self) -> u32 {
        match self.area {
            24 => 50 + self.robot_hits.min(5) as u32,
            2 => 40,
            1 => {
                20 + self
                    .players
                    .iter()
                    .filter(|p| p.active)
                    .map(|p| p.x as u32 / 128)
                    .max()
                    .unwrap_or(0)
                    .min(19)
            }
            _ => self
                .players
                .iter()
                .filter(|p| p.active)
                .map(|p| 1024u32.saturating_sub(p.y as u32) / 64 + p.x as u32 / 256)
                .max()
                .unwrap_or(0)
                .min(19),
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
            self.phase as f32 / 239.,
            b(self.phase % 16 < 2),
            self.area as f32 / 24.,
            self.camera_y as f32 / 2048.,
            self.robot_hits.min(5) as f32 / 5.,
            b(self.phase % 80 < 35),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..22 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 2048.,
                p.y as f32 / 2048.,
                p.health as f32 / 3.,
                p.reserve_lives as f32 / 9.,
                p.action as f32 / 12.,
                b(p.dead),
                b(p.carrying),
                p.carried_slot as f32 / 15.,
                p.invincibility as f32 / 128.,
                b(p.jump_held),
                b(p.throw_held),
                b(p.action == 6),
                b(p.action == 0),
            ]);
        }
        let me = &self.players[slot];
        for (i, e) in self.objects.iter().enumerate() {
            o[40 + i * 12..46 + i * 12].copy_from_slice(&[
                b(e.present),
                e.kind as f32 / 127.,
                (e.x as f32 - me.x as f32) / 2048.,
                (e.y as f32 - me.y as f32) / 2048.,
                e.x as f32 / 2048.,
                e.y as f32 / 2048.,
            ]);
        }
        o[208] = self.robot_hit_cooldown as f32 / 60.;
        for v in &mut o {
            *v = v.clamp(-1., 1.);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("Chip task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [128, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
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
    let phase = crate::control::Phase::classify(
        r(0x214) == 0x67
            || ![0x5b4, 0x5e4]
                .into_iter()
                .any(|a| r(a) & 0xf0 == 0x80 && r(a + 2) & 0xf0 == 0x80),
        f.supported,
        f.gameplay,
    );
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
    fn robot_completion_requires_the_correct_fight_and_survivor() {
        let mut r = [0; 2048];
        r[0x5b4] = 0x80;
        r[0x5b6] = 0x82;
        r[0x400] = 208;
        r[0x401] = 208;
        r[0x50] = 1;
        r[0x6e] = 24;
        r[0x42e] = 114;
        r[0xef] = 14;
        let read = |r: &[u8; 2048]| from_ram(|a| r[a as usize], 0);
        for hits in 0..5 {
            r[0xee] = hits;
            assert!(!read(&r).zone_cleared);
        }
        r[0xee] = 5;
        assert!(read(&r).zone_cleared);
        assert!(!read(&r).gameplay);
        for (a, v) in [(0x50, 0), (0x6e, 0), (0x6e, 1), (0x42e, 36), (0xef, 2)] {
            let mut bad = r;
            bad[a] = v;
            assert!(!read(&bad).zone_cleared);
        }
        r[0x54] = 12;
        r[0x55] = 12;
        assert!(!read(&r).zone_cleared);
        assert_eq!(read(&r).players.each_ref().map(|p| p.health), [0, 0]);
        assert!(read(&r).observation(2).is_err());
        assert!(
            read(&r).observation(0).unwrap()[209..]
                .iter()
                .all(|v| *v == 0.)
        );
    }

    #[test]
    fn offscreen_players_and_respawns_are_not_game_over() {
        let mut r = [0; 2048];
        r[0x50] = 1;
        assert!(!from_ram(|a| r[a as usize], 0).gameplay);
        r[0x5b4] = 0x80;
        r[0x5b6] = 0x82;
        r[0x54] = 10;
        r[0x55] = 1;
        let f = from_ram(|a| r[a as usize], 0);
        assert!(f.gameplay);
        assert!(f.players.iter().all(|p| p.active));
        assert!(f.players[0].dead);
        assert!(!f.players[1].dead);
        r[0x98] = 1;
        assert!(!from_ram(|a| r[a as usize], 0).gameplay);
    }
}
