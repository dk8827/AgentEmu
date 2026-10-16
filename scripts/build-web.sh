#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
. ./scripts/env.sh
cargo build --locked --release --target wasm32-unknown-unknown -p web-runtime
wasm-bindgen --target web --out-dir web/pkg --out-name emulator target/wasm32-unknown-unknown/release/web_runtime.wasm

sh scripts/build-player-ui.sh
