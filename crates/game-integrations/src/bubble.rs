//! Pinned Bubble Bobble round-one observations, verified by independent controller traces.
use crate::Result;
pub use crate::contracts::bubble::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub x: u8,
    pub y: u8,
    pub lives: u8,
    pub dead: bool,
    pub airborne: bool,
    pub invincible: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub x: u8,
    pub y: u8,
    pub state: u8,
    pub motion: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub round: u8,
    pub transition: u8,
    pub phase: u8,
    pub players: [Player; 2],
    pub entities: Vec<Entity>,
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(|a| m.peek(a).expect("Bubble adapter reads RAM only"))
}
fn from_ram(r: impl Fn(u16) -> u8) -> Frame {
    let round = r(0x401);
    let players = [0u16, 1].map(|p| {
        let b = 0x2e + p * 20;
        Player {
            active: r(b) > 0,
            lives: r(b),
            x: r(0x203 + p * 8),
            y: r(0x200 + p * 8),
            dead: r(b) == 0 || r(b + 17) & 0x80 != 0,
            airborne: r(b + 3) != 0,
            invincible: r(b + 17) != 0 && r(b + 17) & 0x80 == 0,
        }
    });
    // Game routines iterate seven four-byte records at $68, paired with
    // two-sprite positions at $210 + 8*slot. A zero record is unoccupied.
    let entities = (0..7u16)
        .filter(|p| r(0x68 + p * 4) != 0 && r(0x210 + p * 8) < 240)
        .map(|p| Entity {
            slot: p as usize,
            x: r(0x213 + p * 8),
            y: r(0x210 + p * 8),
            state: r(0x6a + p * 4),
            motion: r(0x69 + p * 4),
        })
        .collect();
    Frame {
        // Round briefly becomes zero while its completion routine switches banks.
        // Treat that interval as a neutral transition, not a task failure/handoff.
        supported: round <= 1,
        gameplay: round == 1 && r(0x400) == 0 && players.iter().any(|p| p.active),
        round,
        transition: r(0x400),
        phase: r(0x1f),
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
            b(self.phase & 2 != 0),
            self.round as f32 / 100.,
            self.entities.len() as f32 / 7.,
            b(self.players[1 - slot].active),
            b(self.phase % 40 < 20),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..16 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 255.,
                p.y as f32 / 240.,
                p.lives as f32 / 10.,
                b(p.dead),
                b(p.airborne),
                b(p.invincible),
                0.,
            ]);
        }
        let me = &self.players[slot];
        let mut entities: Vec<_> = self.entities.iter().collect();
        entities.sort_by_key(|e| {
            (
                (e.x as i32 - me.x as i32).abs() + (e.y as i32 - me.y as i32).abs(),
                e.slot,
            )
        });
        for (i, e) in entities.iter().enumerate() {
            o[40 + i * 8..48 + i * 8].copy_from_slice(&[
                1.,
                (e.x as f32 - me.x as f32) / 256.,
                (e.y as f32 - me.y as f32) / 240.,
                e.state as f32 / 255.,
                e.motion as f32 / 255.,
                0.,
                0.,
                0.,
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
        return Err("Bubble task uses normal two-player reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [8, 8]),
        (240, [0, 0]),
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
    let phase = crate::control::Phase::classify(
        r(0x401) == 0 && r(0x2e) == 0 && r(0x42) == 0,
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
    fn round_gate_slots_and_observation_mask() {
        let mut ram = [0u8; 2048];
        ram[0x401] = 1;
        ram[0x2e] = 3;
        ram[0x42] = 2;
        ram[0x203] = 24;
        ram[0x20b] = 216;
        ram[0x68] = 1;
        ram[0x210] = 100;
        ram[0x213] = 80;
        let f = from_ram(|a| ram[a as usize]);
        assert!(f.gameplay);
        assert_eq!(f.players[1].x, 216);
        assert_eq!(f.entities.len(), 1);
        let o = f.observation(0).unwrap();
        assert_eq!(o.len(), 256);
        assert_eq!(o[41], 56. / 256.);
        assert!(o[48..].iter().all(|&v| v == 0.));
        assert!(f.observation(2).is_err());
        ram[0x401] = 2;
        assert!(!from_ram(|a| ram[a as usize]).supported);
        ram[0x401] = 1;
        ram[0x400] = 2;
        assert!(!from_ram(|a| ram[a as usize]).gameplay);
        ram[0x401] = 0;
        ram[0x400] = 0;
        let transition = from_ram(|a| ram[a as usize]);
        assert!(transition.supported);
        assert!(!transition.gameplay);
    }
}
