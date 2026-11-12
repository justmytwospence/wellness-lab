#!/usr/bin/env bash
# Run a tools/goldens script against the local garmin-mcp snapshot (read-only) and garmin-mcp sources.
# usage: scripts/goldens.sh <script.py> [args...]
set -euo pipefail
cd "$(dirname "$0")/.."
script=$1; shift
export TC_MOUNTS="-v $HOME/Projects/homelab/garmin-mcp/data/snapshot:/snapshot:ro -v $HOME/Projects/garmin-mcp:/gmcp:ro ${TC_MOUNTS:-}"
exec scripts/tc bash -c "cd tools/goldens && uv run python $script $*"
