#!/usr/bin/env bash
# Build the Python wheel and install it into the goldens environment. Run through scripts/tc.
set -euo pipefail
cd "$(dirname "$0")/.."
maturin build --release -m crates/py/Cargo.toml -o .local/wheels
(cd tools/goldens && uv sync -q)
uv pip install --python tools/goldens/.venv/bin/python .local/wheels/wellness_lab_engine-*.whl --reinstall
