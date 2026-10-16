#!/usr/bin/env sh
# Build the two runners used by commercial-cartridge parity checks.
set -eu
cd "$(dirname "$0")/.."
. ./scripts/env.sh
cargo build --locked --release -p nes-cli
cargo build --locked --release --target wasm32-unknown-unknown -p web-runtime
wasm-bindgen --target web --out-dir web/pkg --out-name emulator target/wasm32-unknown-unknown/release/web_runtime.wasm
