"""Check a wellness-v1 directory against the data contract (schema/wellness-v1.json).

    uv run python check_schema_files.py <dir>

DuckDB DESCRIBEs every Parquet file; its column names, order and types must equal the schema's entry for the file
(`<table>.parquet`, `series/<name>/<YYYY-MM>.parquet`, `rr/<id>.parquet`). The manifest must list exactly the files
present, with their row counts and sha256. Prints a pass/fail table; exit 1 on any failure.
"""

import hashlib
import json
import re
import sys
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads((ROOT / "schema" / "wellness-v1.json").read_text())


def expected(rel: str):
    """The schema's columns for a file path relative to the directory, or None for an unknown path."""
    parts = rel.split("/")
    if len(parts) == 1 and rel.endswith(".parquet"):
        return SCHEMA["tables"].get(rel[: -len(".parquet")])
    if len(parts) == 3 and parts[0] == "series" and re.fullmatch(r"\d{4}-\d{2}\.parquet", parts[2]):
        return SCHEMA["series"].get(parts[1])
    if len(parts) == 2 and parts[0] == "rr" and rel.endswith(".parquet"):
        return SCHEMA["rr"]
    return None


def main(argv):
    if len(argv) != 2:
        print(__doc__)
        return 2
    d = Path(argv[1])
    con = duckdb.connect()
    rows = []  # (check, ok, detail)
    man_path = d / "manifest.json"
    if not man_path.exists():
        print(f"FAIL no manifest.json in {d}")
        return 1
    man = json.loads(man_path.read_text())
    rows.append(("manifest schema_version", man.get("schema_version") == SCHEMA["schema_version"], str(man.get("schema_version"))))
    listed = {f["path"]: f for f in man["files"]}
    present = {str(p.relative_to(d)) for p in d.rglob("*.parquet")}
    rows.append(("manifest lists every file", present == set(listed),
                 f"{len(present)} files; unlisted {sorted(present - set(listed))[:3]}, missing {sorted(set(listed) - present)[:3]}"))
    for name in SCHEMA["tables"]:
        rows.append((f"{name}.parquet present", f"{name}.parquet" in present, ""))

    by_kind: dict[str, list] = {}
    for rel in sorted(present):
        want = expected(rel)
        kind = "/".join(rel.split("/")[:2]) if rel.startswith("series/") else "rr" if rel.startswith("rr/") else rel
        if want is None:
            by_kind.setdefault(kind, []).append((rel, False, "not a wellness-v1 path"))
            continue
        path = str(d / rel).replace("'", "''")
        got = [(r[0], r[1]) for r in con.execute(f"DESCRIBE SELECT * FROM read_parquet('{path}')").fetchall()]
        want = [tuple(c) for c in want]
        problems = []
        if got != want:
            gd = dict(got)
            if [c for c, _ in got] != [c for c, _ in want]:
                problems.append(f"columns {[c for c, _ in got]} vs {[c for c, _ in want]}")
            problems += [f"{c}: {gd[c]} vs {t}" for c, t in want if c in gd and gd[c] != t]
        n = con.execute(f"SELECT count(*) FROM read_parquet('{path}')").fetchone()[0]
        entry = listed.get(rel)
        if entry is not None:
            if entry.get("rows") != n:
                problems.append(f"rows {n} vs manifest {entry.get('rows')}")
            sha = hashlib.sha256((d / rel).read_bytes()).hexdigest()
            if entry.get("sha256") != sha:
                problems.append("sha256 differs from the manifest")
        by_kind.setdefault(kind, []).append((rel, not problems, "; ".join(problems)))

    for kind, files in sorted(by_kind.items()):
        bad = [f for f in files if not f[1]]
        detail = f"{len(files)} files" + (f"; {bad[0][0]}: {bad[0][2]}" if bad else "")
        rows.append((f"DESCRIBE {kind}", not bad, detail))

    w = max(len(r[0]) for r in rows)
    for check, ok, detail in rows:
        print(f"{'PASS' if ok else 'FAIL'}  {check:<{w}}  {detail}")
    failed = sum(not r[1] for r in rows)
    print(f"{len(rows) - failed}/{len(rows)} PASS" if not failed else f"{failed} FAIL")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
