# Adapted from effort-cubes tests/xbind/make_cases.py @ ee7ff1d (same author), relicensed under AGPL-3.0-only
"""Write cross-binding cases from the installed wheel: .local/xbind/cases.json, <job>.json and raw .bin arrays.

    scripts/tc bash -c 'cd tools/goldens && uv run python ../../tests/xbind/make_cases.py'
    scripts/tc node tests/xbind/run.mjs

Twelve jobs run on synth(7, 400) (plus one synthetic beat-interval recording for `beats`). tests/xbind/run.mjs replays
them through the nodejs wasm build and requires byte-equal JSON and arrays.
"""

import json
import shutil
from pathlib import Path

import numpy as np
import wellness_lab as wl

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / ".local" / "xbind"
SEED, DAYS = 7, 400
FROM = 19_723  # 2024-01-01, where synth starts
TO = FROM + DAYS - 1
RR_ID = "synth-1"

JOBS = {
    "nights": {"kind": "nights"},
    "night": {"kind": "night", "day": FROM + 200},
    "raster": {"kind": "raster", "metric": "hr", "align": "onset", "from": FROM, "to": TO},
    "typical": {"kind": "typical", "metric": "hr", "align": "onset", "a": [FROM, FROM + 199], "b": [FROM + 200, TO]},
    "hrv4t": {"kind": "hrv4t", "range_days": 60, "k_daily": 1.5, "k_swc": 0.5},
    "detect": {"kind": "detect"},
    "event_study": {"kind": "event_study", "label_kind": "illness", "metrics": ["hr_min30", "ln_rmssd", "resp_mean"],
                    "window": [-7, 14]},
    "bench": {"kind": "bench"},
    "compare": {"kind": "compare", "sel": [[FROM + 100, FROM + 110]], "base": [[FROM + 60, FROM + 99]]},
    "circadian": {"kind": "circadian", "from": FROM + 100, "to": FROM + 160},
    "teardown": {"kind": "teardown"},
    "beats": {"kind": "beats", "id": RR_ID},
}


def rr_recording(seed=3, minutes=45):
    """Beat intervals (ms) of a steady run with drift, breathing, a sensor gap and a few artifacts, as float32."""
    rng = np.random.default_rng(seed)
    out, t = [], 0.0
    while t < minutes * 60_000:
        m = t / 60_000
        rr = 420 + 60 * np.exp(-m / 8) + 12 * np.sin(2 * np.pi * t / 4000) + rng.normal(0, 6)
        out.append(rr)
        t += rr
    rr = np.array(out, dtype=np.float32)
    for i in rng.choice(len(rr), 25, replace=False):
        rr[i] *= rng.choice([0.5, 1.9])  # missed and extra beats
    rr[1000] = 5000.0  # a sensor gap
    return rr


def save(name, a):
    a = np.ascontiguousarray(a)
    f = f"{name}.bin"
    (OUT / f).write_bytes(a.astype(a.dtype.newbyteorder("<")).tobytes())
    return {"file": f, "dtype": str(a.dtype), "len": int(a.size)}


def main():
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    lab = wl.synth(SEED, DAYS)
    rr = rr_recording()
    lab.load_rr(RR_ID, rr)
    man = {"seed": SEED, "days": DAYS, "rr": {"id": RR_ID, **save("rr", rr)}, "jobs": []}
    for name, job in JOBS.items():
        text = json.dumps(job, separators=(",", ":"))
        res, arrays = lab.run_json(text)
        (OUT / f"{name}.json").write_text(res, encoding="utf-8")
        man["jobs"].append({
            "name": name,
            "job": text,
            "json": f"{name}.json",
            "arrays": {k: save(f"{name}.{k}", v) for k, v in arrays.items()},
        })
        print(f"{name:12s} {len(res):>9d} bytes json, {len(arrays)} arrays")
    (OUT / "cases.json").write_text(json.dumps(man, indent=1))
    print("cases in", OUT)


if __name__ == "__main__":
    main()
