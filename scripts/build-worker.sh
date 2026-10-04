#!/usr/bin/env bash
# Build the Rust Worker to wasm (worker/build/). Installs the wasm target and
# worker-build when they are missing, so CI and Workers Builds need nothing
# but a Rust toolchain. worker-build must track the `worker` crate's minor
# line (0.8), or its bundled wasm-bindgen CLI will not match the crate.
set -euo pipefail
cd "$(dirname "$0")/../worker"

if ! command -v cargo >/dev/null 2>&1; then
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    echo "build-worker: no Rust toolchain; installing a minimal one" >&2
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
      sh -s -- -y --no-modify-path --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
fi

if command -v rustup >/dev/null 2>&1 &&
  ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  rustup target add wasm32-unknown-unknown
fi

if ! command -v worker-build >/dev/null 2>&1; then
  cargo install --locked "worker-build@^0.8"
fi

worker-build --release
