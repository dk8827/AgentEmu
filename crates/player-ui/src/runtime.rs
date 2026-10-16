//! Frontend transport only. Both implementations execute session-engine::Runtime.
use session_engine::realtime::{Input, Output, Setup, View};
pub struct Stopped {
    pub state: Vec<u8>,
    pub view: View,
    pub pixels: Vec<u16>,
    pub battery: Option<Vec<u8>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Starting,
    Running,
    Pausing,
}
#[cfg(not(target_arch = "wasm32"))]
#[path = "runtime_native.rs"]
mod host;
#[cfg(target_arch = "wasm32")]
#[path = "runtime_web.rs"]
mod host;
pub use host::Worker;
