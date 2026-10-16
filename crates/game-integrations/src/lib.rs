//! Revision-pinned game integrations and weights-only policies.
//! Memory addresses were researched from vermiceli/nes-contra-us; no game code/data is included.
pub mod adapter;
pub mod base;
pub mod bubble;
pub mod chip;
pub mod dd2;
pub mod ice;
pub mod jackal;
pub mod lifeforce;
pub mod policy;
pub mod rcr;
pub mod snow;
pub mod tasks;
pub mod tmnt;
pub mod worldcup;
pub use adapter::{Frame, ROM_SHA256, observe, validate_rom};
pub use policy::{Agents, Bundle};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub mod control;

pub mod registry;
#[rustfmt::skip]
pub mod contracts;

pub mod observations;
#[rustfmt::skip]
mod observation_contracts;

pub mod temporal;
