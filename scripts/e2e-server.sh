#!/usr/bin/env bash
# Server-mode e2e: build the server, synth data, serve app/dist with a token, run server.spec.ts. Run through scripts/tc.
set -euo pipefail
cd "$(dirname "$0")/.."
export LAB_DATA_DIR=/w/.local/e2e-server-data LAB_TOKEN=e2e LAB_STATIC_DIR=/w/app/dist LAB_PORT=8091
rm -rf "$LAB_DATA_DIR"
cargo build -q -p wellness-lab-server
target/debug/wellness-lab-server synth --seed 3 --days 400 >/dev/null
target/debug/wellness-lab-server serve >/tmp/server.log 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do curl -sf http://localhost:8091/health >/dev/null && break; sleep 0.2; done
cd app
LAB_E2E_SERVER=http://localhost:8091 PW_BASE_URL=http://localhost:8091 npx playwright test e2e/server.spec.ts "$@"
