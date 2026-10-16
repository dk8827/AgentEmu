//! Compatibility facade. New clients should use game-integrations and session-engine.
pub use game_integrations::*;
pub use session_engine::episode;
pub mod tasks {
    pub use game_integrations::tasks::*;
    pub use session_engine::tasks::*;
}
