//! Portable save-slot records. ROM bytes are never persisted here.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct LocalSave {
    pub version: u8,
    pub cartridge: String,
    pub saved_at: String,
    pub frame: u64,
    state: String,
    pub preview: String,
}
impl LocalSave {
    pub fn new(
        cartridge: String,
        saved_at: String,
        frame: u64,
        state: &[u8],
        preview: &[u8],
    ) -> Self {
        Self {
            version: 1,
            cartridge,
            saved_at,
            frame,
            state: STANDARD.encode(state),
            preview: STANDARD.encode(preview),
        }
    }
    pub fn decode(text: &str, cartridge: &str) -> Result<Self, String> {
        if text.len() > 4 * 1024 * 1024 {
            return Err("Save slot is too large".into());
        }
        let save: Self = serde_json::from_str(text).map_err(|_| "Save slot could not be read")?;
        if save.version != 1 || save.cartridge != cartridge {
            return Err("Save slot belongs to a different cartridge or version".into());
        }
        save.bytes()?;
        Ok(save)
    }
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        STANDARD
            .decode(&self.state)
            .map_err(|_| "Save data is damaged".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slots_roundtrip_and_reject_wrong_cartridges_and_corruption() {
        let save = LocalSave::new("a".into(), "Today".into(), 123, &[1, 2, 3], &[]);
        let text = serde_json::to_string(&save).unwrap();
        assert_eq!(
            LocalSave::decode(&text, "a").unwrap().bytes().unwrap(),
            [1, 2, 3]
        );
        assert!(LocalSave::decode(&text, "b").is_err());
        assert!(LocalSave::decode(&text.replace("AQID", "!!!!"), "a").is_err());
        assert!(LocalSave::decode(&text.replace("\"version\":1", "\"version\":2"), "a").is_err());
    }
}
