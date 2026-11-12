#!/usr/bin/env bash
# Serve the built app and run Playwright specs against it in one container. Usage (through scripts/tc): scripts/e2e.sh [playwright args]
set -euo pipefail
cd "$(dirname "$0")/../app"
PORT="${PW_PORT:-4173}"
npx vite preview --port "$PORT" --strictPort >/tmp/preview.log 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do curl -sf "http://localhost:$PORT/" >/dev/null && break; sleep 0.2; done
PW_BASE_URL="http://localhost:$PORT" npx playwright test "$@"
