"""Write garmin-mcp's wellness-v1 export of the local snapshot (personal data: run locally only).

    scripts/goldens.sh export_wellness.py [--out /w/.local/wellness-v1-mcp] [--snapshot /snapshot/garmin.duckdb]

Runs garmin-mcp's `WellnessExporter` from its checkout (mounted read-only at /gmcp by scripts/goldens.sh) on the
snapshot (mounted read-only at /snapshot). check_hrv.py and check_import.py read the result. The exporter is
incremental: files whose bytes did not change are left alone.
"""

import argparse
import json
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(ROOT / ".local" / "wellness-v1-mcp"))
    ap.add_argument("--snapshot", default="/snapshot/garmin.duckdb")
    ap.add_argument("--gmcp", default="/gmcp/src", help="garmin-mcp's source directory")
    ap.add_argument("--memory-limit", default="4GB")
    a = ap.parse_args()
    if not Path(a.snapshot).exists():
        print(f"no snapshot at {a.snapshot} (run through scripts/goldens.sh, which mounts it)")
        return 2
    sys.path.insert(0, a.gmcp)
    import garmin_mcp
    from garmin_mcp.wellness_export import WellnessExporter

    importer = f"garmin-mcp@{garmin_mcp.__version__}"
    t0 = time.time()
    res = WellnessExporter(Path(a.out), importer=importer, memory_limit=a.memory_limit).run(Path(a.snapshot))
    print(json.dumps(res, indent=1, default=str))
    print(f"wrote {a.out} in {time.time() - t0:.0f} s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
