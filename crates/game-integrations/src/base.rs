//! Contra Base 1's observation contract includes wall objectives, health and room transitions.
pub use crate::contracts::base::{ARCHITECTURE, SCHEMA};
use crate::{Result, adapter};
use nes_core::machine::Machine;
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct Entity {
    pub slot: usize,
    pub kind: u8,
    pub x: u8,
    pub y: u8,
    pub vx: f32,
    pub vy: f32,
    pub hp: u8,
    pub requires_crouch: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub level: u8,
    pub room: u8,
    pub room_cleared: bool,
    pub barrier_removed: bool,
    pub boss_area: bool,
    pub boss_defeated: bool,
    pub transitioning: bool,
    pub advancing: [bool; 2],
    pub phase: u8,
    pub players: [adapter::Player; 2],
    pub entities: Vec<Entity>,
    #[serde(skip)]
    player_features: [Vec<f32>; 2],
}
pub fn observe(m: &Machine) -> Frame {
    let r = |a| m.peek(a).expect("Base adapter reads only RAM");
    let common = adapter::observe(m);
    let supported = r(0x30) == 1;
    let gameplay = supported && r(0x18) == 5 && r(0x2c) == 4 && r(0x1c) == 0 && r(0x25) == 0;
    let entities = (0..16u16)
        .filter(|i| r(0x4b8 + i) != 0)
        .map(|i| Entity {
            slot: i as usize,
            kind: r(0x528 + i),
            x: r(0x33e + i),
            y: r(0x324 + i),
            vx: r(0x508 + i) as i8 as f32 + r(0x518 + i) as f32 / 256.,
            vy: r(0x4e8 + i) as i8 as f32 + r(0x4f8 + i) as f32 / 256.,
            hp: r(0x578 + i),
            requires_crouch: r(0x598 + i) & 0x20 != 0,
        })
        .collect();
    Frame {
        supported,
        gameplay,
        level: r(0x30),
        room: r(0x64),
        room_cleared: r(0x37) != 0,
        barrier_removed: r(0x37) & 0x80 != 0,
        boss_area: r(0x40) & 0x80 != 0 && r(0x73) == 0,
        boss_defeated: r(0x3b) != 0,
        transitioning: r(0x73) != 0,
        advancing: [r(0xd0) != 0, r(0xd1) != 0],
        phase: r(0x1a) & 7,
        player_features: std::array::from_fn(|p| {
            common.observation(p).expect("valid port")[8..40].to_vec()
        }),
        players: common.players,
        entities,
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
            self.room as f32 / 8.,
            b(self.room_cleared),
            b(self.barrier_removed),
            b(self.boss_area),
            b(self.boss_defeated),
            b(self.players[1].active),
            b(self.transitioning),
            b(self.advancing[player]),
            b(self.advancing[1 - player]),
            b(self.supported),
            0.,
            0.,
        ];
        o.extend_from_slice(&self.player_features[player]);
        let me = &self.players[player];
        // Stable slot order keeps wall cores present even when no sprite is displayed.
        for slot in 0..16 {
            if let Some(e) = self.entities.iter().find(|e| e.slot == slot) {
                o.extend([
                    1.,
                    (e.x as f32 - me.x as f32) / 256.,
                    (e.y as f32 - me.y as f32) / 240.,
                    e.vx / 8.,
                    e.vy / 8.,
                    e.hp as f32 / 64.,
                    b(e.kind == 0x14),
                    b(e.kind == 0x13),
                    b(matches!(e.kind, 1 | 0x0b | 0x0f | 0x1b)),
                    b(e.kind == 0x11),
                    b(e.kind == 0x12),
                    b(matches!(e.kind, 0x0a | 0x10 | 0x1c)),
                    b(e.requires_crouch),
                ]);
            } else {
                o.extend([0.; 13])
            }
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
    let bytes = start.ok_or("Base 1 needs a locally prepared entry state")?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("entry state exceeds 4 MiB".into());
    }
    nes_core::state_file::restore(machine, bytes)?;

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
