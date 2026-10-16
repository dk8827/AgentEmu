//! Shared input arbitration for native, WASM and training clients.
use serde::{Deserialize, Serialize};

pub const GROUPS: [u8; 4] = [0xc0, 0x30, 0x01, 0x02];
pub const GAMEPLAY: u8 = 0xf3;
pub const MENU: u8 = 0x0c;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Menu,
    Transition,
    SupportedGameplay,
    UnsupportedGameplay,
}
impl Phase {
    pub fn classify(menu: bool, supported: bool, gameplay: bool) -> Self {
        if gameplay && supported {
            Self::SupportedGameplay
        } else if menu {
            Self::Menu
        } else if !supported {
            Self::UnsupportedGameplay
        } else {
            Self::Transition
        }
    }
}

pub fn valid_ownership(mask: u8) -> bool {
    mask & MENU == 0 && GROUPS.into_iter().all(|g| mask & g == 0 || mask & g == g)
}
pub fn normalize(mut mask: u8) -> u8 {
    for pair in [0xc0, 0x30] {
        if mask & pair == pair {
            mask &= !pair;
        }
    }
    mask
}
/// Each group has exactly one owner. Start/Select can never come from a policy.
pub fn arbitrate(human: u8, proposal: u8, ai_groups: u8) -> u8 {
    normalize((human & !ai_groups) | (proposal & ai_groups & GAMEPLAY))
}
/// A training action is neutral whenever its policy is inactive.
pub fn policy_action(action: u8, gameplay: bool) -> u8 {
    if gameplay {
        normalize(action & GAMEPLAY)
    } else {
        0
    }
}
/// Shared ABI: versioned game features, eight normalized human button bits in NES
/// order, then ownership bits for Left/Right, Up/Down, A and B. Start/Select
/// remain human-owned. The same encoder is exposed directly to Python.
pub fn assistance_features(
    observation: &[f32],
    human: u8,
    ownership: u8,
) -> crate::Result<Vec<f32>> {
    if ![256, 272, 296].contains(&observation.len())
        || observation.iter().any(|x| !x.is_finite())
        || !valid_ownership(ownership)
    {
        return Err("invalid assistance observation or ownership".into());
    }
    let mut result = Vec::with_capacity(observation.len() + 12);
    result.extend_from_slice(observation);
    let human = normalize(human);
    result.extend((0..8).map(|b| ((human >> b) & 1) as f32));
    result.extend(GROUPS.map(|g| u8::from(ownership & g == g) as f32));
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub human: u8,
    pub proposal: Option<u8>,
    pub applied: u8,
    pub ai_groups: u8,
    pub phase: Phase,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_group_has_exclusive_ownership_and_menu_is_always_human() {
        for owners in 0..16 {
            let mask = GROUPS
                .iter()
                .enumerate()
                .filter(|(i, _)| owners & (1 << i) != 0)
                .fold(0, |a, (_, g)| a | g);
            assert!(valid_ownership(mask));
            for human in 0..=255 {
                for ai in 0..=255 {
                    let applied = arbitrate(human, ai, mask);
                    assert_eq!(applied & MENU, human & MENU);
                    for g in GROUPS {
                        let owner = if mask & g != 0 { ai } else { human };
                        assert_eq!(applied & g, normalize(owner) & g);
                    }
                }
            }
        }
        for mask in [4, 8, 16, 32, 64, 128, 255] {
            assert!(!valid_ownership(mask));
        }
    }
    #[test]
    fn phase_distinguishes_menu_transition_and_unsupported_play() {
        assert_eq!(Phase::classify(true, false, false), Phase::Menu);
        assert_eq!(Phase::classify(false, true, false), Phase::Transition);
        assert_eq!(Phase::classify(false, true, true), Phase::SupportedGameplay);
        assert_eq!(
            Phase::classify(false, false, false),
            Phase::UnsupportedGameplay
        );
        assert_eq!(policy_action(255, false), 0);
        assert_eq!(policy_action(143, true), 131);
    }
}
