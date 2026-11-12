"""Record the notebook's outputs as goldens: `make_goldens.py --version 1`. Goldens are frozen: an existing version is never overwritten.

Reads the garmin-mcp snapshot at /snapshot/garmin.duckdb (scripts/goldens.sh mounts it) and writes
.local/goldens/hrv-v<version>/ at the repository root. Personal data: never committed.
"""

import argparse
import json
import sys
from pathlib import Path

import polars as pl

import reference_hrv as ref

ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", required=True)
    ap.add_argument("--snapshot", default="/snapshot/garmin.duckdb")
    ap.add_argument("--parts", default="hrv", help="comma list: hrv, a1, beats")
    a = ap.parse_args()
    out = ROOT / ".local" / "goldens" / f"hrv-v{a.version}"
    if out.exists():
        print(f"goldens v{a.version} exist; bump --version", file=sys.stderr)
        return 1
    out.mkdir(parents=True)
    parts = set(a.parts.split(","))
    if "hrv" in parts:
        r = ref.run_all(a.snapshot)
        for k, v in r.items():
            if isinstance(v, pl.DataFrame):
                v.write_parquet(out / f"{k}.parquet")
        (out / "MODEL.json").write_text(json.dumps(r["MODEL"]))
        (out / "LOAD_CUTS.json").write_text(json.dumps(r["LOAD_CUTS"]))
        print("hrv:", {k: (v.shape if isinstance(v, pl.DataFrame) else v) for k, v in r.items() if k != "MODEL"})
    if "a1" in parts or "beats" in parts:
        import reference_a1 as ra

        ra.record(a.snapshot, out, parts)
    return 0


if __name__ == "__main__":
    sys.exit(main())
