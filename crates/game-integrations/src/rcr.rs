//! Read-only River City Ransom US fields; objective: defeat Moose.
use crate::Result;
pub use crate::contracts::rcr::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub dead: bool,
    pub x: i16,
    pub depth: u8,
    pub height: u8,
    pub stamina: u8,
    pub max_stamina: u8,
    pub willpower: u8,
    pub strength: u8,
    pub motion: u8,
    pub animation: u8,
    pub facing_right: bool,
    pub punch_held: bool,
    pub kick_held: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub x: i16,
    pub depth: u8,
    pub height: u8,
    pub graphics: u8,
    pub motion: u8,
    pub animation: u8,
    pub stamina: u8,
    pub willpower: u8,
    pub kind: u8,
    pub enemy: bool,
    pub moose: bool,
    pub coin: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub started: bool,
    pub paused: bool,
    pub exit_code: u8,
    pub room: u8,
    pub clock: u8,
    pub moose_defeated: bool,
    pub scroll_x: u16,
    pub players: [Player; 2],
    pub entities: Vec<Entity>,
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(|a| m.peek(a).expect("RCR adapter reads RAM only"))
}
fn from_ram(r: impl Fn(u16) -> u8) -> Frame {
    let started = r(0x40) == 1;
    let room = r(0x42);
    let moose_defeated = started && r(0x45) & 8 != 0;
    let supported = room <= 5 && !moose_defeated;
    let paused = r(0x3f) != 0;
    // Nonzero exit codes can require continued walking; they are not a pause flag.
    let exit_code = r(0x36);
    let x = |i| i16::from_le_bytes([r(0x83 + i), r(0x8c + i)]);
    let players = [0u16, 1].map(|i| {
        // The cartridge clears bit 7 at actual defeat. Zero stamina alone is not death.
        let active = started && r(0x48 + i) & 128 != 0;
        Player {
            active,
            dead: started && !active,
            x: x(i),
            depth: r(0x9e + i),
            height: r(0xb9 + i),
            stamina: r(0x4bf + i),
            max_stamina: r(0x4c3 + i),
            willpower: r(0x4bb + i),
            strength: r(0x4b7 + i),
            motion: r(0x4f + i) & 127,
            animation: r(0xd3 + i) & 127,
            facing_right: r(0xd3 + i) & 128 != 0,
            punch_held: r(0xf0 + i) & 128 != 0,
            kick_held: r(0xf0 + i) & 64 != 0,
        }
    });
    let entities = (2..8u16)
        .filter(|i| r(0xcb + i) != 0)
        .map(|i| {
            let graphics = r(0xcb + i);
            let enemy = i < 4 && graphics == 2;
            let kind = if i < 4 { r(0x4f5 + i) } else { 0 };
            Entity {
                slot: i as usize,
                x: x(i),
                depth: r(0x9e + i),
                height: r(0xb9 + i),
                graphics,
                motion: r(0x4f + i) & 127,
                animation: r(0xd3 + i) & 127,
                stamina: if enemy { r(0x4bf + i) } else { 0 },
                willpower: if enemy { r(0x4bb + i) } else { 0 },
                kind,
                enemy,
                moose: enemy && kind == 9,
                coin: graphics == 16,
            }
        })
        .collect();
    Frame {
        supported,
        gameplay: supported && started && !paused && players.iter().any(|p| p.active),
        started,
        paused,
        exit_code,
        room,
        clock: r(0x52b),
        moose_defeated,
        scroll_x: u16::from_le_bytes([r(0xdc), r(0xdd)]),
        players,
        entities,
    }
}
impl Frame {
    pub fn progress(&self) -> u32 {
        let area = match self.room {
            0 => 0,
            1 => 1,
            2 => 2,
            3 | 4 => 3,
            _ => 4,
        };
        let x = self
            .players
            .iter()
            .filter(|p| p.active)
            .map(|p| p.x.max(0) as u32)
            .min()
            .unwrap_or(0);
        let boss = self
            .entities
            .iter()
            .find(|e| e.moose)
            .map(|e| 4 + (52u32.saturating_sub(e.stamina as u32) / 8))
            .unwrap_or(0);
        area * 4 + (x / 128).min(3) + boss
    }
    pub fn observation(&self, slot: usize) -> Result<Vec<f32>> {
        if slot > 1 {
            return Err("player must be 0 or 1".into());
        }
        let b = |v: bool| v as u8 as f32;
        let coordinate = |v: i16| (v as f32 / 1024.).clamp(-1., 1.);
        let mut o = vec![0.; 256];
        o[..8].copy_from_slice(&[
            slot as f32,
            b(self.gameplay),
            self.clock as f32 / 255.,
            b(self.clock & 2 != 0),
            (self.room as f32 / 34.).min(1.),
            b(self.moose_defeated),
            (self.scroll_x as f32 / 1024.).min(1.),
            b(self.players[1 - slot].active),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..24 + i * 16].copy_from_slice(&[
                b(p.active),
                coordinate(p.x),
                p.depth as f32 / 255.,
                p.stamina as f32 / 255.,
                b(p.dead),
                b(p.height == 0),
                p.height as f32 / 255.,
                p.motion as f32 / 127.,
                b(p.facing_right),
                p.animation as f32 / 127.,
                p.willpower as f32 / 255.,
                p.max_stamina as f32 / 255.,
                b(p.punch_held),
                b(p.kick_held),
                p.strength as f32 / 255.,
                0.,
            ]);
        }
        let p = &self.players[slot];
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|e| {
            (
                (e.x as i32 - p.x as i32).abs() + (e.depth as i32 - p.depth as i32).abs(),
                e.slot,
            )
        });
        for (i, e) in entities.iter().take(7).enumerate() {
            o[40 + i * 16..56 + i * 16].copy_from_slice(&[
                1.,
                ((e.x as f32 - p.x as f32) / 1024.).clamp(-1., 1.),
                (e.depth as f32 - p.depth as f32) / 255.,
                e.height as f32 / 255.,
                e.stamina as f32 / 255.,
                e.willpower as f32 / 255.,
                b(e.enemy),
                b(e.moose),
                b(e.coin),
                e.graphics as f32 / 255.,
                e.kind as f32 / 255.,
                e.motion as f32 / 127.,
                e.animation as f32 / 127.,
                e.slot as f32 / 8.,
                0.,
                0.,
            ]);
        }
        crate::observations::validate(SCHEMA, o)
    }
}
/// Versioned task startup using ordinary controller input or a validated entry state.
pub(crate) fn initialize(machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
    if start.is_some() {
        return Err("RCR task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [32, 0]),
        (30, [0, 0]),
        (1, [8, 8]),
        (120, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
        (1, [8, 8]),
        (600, [0, 0]),
        (1, [1, 1]),
        (120, [0, 0]),
        (1, [1, 1]),
        (120, [0, 0]),
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
    let phase = crate::control::Phase::classify(r(0x40) != 1, f.supported, f.gameplay);
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
    fn goal_requires_real_game_and_moose_flag() {
        let mut r = [0u8; 2048];
        r[0x45] = 8;
        assert!(!from_ram(|a| r[a as usize]).moose_defeated);
        r[0x40] = 1;
        assert!(from_ram(|a| r[a as usize]).moose_defeated);
        r[0x45] = 16;
        assert!(!from_ram(|a| r[a as usize]).moose_defeated);
    }
    #[test]
    fn zero_stamina_and_empty_enemy_list_do_not_prove_defeat() {
        let mut r = [0u8; 2048];
        r[0x40] = 1;
        r[0x48] = 128;
        r[0x49] = 129;
        let f = from_ram(|a| r[a as usize]);
        assert!(f.gameplay);
        assert!(!f.moose_defeated);
        assert!(!f.players[0].dead);
        r[0x48] = 0;
        assert!(from_ram(|a| r[a as usize]).players[0].dead);
    }
    #[test]
    fn pause_and_out_of_scope_rooms_disable_neural_play() {
        let mut r = [0u8; 2048];
        r[0x40] = 1;
        r[0x48] = 128;
        for (addr, v) in [(0x3f, 160), (0x42, 6)] {
            r[addr] = v;
            assert!(!from_ram(|a| r[a as usize]).gameplay);
            r[addr] = 0;
        }
    }
    #[test]
    fn exit_code_does_not_strand_players_at_room_boundary() {
        let mut r = [0u8; 2048];
        r[0x40] = 1;
        r[0x48] = 128;
        r[0x42] = 2;
        r[0x36] = 24;
        assert!(from_ram(|a| r[a as usize]).gameplay);
    }
    #[test]
    fn observations_are_bounded_and_keep_players_separate() {
        let mut r = [255u8; 2048];
        r[0x83] = 40;
        r[0x84] = 80;
        r[0x8c] = 0;
        r[0x8d] = 0;
        let f = from_ram(|a| r[a as usize]);
        let a = f.observation(0).unwrap();
        let b = f.observation(1).unwrap();
        assert_ne!(a[9], b[9]);
        assert!(
            a.iter()
                .chain(b.iter())
                .all(|v| v.is_finite() && (-1. ..=1.).contains(v))
        );
        assert!(f.observation(2).is_err());
    }
}
