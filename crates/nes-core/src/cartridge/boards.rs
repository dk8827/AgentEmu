use serde::Deserialize;
use std::sync::LazyLock;
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Profile {
    prg_sha256: String,
    #[allow(dead_code)]
    note: String,
    pub mapper: Option<u8>,
    pub ignore_four_screen: bool,
    pub mmc6: bool,
    pub pal: bool,
    pub zapper: bool,
}
static PROFILES: LazyLock<Vec<Profile>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("boards.json")).expect("validated board database")
});
static UNKNOWN: LazyLock<Profile> = LazyLock::new(Profile::default);
pub(super) fn lookup(hash: &str) -> &'static Profile {
    PROFILES
        .iter()
        .find(|p| p.prg_sha256 == hash)
        .unwrap_or(&UNKNOWN)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrections_have_unique_program_identities_and_explanations() {
        let mut seen = std::collections::HashSet::new();
        for profile in PROFILES.iter() {
            assert_eq!(profile.prg_sha256.len(), 64);
            assert!(profile.prg_sha256.bytes().all(|b| b.is_ascii_hexdigit()));
            assert!(seen.insert(&profile.prg_sha256));
            assert!(!profile.note.is_empty());
        }
    }
}
