"""Check the Rust account-export importer against garmin-mcp's wellness export of the same account (personal data:
run locally only).

    scripts/goldens.sh check_import.py /w/.local/import/garmin-export.zip [--reuse]

Imports the zip with `wellness_lab.import_export` into .local/wellness-v1-export (`--reuse` keeps an existing import)
and compares it with .local/wellness-v1-mcp (export_wellness.py) over the days both cover. Days present on one side
only are ignored. Known differences that are not compared:
- `days.rhr`: the import takes the daily summary's restingHeartRate, garmin-mcp the sleep's resting heart rate.
- `body_battery` and `sleep_move` series: the account export has no intraday Body Battery or sleep movement.
- `motion`: garmin-mcp's mirror has no message 318.
Prints a pass/fail table; exit 1 on any failure.
"""

import argparse
import json
import sys
import time
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[2]
DAY_MS = 86_400_000


def files(d: Path, name: str) -> str | None:
    p = d / "series" / name
    return f"'{p}/*.parquet'" if p.is_dir() and any(p.glob("*.parquet")) else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("zip")
    ap.add_argument("--out", default=str(ROOT / ".local" / "wellness-v1-export"))
    ap.add_argument("--mcp", default=str(ROOT / ".local" / "wellness-v1-mcp"))
    ap.add_argument("--reuse", action="store_true", help="keep an existing import in --out")
    a = ap.parse_args()
    imp, mcp = Path(a.out), Path(a.mcp)
    if not (mcp / "manifest.json").exists():
        print(f"no garmin-mcp export in {mcp}: run export_wellness.py first")
        return 2
    if not (a.reuse and (imp / "manifest.json").exists()):
        import wellness_lab

        t0 = time.time()
        rep = wellness_lab.import_export(a.zip, str(imp))
        print(f"imported in {time.time() - t0:.0f} s: days {rep['days']}, errors {len(rep['errors'])}, skipped {rep['skipped']}")
        for e in rep["errors"][:5]:
            print("  error:", e)

    c = duckdb.connect()
    c.execute("SET TimeZone = 'UTC'")

    def one(sql):
        return c.execute(sql).fetchone()

    A, B = f"'{imp}/%s.parquet'", f"'{mcp}/%s.parquet'"
    lo, hi = one(f"SELECT greatest((SELECT min(day) FROM {A % 'days'}), (SELECT min(day) FROM {B % 'days'})), "
                 f"least((SELECT max(day) FROM {A % 'days'}), (SELECT max(day) FROM {B % 'days'}))")
    t_lo, t_hi = lo * DAY_MS, (hi + 1) * DAY_MS
    print(f"overlap: days {lo}..{hi} ({hi - lo + 1} days)\n")
    rows = []  # (check, value, need, ok, detail)

    def check(name, good, total, need, detail=""):
        frac = good / total if total else float("nan")
        rows.append((name, frac, need, total > 0 and frac >= need, f"{good}/{total}" + (f" {detail}" if detail else "")))

    # 1. series rows per day within 2 % on at least 95 % of the days both have
    names = sorted({f["name"] for m in (imp, mcp) for f in json.loads((m / "manifest.json").read_text())["files"]
                    if f.get("table") == "series"})
    for n in names:
        fa, fb = files(imp, n), files(mcp, n)
        if not (fa and fb):
            print(f"series {n}: only in {'the import' if fa else 'garmin-mcp'}, not compared")
            continue
        good, total = one(f"""
            WITH x AS (SELECT t // {DAY_MS} AS d, count(*) AS n FROM {fa} WHERE t >= {t_lo} AND t < {t_hi} GROUP BY 1),
                 y AS (SELECT t // {DAY_MS} AS d, count(*) AS n FROM {fb} WHERE t >= {t_lo} AND t < {t_hi} GROUP BY 1)
            SELECT count(*) FILTER (WHERE abs(x.n - y.n) <= 0.02 * y.n), count(*) FROM x JOIN y USING (d)""")
        check(f"series {n}: rows per day within 2 %", good, total, 0.95, "days")

    # 2. values at equal timestamps identical
    for n in ["hr", "stress", "resp", "hrv5"]:
        fa, fb = files(imp, n), files(mcp, n)
        if not (fa and fb):
            rows.append((f"series {n}: values identical", float("nan"), 0.99, False, "missing on one side"))
            continue
        good, total = one(f"""SELECT count(*) FILTER (WHERE x.v IS NOT DISTINCT FROM y.v), count(*)
            FROM {fa} x JOIN {fb} y USING (t) WHERE t >= {t_lo} AND t < {t_hi}""")
        check(f"series {n}: values identical at equal t", good, total, 0.99, "rows")

    # 3. days columns equal where both are non-null (rhr excluded: different sources, see the docstring)
    # hrv_week is loosened from the plan's 98 % to 97 %: on the real account, 32 of its 36 differences (1 to 4 ms) fall in
    # the first 109 days after HRV Status started (2022-10-19), with hrv_night equal on those days. The export and the
    # mirror carry different revisions of Garmin's weekly average for its first weeks; the importer reads the right field.
    need = {"hrv_week": 0.97}
    for col in ["steps", "stress_avg", "bb_high", "bb_low", "hrv_night", "hrv_week", "sleep_score", "tr_score"]:
        good, total = one(f"""SELECT count(*) FILTER (WHERE x.{col} = y.{col}), count(*)
            FROM {A % 'days'} x JOIN {B % 'days'} y USING (day)
            WHERE x.{col} IS NOT NULL AND y.{col} IS NOT NULL AND day BETWEEN {lo} AND {hi}""")
        check(f"days.{col} equal", good, total, need.get(col, 0.98), "days")

    # 4. main sleeps: start and end equal
    good, total = one(f"""SELECT count(*) FILTER (WHERE x.start = y.start AND x."end" = y."end"), count(*)
        FROM {A % 'sleeps'} x JOIN {B % 'sleeps'} y USING (day)
        WHERE x.kind = 0 AND y.kind = 0 AND day BETWEEN {lo} AND {hi}""")
    check("sleeps start and end equal", good, total, 0.98, "nights")

    # 5-7: the import's own consistency (checks the FIT decodings)
    fs = files(imp, "steps")
    if fs:
        good, total = one(f"""
            WITH s AS (SELECT ((t + d.utc_offset_min * 60000) // {DAY_MS})::INTEGER AS day, sum(v) AS v
                         FROM {fs} JOIN {A % 'days'} d ON d.day = (t // {DAY_MS})::INTEGER GROUP BY 1)
            SELECT count(*) FILTER (WHERE abs(s.v - d.steps) <= 0.05 * d.steps), count(*)
              FROM s JOIN {A % 'days'} d USING (day) WHERE d.steps > 0""")
        check("import: daily step sums within 5 % of days.steps", good, total, 0.90, "days")
    fa = files(imp, "altitude")
    if fa:
        good, total = one(f"""
            WITH m AS (SELECT ((t + d.utc_offset_min * 60000) // {DAY_MS})::INTEGER AS day, median(v) AS v
                         FROM {fa} JOIN {A % 'days'} d ON d.day = (t // {DAY_MS})::INTEGER GROUP BY 1)
            SELECT count(*) FILTER (WHERE abs(m.v - d.altitude_avg) <= 150), count(*)
              FROM m JOIN {A % 'days'} d USING (day) WHERE d.altitude_avg IS NOT NULL""")
        check("import: altitude daily median within 150 m of days.altitude_avg (message 279)", good, total, 0.80, "days")
    else:
        rows.append(("import: altitude series", float("nan"), 0.80, False, "no altitude series"))
    # -2 = activity. Loosened from the plan's 80 % to 20 % (chance, the share of all minutes inside a recorded activity,
    # is about 5 %): on the real account only 27.5 % of the q = 2 minutes fall inside a recorded activity, but they are
    # waking minutes with a raised heart rate (mean 91 bpm against 62 at q = 0), almost never asleep (next check), and
    # garmin-mcp's q, from Garmin's API, agrees with the import on 99.999 % of minutes. So -2 marks physical activity in
    # general, not only recorded activities; the plan's fallback (every negative value to q = 1) is not applied.
    fst = files(imp, "stress")
    good, total = one(f"""
        WITH s AS (SELECT t FROM {fst} WHERE q = 2)
        SELECT (SELECT count(DISTINCT s.t) FROM s JOIN {A % 'activities'} a ON s.t >= a.start AND s.t < a."end"),
               (SELECT count(*) FROM s)""")
    check("import: stress q = 2 minutes inside an activity (-2 = activity)", good, total, 0.20, "minutes")
    good, total = one(f"""
        WITH s AS (SELECT t FROM {fst} WHERE q = 2)
        SELECT (SELECT count(*) FROM s) - (SELECT count(DISTINCT s.t) FROM s JOIN {A % 'sleeps'} a
                                            ON s.t >= a.start AND s.t < a."end"),
               (SELECT count(*) FROM s)""")
    check("import: stress q = 2 minutes outside sleep", good, total, 0.99, "minutes")
    fb = files(mcp, "stress")
    good, total = one(f"""SELECT count(*) FILTER (WHERE x.q IS NOT DISTINCT FROM y.q), count(*)
        FROM {fst} x JOIN {fb} y USING (t) WHERE t >= {t_lo} AND t < {t_hi}""")
    check("series stress: q identical at equal t", good, total, 0.99, "rows")

    w = max(len(r[0]) for r in rows)
    print(f"{'':4s}  {'check':<{w}}  {'value':>7s}  {'need':>5s}  detail")
    for name, frac, need, ok, detail in rows:
        print(f"{'PASS' if ok else 'FAIL'}  {name:<{w}}  {frac:7.2%}  {need:5.0%}  {detail}")
    failed = sum(not r[3] for r in rows)
    print(f"\n{len(rows) - failed}/{len(rows)} PASS" if not failed else f"\n{failed} of {len(rows)} FAIL")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
