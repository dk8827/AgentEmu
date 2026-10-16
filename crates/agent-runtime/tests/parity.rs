use agent_runtime::{Policy, decode};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixtures {
    players: Vec<Player>,
}
#[derive(Deserialize)]
struct Player {
    weights: String,
    observations: Vec<Vec<f32>>,
    logits: Vec<Vec<f32>>,
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/policies")
}

#[test]
fn pytorch_exports_agree_on_varied_observations_and_actions() {
    let data: Fixtures =
        serde_json::from_slice(&std::fs::read(root().join("parity.json")).unwrap()).unwrap();
    for player in data.players {
        let policy = Policy::load(&std::fs::read(root().join(player.weights)).unwrap()).unwrap();
        for (observation, expected) in player.observations.iter().zip(&player.logits) {
            let actual = policy.forward(observation).unwrap();
            for (a, e) in actual.iter().zip(expected) {
                assert!((a - e).abs() < 1e-5, "{a} != {e}");
            }
            assert_eq!(decode(&actual).unwrap(), decode(expected).unwrap());
        }
        assert!(policy.forward(&[0.; 31]).is_err());
        assert!(policy.forward(&[f32::INFINITY; 32]).is_err());
    }
}

#[test]
fn reject_invalid_tensors() {
    for name in [
        "invalid-nan.safetensors",
        "invalid-shape.safetensors",
        "invalid-extra.safetensors",
    ] {
        assert!(
            Policy::load(&std::fs::read(root().join(name)).unwrap()).is_err(),
            "{name}"
        );
    }
}
