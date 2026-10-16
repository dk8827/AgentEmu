//! Developer-only homebrew fixture for the real-browser acceptance suite.
//! This is not a launch mode or an included game in AgentEmu.
use std::io::Write;

fn main() -> std::io::Result<()> {
    let path = std::env::args_os().nth(1).expect("output path required");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&nes_core::diagnostic::playground_rom())
}
