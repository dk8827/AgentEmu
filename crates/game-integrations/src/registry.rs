//! Shared, generated contract registry. Executable decoders stay in game modules.
pub type PolicyObservation = (crate::control::Phase, Option<Vec<f32>>);
#[derive(Debug)]
pub struct PolicyVariant {
    pub architecture: &'static str,
    pub observation: &'static str,
    pub inputs: usize,
    pub assistance: bool,
    pub features: &'static str,
}
#[derive(Debug)]
pub struct Definition {
    pub policy_observation:
        fn(&nes_core::machine::Machine, usize) -> crate::Result<PolicyObservation>,
    pub variants: &'static [PolicyVariant],
    pub id: &'static str,
    pub architecture: &'static str,
    pub observation: &'static str,
    pub assistance_architecture: &'static str,
    pub assistance_observation: &'static str,
    pub rom_sha256: &'static str,
    pub reset: &'static str,
    pub solo_reset: Option<&'static str>,
    pub solo_menu: &'static [(u32, [u8; 2])],
    pub scope: &'static str,
    pub actions: [&'static str; 2],
}
pub fn all() -> &'static [Definition] {
    crate::contracts::TASKS
}
pub fn task(id: &str) -> Option<&'static Definition> {
    all().iter().find(|t| t.id == id)
}
pub fn schema(id: &str) -> Option<&'static Definition> {
    all().iter().find(|t| {
        t.observation == id
            || t.assistance_observation == id
            || t.variants.iter().any(|v| v.observation == id)
    })
}

pub(crate) fn run_reset(
    m: &mut nes_core::machine::Machine,
    masks: [u8; 2],
    frames: u32,
) -> crate::Result<()> {
    m.set_controllers(masks);
    for _ in 0..frames {
        m.run_frame()?;
        m.take_audio();
    }
    Ok(())
}

pub fn variant(id: &str) -> Option<&'static PolicyVariant> {
    schema(id)?.variants.iter().find(|v| v.observation == id)
}
/// Return game features for the declared policy, before optional human context.
pub fn policy_observation(
    m: &nes_core::machine::Machine,
    slot: usize,
    schema_id: &str,
) -> crate::Result<PolicyObservation> {
    let d = schema(schema_id).ok_or("unknown policy schema")?;
    let (phase, observation) = (d.policy_observation)(m, slot)?;
    let observation = observation
        .map(|o| {
            if let Some(v) = variant(schema_id) {
                crate::temporal::extend(m, slot, &o, v.features)
            } else {
                Ok(o)
            }
        })
        .transpose()?;
    Ok((phase, observation))
}
/// Context encoding is shared by training, native execution and WASM.
pub fn policy_features(
    m: &nes_core::machine::Machine,
    slot: usize,
    schema_id: &str,
    human: u8,
    ownership: u8,
) -> crate::Result<Vec<f32>> {
    let d = schema(schema_id).ok_or("unknown policy schema")?;
    if m.cartridge_hash() != d.rom_sha256 {
        return Err("policy ROM mismatch".into());
    }
    let (_, observation) = policy_observation(m, slot, schema_id)?;
    let o = observation.ok_or("policy inactive in this phase")?;
    if schema_id == d.assistance_observation || variant(schema_id).is_some_and(|v| v.assistance) {
        crate::control::assistance_features(&o, human, ownership)
    } else {
        Ok(o)
    }
}
