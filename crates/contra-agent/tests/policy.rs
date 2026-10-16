use contra_agent::Bundle;
use serde_json::Value;
const MANIFEST: &str = include_str!("../../../fixtures/contra-policy/manifest.json");
const WEIGHTS: &[u8] = include_bytes!("../../../fixtures/contra-policy/weights.safetensors");
#[test]
fn pytorch_parity_and_manifest_rejection() {
    let actor = Bundle::load(MANIFEST, WEIGHTS).unwrap();
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/contra-policy/parity.json")).unwrap();
    for c in cases.as_array().unwrap() {
        let o: Vec<f32> = serde_json::from_value(c["observation"].clone()).unwrap();
        for (actual, expected) in actor
            .logits(&o)
            .unwrap()
            .iter()
            .zip(c["logits"].as_array().unwrap())
        {
            assert!((*actual - expected.as_f64().unwrap() as f32).abs() < 2e-5);
        }
        assert_eq!(
            actor.buttons(&o).unwrap(),
            c["buttons"].as_u64().unwrap() as u8
        );
    }
    for (field, value) in [
        ("decision_frames", serde_json::json!(4)),
        ("observation", serde_json::json!("wrong")),
        ("rom_sha256", serde_json::json!("wrong")),
        ("architecture", serde_json::json!("arbitrary-network")),
        ("weights_sha256", serde_json::json!("wrong")),
    ] {
        let mut m: Value = serde_json::from_str(MANIFEST).unwrap();
        m[field] = value;
        assert!(Bundle::load(&m.to_string(), WEIGHTS).is_err());
    }
    assert!(Bundle::load(&" ".repeat(16385), WEIGHTS).is_err());
    assert!(actor.logits(&[f32::NAN; 256]).is_err());
    assert!(actor.logits(&[0.; 32]).is_err());
    let mut damaged = WEIGHTS.to_vec();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(Bundle::load(MANIFEST, &damaged).is_err());
}
