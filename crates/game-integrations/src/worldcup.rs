//! USA's first Tournament 2P match, with the default Tony/Mark assignments.
use crate::Result;
pub use crate::contracts::worldcup::{ARCHITECTURE, ROM_SHA256, SCHEMA};
use nes_core::machine::Machine;
use serde::Serialize;
pub const RESET_FRAMES: u64 = 2294;
#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub active: bool,
    pub dead: bool,
    pub actor: usize,
    pub x: u16,
    pub depth: u16,
}
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub supported: bool,
    pub gameplay: bool,
    pub match_won: bool,
    pub match_finished: bool,
    pub paused: bool,
    pub second_half: bool,
    pub usa_score: u8,
    pub opponent_score: u8,
    pub seconds_remaining: u16,
    pub kickoff_wait: bool,
    pub phase: u16,
    pub players: [Player; 2],
    pub ball: [u16; 2],
    pub actors: [[u16; 2]; 12],
}
pub fn observe(m: &Machine) -> Frame {
    from_ram(
        |a| m.peek(a).expect("World Cup adapter reads RAM only"),
        (m.ppu().completed_frames().saturating_sub(RESET_FRAMES) % 480) as u16,
    )
}
fn from_ram(r: impl Fn(u16) -> u8, phase: u16) -> Frame {
    let position = |i: u16| {
        [
            r(0x4c + i) as u16 | (r(0x5a + i) as u16) << 8,
            r(0x76 + i) as u16 | (r(0x84 + i) as u16) << 8,
        ]
    };
    let first_match = r(0x511) == 1
        && r(0x512) == 0
        && r(0x514) == 0
        && r(0x515) == 4
        && r(0xe7) & 128 == 0
        && r(0x70d) == 4;
    let default_lineup = [4, 3, 0, 2, 1, 5]
        .into_iter()
        .enumerate()
        .all(|(i, v)| r(0x451 + i as u16) == v);
    let mode = first_match && default_lineup;
    let finished = mode && r(0xe8) & 4 != 0;
    let won = finished && r(0xe8) & 0x28 == 0x28 && r(0x517) > r(0x518);
    let supported = mode && !finished;
    let paused = r(0xe8) & 128 != 0;
    let player = |actor: u16| {
        let p = position(actor);
        Player {
            active: mode,
            dead: false,
            actor: actor as usize,
            x: p[0],
            depth: p[1],
        }
    };
    Frame {
        supported,
        gameplay: supported && !paused,
        match_won: won,
        match_finished: finished,
        paused,
        second_half: r(0xe8) & 32 != 0,
        usa_score: r(0x517),
        opponent_score: r(0x518),
        seconds_remaining: r(0x51b) as u16 * 60
            + (r(0x51a) >> 4) as u16 * 10
            + (r(0x51a) & 15) as u16,
        kickoff_wait: r(0x329) & 7 == 0,
        phase,
        players: [player(2), player(0)],
        ball: position(12),
        actors: std::array::from_fn(|i| position(i as u16)),
    }
}
impl Frame {
    pub fn observation(&self, slot: usize) -> Result<Vec<f32>> {
        if slot > 1 {
            return Err("player must be 0 or 1".into());
        }
        let b = |x: bool| x as u8 as f32;
        let mut o = vec![0.; 256];
        o[..8].copy_from_slice(&[
            slot as f32,
            b(self.gameplay),
            self.phase as f32 / 479.,
            b(self.phase % 16 < 4),
            b(self.second_half),
            b(self.kickoff_wait),
            self.seconds_remaining as f32 / 240.,
            b(self.phase % 120 < 4),
        ]);
        for (i, p) in [&self.players[slot], &self.players[1 - slot]]
            .iter()
            .enumerate()
        {
            o[8 + i * 16..14 + i * 16].copy_from_slice(&[
                b(p.active),
                p.x as f32 / 1024.,
                p.depth as f32 / 512.,
                (self.ball[0] as f32 - p.x as f32) / 1024.,
                (self.ball[1] as f32 - p.depth as f32) / 512.,
                p.actor as f32 / 11.,
            ]);
        }
        o[40..44].copy_from_slice(&[
            self.ball[0] as f32 / 1024.,
            self.ball[1] as f32 / 512.,
            self.usa_score as f32 / 99.,
            self.opponent_score as f32 / 99.,
        ]);
        let me = &self.players[slot];
        for (i, p) in self.actors.iter().enumerate() {
            o[48 + i * 2] = (p[0] as f32 - me.x as f32) / 1024.;
            o[49 + i * 2] = (p[1] as f32 - me.depth as f32) / 512.;
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
        return Err("World Cup uses normal USA Tournament 2P reset".into());
    }
    for (n, masks) in [
        (600, [0, 0]),
        (1, [128, 0]),
        (30, [0, 0]),
        (1, [8, 0]),
        (240, [0, 0]),
        (1, [8, 0]),
        (300, [0, 0]),
        (1, [1, 0]),
        (300, [0, 0]),
        (1, [1, 0]),
        (240, [0, 0]),
        (2, [0, 0]),
        (1, [1, 0]),
        (240, [0, 0]),
        (2, [0, 0]),
        (1, [1, 0]),
        (240, [0, 0]),
        (2, [0, 0]),
        (1, [1, 0]),
        (30, [0, 0]),
        (60, [0, 0]),
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
    let phase = crate::control::Phase::classify(r(0x70d) != 4, f.supported, f.gameplay);
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
    fn only_a_completed_winning_default_match_qualifies() {
        let mut r = [0; 2048];
        r[0x511] = 1;
        r[0x515] = 4;
        r[0x70d] = 4;
        r[0x451..0x457].copy_from_slice(&[4, 3, 0, 2, 1, 5]);
        let read = |r: &[u8; 2048]| from_ram(|a| r[a as usize], 0);
        assert!(read(&r).gameplay);
        r[0xe8] = 128;
        assert!(!read(&r).gameplay);
        r[0xe8] = 0;
        r[0x517] = 2;
        r[0xe8] = 0x28;
        assert!(!read(&r).match_won);
        r[0xe8] = 0x2c;
        assert!(read(&r).match_won);
        assert!(!read(&r).gameplay);
        r[0x518] = 2;
        assert!(!read(&r).match_won);
        r[0x518] = 3;
        assert!(!read(&r).match_won);
        r[0x518] = 0;
        for (a, v) in [
            (0x511, 0),
            (0x512, 1),
            (0x514, 7),
            (0x515, 1),
            (0xe7, 128),
            (0x70d, 0),
            (0x451, 3),
        ] {
            let mut altered = r;
            altered[a] = v;
            assert!(!read(&altered).match_won);
        }
        assert!(read(&r).observation(2).is_err());
        assert!(
            read(&r).observation(0).unwrap()[72..]
                .iter()
                .all(|v| *v == 0.)
        );
    }
}
