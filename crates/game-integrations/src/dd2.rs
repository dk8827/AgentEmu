//! Normal / one-player or two-player-A Mission 1, pinned to the US cartridge.
use crate::Result;
pub use crate::contracts::dd2::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct Actor {
    pub slot: usize,
    pub present: bool,
    pub active: bool,
    pub dead: bool,
    pub x: u16,
    pub depth: u16,
    pub elevation: u16,
    pub health: u8,
    pub lives: u8,
    pub state: u8,
    pub kind: u8,
    pub facing_left: bool,
    pub climbing: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub mission_cleared: bool,
    pub two_players: bool,
    pub friendly_fire: bool,
    pub paused: bool,
    pub main_menu: bool,
    pub difficulty: u8,
    pub mission: u8,
    pub substage: u8,
    pub phase: u8,
    pub camera_x: u16,
    pub camera_y: u16,
    pub players: [Actor; 2],
    pub enemies: Vec<Actor>,
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(
        |a| m.peek(a).expect("Double Dragon adapter reads RAM only"),
        m.ppu().frame() as u8,
    )
}
fn from_ram(r: impl Fn(u16) -> u8, phase: u8) -> Frame {
    let two_players = r(0x34) & 128 != 0;
    let word = |lo, hi, i| r(lo + i) as u16 | (r(hi + i) as u16) << 8;
    let actor = |i: u16| {
        let present = r(0x3c + i) & 128 != 0;
        let state = r(0x43 + i) & 127;
        let kind = r(0xbf + i);
        let lives = if i < 2 { r(0x432 + i) } else { 0 };
        // Mission one's boss can revive from these zero-health poses. Hiding
        // it as a defeated enemy makes agents walk away and stall the fight.
        // Keep the existing death gates for players and ordinary enemies.
        let reviving_boss = i >= 2 && kind == 7 && matches!(state, 0x28 | 0x29);
        let dead = ((0x26..=0x29).contains(&state) && !reviving_boss) || (i < 2 && lives == 0);
        Actor {
            slot: i as usize,
            present,
            active: if i < 2 {
                lives > 0 && (i == 0 || two_players)
            } else {
                present && !dead
            },
            dead,
            x: word(0x77, 0x80, i),
            depth: word(0x92, 0x9b, i),
            elevation: word(0xad, 0xb6, i),
            health: r(0x41e + i),
            lives,
            state,
            kind,
            facing_left: r(0x63 + i) & 128 != 0,
            climbing: (3..=5).contains(&state),
        }
    };
    let players = [actor(0), actor(1)];
    let enemies = [actor(2), actor(3)]
        .into_iter()
        .filter(|e| e.active)
        .collect();
    let friendly_fire = r(0x34) & 64 != 0;
    let paused = r(0x35) & 128 != 0;
    let main_menu = r(0x35) & 32 != 0;
    let mission = r(0x422);
    let difficulty = r(0x435);
    let mode = !friendly_fire && !main_menu && difficulty == 1;
    let supported = mode && mission == 0;
    Frame {
        supported,
        gameplay: supported
            && !paused
            && r(0x35) & 2 == 0
            && players.iter().any(|p| p.present && p.active),
        mission_cleared: mode && mission == 1 && players.iter().any(|p| p.active),
        two_players,
        friendly_fire,
        paused,
        main_menu,
        difficulty,
        mission,
        substage: r(0x423),
        phase,
        camera_x: word(0xd0, 0xd1, 0),
        camera_y: word(0xd2, 0xd3, 0),
        players,
        enemies,
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
            self.substage as f32 / 24.,
            self.camera_x as f32 / 1024.,
            self.camera_y as f32 / 512.,
            b(self.phase % 36 < 2),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..21 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 1024.,
                p.depth as f32 / 256.,
                p.elevation as f32 / 512.,
                b(p.dead),
                b(p.present),
                p.health as f32 / 127.,
                p.lives as f32 / 8.,
                p.state as f32 / 127.,
                p.kind as f32 / 255.,
                b(p.facing_left),
                b(p.climbing),
                0.,
            ]);
        }
        let me = &self.players[slot];
        let mut enemies: Vec<_> = self.enemies.iter().collect();
        enemies.sort_by_key(|e| {
            (
                (e.x as i32 - me.x as i32).abs() + 2 * (e.depth as i32 - me.depth as i32).abs(),
                e.slot,
            )
        });
        for (i, e) in enemies.iter().enumerate() {
            o[40 + i * 12..49 + i * 12].copy_from_slice(&[
                1.,
                (e.x as f32 - me.x as f32) / 1024.,
                (e.depth as f32 - me.depth as f32) / 256.,
                (e.elevation as f32 - me.elevation as f32) / 512.,
                e.health as f32 / 127.,
                e.state as f32 / 127.,
                e.kind as f32 / 255.,
                b(e.facing_left),
                e.slot as f32 / 3.,
            ]);
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
        return Err("Double Dragon task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [8, 8]),
        (300, [0, 0]),
        (1, [32, 0]),
        (30, [0, 0]),
        (1, [0, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
        (1, [8, 8]),
        (33, [0, 0]),
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
        r(0x35) & 32 != 0
            || (r(0x422) == 0 && r(0x432) == 0 && r(0x433) == 0 && r(0x3c) == 0 && r(0x3d) == 0),
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
    fn solo_has_no_partner_and_cannot_clear_on_phantom_lives() {
        let mut r = [0; 2048];
        r[0x435] = 1;
        r[0x432] = 4;
        r[0x433] = 4; // Stale P2 counters must not make a partner exist.
        r[0x3c] = 128;
        let f = from_ram(|a| r[a as usize], 0);
        assert!(f.supported && f.gameplay && !f.two_players);
        assert_eq!(f.players.each_ref().map(|p| p.active), [true, false]);
        assert_eq!(f.observation(0).unwrap()[24], 0.);
        r[0x422] = 1;
        assert!(from_ram(|a| r[a as usize], 0).mission_cleared);
        r[0x432] = 0;
        assert!(!from_ram(|a| r[a as usize], 0).mission_cleared);
    }
    #[test]
    fn reviving_boss_remains_observable_until_it_disappears() {
        let mut r = [0; 2048];
        r[0x3e] = 129;
        r[0xc1] = 7;
        r[0x79] = 185;
        r[0x82] = 3;
        for state in [0x28, 0x29, 0x2a] {
            r[0x45] = state;
            let f = from_ram(|a| r[a as usize], 0);
            assert_eq!(f.enemies.len(), 1);
            assert!(f.enemies[0].active);
            assert!(!f.enemies[0].dead);
            assert_eq!(f.enemies[0].health, 0);
            let o = f.observation(0).unwrap();
            assert_eq!(o[40], 1.);
            assert_eq!(o[41], 953. / 1024.);
            assert_eq!(o[45], state as f32 / 127.);
        }
        r[0x3e] = 0;
        assert!(from_ram(|a| r[a as usize], 0).enemies.is_empty());
        r[0x3e] = 129;
        r[0xc1] = 1;
        r[0x45] = 0x28;
        assert!(from_ram(|a| r[a as usize], 0).enemies.is_empty());
        r[0x3c] = 128;
        r[0xbf] = 7;
        r[0x43] = 0x28;
        r[0x432] = 3;
        assert!(from_ram(|a| r[a as usize], 0).players[0].dead);
    }

    #[test]
    fn mode_lives_and_completion_gates() {
        let mut r = [0; 2048];
        r[0x34] = 128;
        r[0x435] = 1;
        r[0x432] = 1;
        r[0x433] = 2;
        r[0x3c] = 128;
        r[0x3d] = 128;
        let f = from_ram(|a| r[a as usize], 0);
        assert!(f.gameplay);
        assert!(!f.players[0].dead);
        assert!(!f.mission_cleared);
        r[0x43] = 0x27;
        assert!(from_ram(|a| r[a as usize], 0).players[0].dead);
        r[0x43] = 0;
        r[0x35] = 128;
        assert!(!from_ram(|a| r[a as usize], 0).gameplay);
        r[0x35] = 0;
        r[0x423] = 23;
        assert!(!from_ram(|a| r[a as usize], 0).mission_cleared);
        r[0x422] = 1;
        assert!(from_ram(|a| r[a as usize], 0).mission_cleared);
        for flags in [64, 192] {
            r[0x34] = flags;
            assert!(!from_ram(|a| r[a as usize], 0).mission_cleared);
        }
        r[0x34] = 128;
        r[0x435] = 2;
        assert!(!from_ram(|a| r[a as usize], 0).supported);
        r[0x435] = 1;
        r[0x432] = 0;
        r[0x433] = 0;
        assert!(!from_ram(|a| r[a as usize], 0).mission_cleared);
        let f = from_ram(|a| r[a as usize], 0);
        assert!(f.observation(2).is_err());
        assert!(f.observation(0).unwrap()[64..].iter().all(|v| *v == 0.));
    }
}
