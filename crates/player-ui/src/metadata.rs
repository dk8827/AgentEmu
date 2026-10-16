pub use session_engine::metadata::*;

pub struct Builtin {
    pub manifest: &'static str,
    pub weights: &'static [u8],
    pub notes: &'static str,
    pub experimental: bool,
}
pub fn builtins() -> Vec<Builtin> {
    macro_rules! model {
        ($folder:literal,$notes:literal,$experimental:literal) => {
            Builtin {
                manifest: include_str!(concat!("../../../models/", $folder, "/manifest.json")),
                weights: include_bytes!(concat!(
                    "../../../models/",
                    $folder,
                    "/weights.safetensors"
                )),
                notes: $notes,
                experimental: $experimental,
            }
        };
    }
    include!("builtin_models.in")
}
