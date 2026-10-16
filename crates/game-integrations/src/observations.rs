//! Named, revision-pinned feature layouts live beside each task definition.
//! Validation occurs at the encoder boundary for training and inference alike.
pub struct Schema {
    pub id: &'static str,
    pub reserved: [u64; 4],
    pub json: &'static str,
}
pub fn schema(id: &str) -> Option<&'static Schema> {
    crate::observation_contracts::SCHEMAS
        .iter()
        .find(|s| s.id == id)
}
pub(crate) fn validate(id: &str, values: Vec<f32>) -> crate::Result<Vec<f32>> {
    let layout = schema(id).ok_or("Unknown observation schema")?;
    if values.len() != 256 {
        return Err("Observation width differs from its schema".into());
    }
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() || (layout.reserved[i / 64] & (1 << (i % 64)) != 0 && v != 0.) {
            return Err(format!("Observation {id} violates its contract at feature {i}").into());
        }
    }
    Ok(values)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_task_has_a_named_schema_and_rejects_nonfinite_or_reserved_data() {
        for task in crate::registry::all() {
            let layout = schema(task.observation).unwrap();
            let json: serde_json::Value = serde_json::from_str(layout.json).unwrap();
            assert_eq!(json["schema"], task.observation);
            assert!(validate(task.observation, vec![0.; 256]).is_ok());
            assert!(validate(task.observation, vec![0.; 255]).is_err());
            for i in 0..256 {
                let mut values = vec![0.; 256];
                values[i] = f32::NAN;
                assert!(validate(task.observation, values).is_err());
                if layout.reserved[i / 64] & (1 << (i % 64)) != 0 {
                    let mut values = vec![0.; 256];
                    values[i] = 1.;
                    assert!(validate(task.observation, values).is_err());
                }
            }
        }
    }
}
