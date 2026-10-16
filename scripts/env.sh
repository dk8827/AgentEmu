#!/usr/bin/env sh
# Source from the repository root. Prefer the isolated toolchain when present.
if [ -x "$PWD/.tools/cargo/bin/rustup" ]; then
  export RUSTUP_HOME="$PWD/.tools/rustup"
  export CARGO_HOME="$PWD/.tools/cargo"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
