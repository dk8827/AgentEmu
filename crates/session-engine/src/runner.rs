//! One frame transaction for interactive play, training, and recorded inputs.
//! Hosts choose the source; they do not reimplement policy gating or arbitration.
use game_integrations::{
    Agents,
    control::{Decision, Phase, policy_action},
};
use nes_core::machine::Machine;

pub enum Input<'a> {
    Human {
        agents: &'a Agents,
        buttons: [u8; 2],
    },
    Policy {
        buttons: [u8; 2],
        gameplay: bool,
    },
    /// Exact recorded inputs, including menu buttons. Never run inference again.
    Replay([u8; 2]),
}
pub struct Frame {
    pub cycles: u64,
    pub audio: Vec<f32>,
    pub decisions: Option<[Decision; 2]>,
}
pub fn advance(machine: &mut Machine, input: Input<'_>) -> Result<Frame, String> {
    if let Some(e) = machine.fault() {
        return Err(format!(
            "Emulation stopped: {e}. Restart or open a valid save to continue."
        ));
    }
    let (buttons, decisions) = match input {
        Input::Human { agents, buttons } => {
            let decisions = agents
                .decisions(machine, buttons)
                .map_err(|e| e.to_string())?;
            if decisions
                .iter()
                .any(|d| d.phase == Phase::UnsupportedGameplay)
            {
                return Err("Agent does not support this stage or game mode. Take full control to continue.".into());
            }
            (decisions.each_ref().map(|d| d.applied), Some(decisions))
        }
        Input::Policy { buttons, gameplay } => {
            if buttons
                .iter()
                .any(|m| m & 12 != 0 || m & 48 == 48 || m & 192 == 192)
            {
                return Err(
                    "Policy input cannot contain menu buttons or opposing directions.".into(),
                );
            }
            (buttons.map(|a| policy_action(a, gameplay)), None)
        }
        Input::Replay(buttons) => (buttons, None),
    };
    let output = crate::console::advance(machine, buttons)?;
    Ok(Frame {
        cycles: output.cycles,
        audio: output.audio,
        decisions,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use nes_core::{cartridge::Cartridge, diagnostic::controller_rom};
    fn machine() -> Machine {
        Machine::new(Cartridge::from_ines(&controller_rom()).unwrap())
    }
    #[test]
    fn sources_match_on_valid_inputs_and_inactive_policy_is_neutral() {
        let mut human = machine();
        let mut policy = machine();
        let mut replay = machine();
        let agents = Agents::default();
        for buttons in [[0, 0], [129, 66], [18, 33], [0, 0]] {
            let a = advance(
                &mut human,
                Input::Human {
                    agents: &agents,
                    buttons,
                },
            )
            .unwrap();
            let b = advance(
                &mut policy,
                Input::Policy {
                    buttons,
                    gameplay: true,
                },
            )
            .unwrap();
            let c = advance(&mut replay, Input::Replay(buttons)).unwrap();
            assert_eq!(a.audio, b.audio);
            assert_eq!(b.audio, c.audio);
            assert_eq!(human.save_state(), policy.save_state());
            assert_eq!(policy.save_state(), replay.save_state());
        }
        advance(
            &mut policy,
            Input::Policy {
                buttons: [129, 66],
                gameplay: false,
            },
        )
        .unwrap();
        advance(&mut replay, Input::Replay([0, 0])).unwrap();
        assert_eq!(policy.save_state(), replay.save_state());
        let before = policy.save_state();
        assert!(
            advance(
                &mut policy,
                Input::Policy {
                    buttons: [8, 0],
                    gameplay: true
                }
            )
            .is_err()
        );
        assert_eq!(before, policy.save_state());
    }
}
