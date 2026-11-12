#!/usr/bin/env bash
# Build the browser engine (wasm) for the app and the node build for the cross-binding check.
# Run through scripts/tc.
set -euo pipefail
cd "$(dirname "$0")/.."
# SIMD128: every browser since 2021 runs it; wasm-opt is told in the crate's Cargo.toml
export RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+simd128"
rm -rf packages/engine/pkg .local/wasm-node
wasm-pack build crates/wasm --release --target web --out-dir ../../packages/engine/pkg --out-name wellness_lab
wasm-pack build crates/wasm --release --target nodejs --out-dir ../../.local/wasm-node --out-name wellness_lab
ls -l packages/engine/pkg/wellness_lab_bg.wasm
