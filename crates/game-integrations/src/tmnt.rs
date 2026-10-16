//! US TMNT II scene-one adapter. Bodies occupy slots 0 and 2, not 0 and 1.
use crate::Result;
pub use crate::contracts::tmnt::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub dead: bool,
    pub x: u8,
    pub y: u8,
    pub ground_y: u8,
    pub health: u8,
    pub lives: u8,
    pub airborne: bool,
    pub attacking: bool,
    pub facing_left: bool,
    pub motion: u8,
    pub turtle: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub x: u8,
    pub y: u8,
    pub ground_y: u8,
    pub health: u8,
    pub motion: u8,
    pub facing_left: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub scene_cleared: bool,
    pub demo: bool,
    pub two_players: bool,
    pub paused: bool,
    pub screen: u8,
    pub core_state: u8,
    pub phase: u8,
    pub players: [Player; 2],
    pub entities: Vec<Entity>,
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(|a| m.peek(a).expect("TMNT adapter reads RAM only"))
}
fn from_ram(r: impl Fn(u16) -> u8) -> Frame {
    let screen = r(0x42);
    let core_state = r(0x3c);
    let demo = r(0x1f) != 0;
    let two_players = r(0x47) != 0;
    let paused = r(0x3d) != 0;
    let supported = !demo && screen <= 1;
    let gameplay = supported && core_state == 3 && !paused;
    let players = [0u16, 1].map(|p| {
        let i = p * 2;
        let lives = r(0x4d + p);
        let health = r(0x568 + i);
        Player {
            active: lives < 128 && (p == 0 || two_players),
            dead: lives >= 128 || (gameplay && health == 0),
            x: r(0x47e + i),
            y: r(0x46c + i),
            ground_y: r(0x4a2 + i),
            health,
            lives,
            airborne: r(0x412 + i) & 1 != 0,
            attacking: r(0x412 + i) & 2 != 0,
            facing_left: r(0x436 + i) & 64 != 0,
            motion: r(0x424 + i),
            turtle: r(0x33 + p),
        }
    });
    let entities = (7..18u16)
        .filter(|&i| r(0x400 + i) != 0 && (1..128).contains(&r(0x568 + i)))
        .map(|i| Entity {
            slot: i as usize,
            x: r(0x47e + i),
            y: r(0x46c + i),
            ground_y: r(0x4a2 + i),
            health: r(0x568 + i),
            motion: r(0x424 + i),
            facing_left: r(0x436 + i) & 64 != 0,
        })
        .collect();
    Frame {
        supported,
        gameplay,
        scene_cleared: !demo && screen == 2 && players.iter().any(|p| p.active),
        demo,
        two_players,
        paused,
        screen,
        core_state,
        phase: r(0x1b),
        players,
        entities,
    }
}
impl Frame {
    pub fn observation(&self, slot: usize) -> Result<Vec<f32>> {
        if slot > 1 {
            return Err("player must be 0 or 1".into());
        }
        let b = |v: bool| v as u8 as f32;
        let mut o = vec![0.; 256];
        o[..8].copy_from_slice(&[
            slot as f32,
            b(self.gameplay),
            self.phase as f32 / 255.,
            b(self.phase % 4 < 2),
            self.screen as f32 / 8.,
            self.entities.len() as f32 / 11.,
            b(self.players[1 - slot].active),
            b(self.phase % 36 < 2),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..21 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 255.,
                p.y as f32 / 255.,
                p.lives.min(127) as f32 / 127.,
                b(p.dead),
                b(p.airborne),
                p.ground_y as f32 / 255.,
                p.health as f32 / 255.,
                b(p.attacking),
                b(p.facing_left),
                p.motion as f32 / 255.,
                p.turtle as f32 / 3.,
                (p.ground_y as f32 - p.y as f32) / 255.,
            ]);
        }
        let me = &self.players[slot];
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|e| {
            (
                (e.x as i32 - me.x as i32).abs()
                    + 2 * (e.ground_y as i32 - me.ground_y as i32).abs(),
                e.slot,
            )
        });
        for (i, e) in entities.iter().enumerate() {
            o[40 + i * 12..48 + i * 12].copy_from_slice(&[
                1.,
                (e.x as f32 - me.x as f32) / 255.,
                (e.ground_y as f32 - me.ground_y as f32) / 255.,
                (e.y as f32 - me.y as f32) / 255.,
                e.health as f32 / 255.,
                e.motion as f32 / 255.,
                b(e.facing_left),
                e.slot as f32 / 17.,
            ]);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("TMNT task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [8, 8]),
        (240, [0, 0]),
        (1, [4, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (240, [0, 0]),
        (1, [1, 1]),
        (600, [0, 0]),
        (1, [1, 1]),
        (120, [0, 0]),
        (1, [1, 1]),
        (69, [0, 0]),
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
        crate::control::Phase::classify(r(0x3c) < 3 || r(0x1f) != 0, f.supported, f.gameplay);
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
    fn solo_ignores_unused_body_and_still_gates_demo_and_later_scenes() {
        let mut r = [0; 2048];
        r[0x3c] = 3;
        r[0x568] = 60;
        r[0x56a] = 60; // The real cartridge initializes unused P2 health/lives.
        r[0x4d] = 2;
        r[0x4e] = 2;
        let f = from_ram(|a| r[a as usize]);
        assert!(f.supported && f.gameplay && !f.two_players);
        assert_eq!(f.players.each_ref().map(|p| p.active), [true, false]);
        assert_eq!(f.observation(0).unwrap()[6], 0.);
        assert_eq!(f.observation(0).unwrap()[24], 0.);
        r[0x42] = 2;
        let f = from_ram(|a| r[a as usize]);
        assert!(f.scene_cleared && !f.supported && !f.gameplay);
        r[0x1f] = 1;
        assert!(!from_ram(|a| r[a as usize]).scene_cleared);
        r[0x1f] = 0;
        r[0x4d] = 255;
        assert!(!from_ram(|a| r[a as usize]).scene_cleared);
    }
    #[test]
    fn body_slots_and_scene_gate() {
        let mut r = [0; 2048];
        r[0x47] = 1;
        r[0x3c] = 3;
        r[0x568] = 60;
        r[0x56a] = 60;
        r[0x47e] = 24;
        r[0x480] = 64;
        r[0x47f] = 200;
        r[0x412] = 1;
        r[0x407] = 1;
        r[0x56f] = 20;
        r[0x485] = 100;
        let f = from_ram(|a| r[a as usize]);
        assert!(f.gameplay);
        assert!(!f.scene_cleared);
        assert_eq!(f.players[1].x, 64);
        assert!(f.players[0].airborne);
        let o = f.observation(0).unwrap();
        assert_eq!(o[41], 76. / 255.);
        assert!(o[172..].iter().all(|v| *v == 0.));
        assert!(f.observation(2).is_err());
        r[0x3d] = 1;
        assert!(!from_ram(|a| r[a as usize]).gameplay);
        r[0x3d] = 0;
        r[0x42] = 1;
        assert!(!from_ram(|a| r[a as usize]).scene_cleared);
        r[0x42] = 2;
        let f = from_ram(|a| r[a as usize]);
        assert!(f.scene_cleared);
        assert!(!f.gameplay);
        r[0x1f] = 1;
        assert!(!from_ram(|a| r[a as usize]).scene_cleared);
        r[0x1f] = 0;
        r[0x4d] = 255;
        r[0x4e] = 255;
        assert!(!from_ram(|a| r[a as usize]).scene_cleared);
    }
}
