# JavaScript reference tests

These small reference implementations preserve regression cases from the former
HTML player. They are not shipped, served, or used by the current Rust interface.
They test the historical JS behavior only. Production coverage lives in Rust
unit tests, the WASM runtime tests and `scripts/test-rust-ui.mjs`.

The version-one replay validator also supports the local replay verification
script. Generated task IDs come from `integrations/`. Do not add application
features here; change the Rust session engine and UI.
