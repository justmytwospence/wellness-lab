"""The Python bindings on synthetic data (no personal data).

    scripts/tc scripts/build-py.sh
    scripts/tc bash -c 'cd tools/goldens && uv run pytest -q ../../crates/py/tests'
"""

import json
from pathlib import Path

import numpy as np
import pytest
import wellness_lab as wl

ROOT = Path(__file__).resolve().parents[3]
SEED, DAYS = 7, 400
FROM = 19_723  # 2024-01-01, where synth starts


@pytest.fixture(scope="module")
def lab():
    return wl.synth(SEED, DAYS)


@pytest.fixture(scope="module")
def truth():
    return wl.synth_labels(SEED, DAYS)


def overlaps(p, t):
    return p["start_day"] <= t["end_day"] and p["end_day"] >= t["start_day"]


def test_version_and_registry():
    assert wl.version()
    reg = wl.registry()
    assert reg, "empty registry"
    text = json.dumps(reg)
    assert "hrv4t" in text or "HRV" in text


def test_synth_round_trip(tmp_path, lab):
    """write_synth (the native writer) then load_dir gives back the same data, at float32 precision."""
    man = wl.write_synth(str(tmp_path), SEED, DAYS)
    assert man["schema_version"] == 1 and man["files"]
    paths = {f["path"] for f in man["files"]}
    assert {"days.parquet", "sleeps.parquet"} <= paths
    assert any(p.startswith("series/hr/") for p in paths)
    back = wl.Lab()
    back.load_dir(str(tmp_path))
    schema = json.loads((ROOT / "schema" / "wellness-v1.json").read_text())
    cols = [c for c, _ in schema["tables"]["days"]]
    job = {"kind": "days", "from": FROM, "to": FROM + DAYS, "columns": cols}
    a, _ = lab.run(job)
    b, _ = back.run(job)
    assert a["n"] == b["n"] == DAYS
    # the reader fills the columns synth leaves out with nulls
    for kind in ("num", "txt"):
        assert a[kind].keys() <= b[kind].keys()
        assert all(all(v is None for v in b[kind][c]) for c in b[kind].keys() - a[kind].keys())
    assert all(a["txt"][c] == b["txt"][c] for c in a["txt"])
    for c in a["num"]:
        x = np.array(a["num"][c], dtype=float)
        y = np.array(b["num"][c], dtype=float)
        np.testing.assert_allclose(y, x, rtol=1e-6, err_msg=c)  # FLOAT columns are float32 on disk
    ca, _ = lab.run({"kind": "coverage"})
    cb, _ = back.run({"kind": "coverage"})
    assert json.dumps(ca, sort_keys=True) == json.dumps(cb, sort_keys=True)
    # and the export-result path (what the browser writes) has the same rows
    res = wl.synth_export(SEED, DAYS)
    assert len(res.numeric("days", "day")) == DAYS
    assert set(res.series()) >= {"hr", "hrv5", "stress"}


def test_nights_has_one_row_per_main_sleep(lab):
    kind = wl.synth_export(SEED, DAYS).numeric("sleeps", "kind")
    main = int(np.sum(kind == 0))
    nights, _ = lab.run({"kind": "nights"})
    assert nights["n"] == main
    assert len(nights["num"]["day"]) == main


def test_detect_recovers_injected_illnesses(truth):
    fresh = wl.synth(SEED, DAYS)
    fresh.set_labels([])  # detection proposes only where nothing is labelled yet
    res, _ = fresh.run({"kind": "detect"})
    props = [p for p in res["proposals"] if p["model_version"] != "exposure-1"]
    ill = [t for t in truth if t["kind"] == "illness"]
    assert len(ill) >= 4
    found = sum(any(overlaps(p, t) for p in props) for t in ill)
    assert found >= 0.8 * len(ill), f"illness recall {found}/{len(ill)}"


def test_hrv4t_documented_keys(lab):
    res, _ = lab.run({"kind": "hrv4t", "range_days": 60, "k_daily": 1.5, "k_swc": 0.5})
    keys = {"nightly", "scored", "acute_lag", "acute_pts", "barcode", "stage_hrv", "night_sens", "model_view", "model",
            "load_cuts", "headline", "spark"}
    assert keys <= set(res), keys - set(res)
    assert res["nightly"]["n"] > 0
    assert {"beta", "tau", "sigma_eps", "sigma_eta", "lr", "nights"} <= set(res["model"])


def test_bench_is_not_empty(lab):
    rows, _ = lab.run({"kind": "bench"})
    assert rows, "empty bench"
    assert {"metric", "grade", "te", "icc"} <= set(rows[0])
    assert any(r["metric"] == "hr_min30" for r in rows)


def test_event_study_illness_raises_hr():
    # Two years, as the core test: in 400 days the 25 alcohol nights (each excluding controls 14 days around it) leave
    # fewer than the 3 matched episode-control pairs at r = 1 that a bootstrap interval needs.
    lab = wl.synth(SEED, 730)
    job = {"kind": "event_study", "label_kind": "illness", "metrics": ["hr_min30"], "window": [-7, 14],
           "controls": {"same_weekday": False, "same_load_class": False}}
    res, _ = lab.run(job)
    assert res["episodes"] >= 4 and not res["too_few"]
    m = res["metrics"][0]
    assert m["metric"] == "hr_min30"
    i = m["rel"].index(1)
    assert m["diff"][i] > 3, m["diff"][i]
    assert m["lo"][i] > 0, (m["lo"][i], m["hi"][i])


def test_arrays_come_back_as_numpy(lab):
    meta, arrays = lab.run({"kind": "raster", "metric": "hr", "from": FROM, "to": FROM + 60, "align": "onset"})
    rgba = arrays["rgba"]
    assert rgba.dtype == np.uint8
    assert rgba.size == meta["width"] * meta["height"] * 4
    text, arrays2 = lab.run_json({"kind": "raster", "metric": "hr", "from": FROM, "to": FROM + 60, "align": "onset"})
    assert json.loads(text) == meta
    assert np.array_equal(arrays2["rgba"], rgba)


def test_loaders():
    lab = wl.Lab()
    day = np.arange(FROM, FROM + 3, dtype=float)
    lab.load_table("days", {"day": day, "rhr": np.array([50.0, np.nan, 52.0])}, {"hrv_status": ["BALANCED", None, "LOW"]})
    t = (FROM * 86_400_000 + np.arange(5) * 60_000).astype(np.int64)
    lab.load_series("hr", t, {"v": np.array([50, 51, 52, 53, 54.0])})
    lab.load_rr("1", np.full(300, 800.0))
    lab.set_settings({})
    lab.set_proposals([])
    lab.set_labels([])
    res, _ = lab.run({"kind": "days", "from": FROM, "to": FROM + 2, "columns": ["rhr", "hrv_status"]})
    assert res["num"]["rhr"] == [50.0, None, 52.0]
    assert res["txt"]["hrv_status"] == ["BALANCED", None, "LOW"]
    with pytest.raises(RuntimeError):
        lab.load_table("nope", {})
    with pytest.raises(RuntimeError):
        lab.run({"kind": "nope"})


def test_export_builder_finishes_empty():
    b = wl.ExportBuilder()
    res = b.finish()
    assert res.tables() == ["days", "sleeps", "stages", "activities", "tags", "devices"]
    assert isinstance(res.report(), dict)
    with pytest.raises(RuntimeError):
        b.finish()
