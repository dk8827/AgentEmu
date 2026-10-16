#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
. ./scripts/env.sh
cargo build --locked --release --target wasm32-unknown-unknown -p player-ui
wasm-bindgen --target web --out-dir web/pkg --out-name player_ui target/wasm32-unknown-unknown/release/player_ui.wasm
