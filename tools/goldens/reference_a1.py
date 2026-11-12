"""The notebook's beat-to-beat cells as plain Python: DFA a1 windows and thresholds, durability, recording quality, the
cleaned series, the one-minute window, the Poincare plot and the Lomb-Scargle spectrum. Transcribed on 2026-11-12 from the same
notebook as reference_hrv.py (cells a1_build, a1_thresholds, durability_chart, rr_sessions, beats, window_start, window_steps,
session_rmssd, poincare_chart); the cell bodies are verbatim, marimo widgets are parameters.

One deliberate difference: beat intervals are rounded to whole milliseconds (FIT stores them with 1 ms resolution), so that
exact-threshold decisions do not depend on floating-point noise.
"""

from __future__ import annotations

import json

import numpy as np
import polars as pl
from scipy.signal import lombscargle

import reference_hrv as ref


def dfa_a1(rr, scales=np.arange(4, 17)):
    y = np.cumsum(rr - rr.mean())
    F = []
    for n in scales:
        m = len(y) // n
        seg = y[: m * n].reshape(m, n)
        x = np.arange(n) - (n - 1) / 2
        b = (seg * x).sum(1) / (x ** 2).sum()
        res = seg - seg.mean(1)[:, None] - b[:, None] * x
        F.append(np.sqrt((res ** 2).mean()))
    return np.polyfit(np.log(scales), np.log(F), 1)[0]


def _qd(x):
    s = pl.Series(np.abs(x))
    return 5.2 * (
        s.rolling_quantile(0.75, window_size=91, min_samples=10, center=True).to_numpy()
        - s.rolling_quantile(0.25, window_size=91, min_samples=10, center=True).to_numpy()
    ) / 2


def _correct(rr):
    med = pl.Series(rr).rolling_median(11, min_samples=3, center=True).to_numpy()
    drr = np.diff(rr, prepend=rr[0])
    mrr = rr - med
    bad = ((np.abs(drr) > np.maximum(_qd(drr), 20)) & (np.abs(mrr) > np.maximum(_qd(mrr), 20))) | (np.abs(mrr) > 0.3 * med)
    out, i = [], 0
    while i < len(rr):
        if not bad[i]:
            out.append(rr[i]); i += 1
        elif abs(rr[i] / 2 - med[i]) < 0.2 * med[i]:
            out += [rr[i] / 2, rr[i] / 2]; i += 1
        elif i + 1 < len(rr) and abs(rr[i] + rr[i + 1] - med[i]) < 0.2 * med[i]:
            out.append(rr[i] + rr[i + 1]); i += 2
        else:
            out.append(med[i]); i += 1
    return np.array(out), bad


def _windows(rr_raw, win_s=120, step_s=30):
    valid = (rr_raw < 4999) & (rr_raw >= 250) & (rr_raw <= 2000)
    seg_id = np.cumsum(~valid)
    rows, t_off = [], 0.0
    for sid in np.unique(seg_id[valid]):
        seg = rr_raw[valid & (seg_id == sid)]
        if seg.sum() < win_s * 1000:
            t_off += seg.sum() / 1000
            continue
        corr, bad = _correct(seg)
        t_raw, t_cor = np.cumsum(seg) / 1000, np.cumsum(corr) / 1000
        start = 0.0
        while start + win_s <= t_cor[-1]:
            sel = (t_cor > start) & (t_cor <= start + win_s)
            selr = (t_raw > start) & (t_raw <= start + win_s)
            w, raw = corr[sel], seg[selr]
            if len(w) >= 60:
                rows.append((t_off + start + win_s, 60000 / w.mean(), dfa_a1(w), bad[selr].mean(), float((np.diff(raw) == 0).mean())))
            start += step_s
        t_off += t_raw[-1]
    return rows


_SPORT = {
    "running": "run", "trail_running": "run", "treadmill_running": "run", "track_running": "run",
    "cycling": "ride", "road_biking": "ride", "gravel_cycling": "ride", "indoor_cycling": "ride", "mountain_biking": "ride", "virtual_ride": "ride",
    "backcountry_skiing": "ski", "skate_skiing_ws": "ski", "cross_country_skiing_ws": "ski", "resort_skiing": "ski",
}


def a1_build(q, activity_ids):
    parts = []
    _rr = q(f"select activity_id, round(rr_interval * 1000) as rr from garmin.fit_hrv where activity_id in ({','.join(map(str, activity_ids))}) order by activity_id, idx, pos")
    for (_aid,), _g in _rr.group_by(["activity_id"], maintain_order=True):
        _r = _windows(_g["rr"].to_numpy())
        parts.append(pl.DataFrame(_r, schema=["t_s", "hr", "a1", "art", "rep"], orient="row").with_columns(activity_id=pl.lit(_aid, pl.Int64)) if _r else pl.DataFrame({"activity_id": [_aid]}, schema={"activity_id": pl.Int64}))
    a1_windows = pl.concat(parts, how="diagonal_relaxed").drop_nulls("t_s")
    a1_acts = (
        q("select activity_id, activity_name as name, activity_type, start_time_local as start, duration / 60 as minutes, average_hr from garmin.activity where deleted_at is null")
        .join(a1_windows.group_by("activity_id").agg(windows=pl.len(), art=pl.col("art").mean(), rep=pl.col("rep").median()), on="activity_id")
        .with_columns(sport=pl.col("activity_type").replace_strict(_SPORT, default="other"), day=pl.col("start").dt.date())
        .sort("start", descending=True)
    )
    return a1_acts, a1_windows


def a1_curve(w, max_art=0.05, max_rep=0.3, bin_bpm=2, min_n=4):
    g = (
        w.filter((pl.col("art") <= max_art) & (pl.col("rep") <= max_rep) & pl.col("a1").is_finite())
        .with_columns(hb=(pl.col("hr") / bin_bpm).floor() * bin_bpm + bin_bpm / 2)
        .group_by("hb").agg(a1=pl.col("a1").median(), n=pl.len()).filter(pl.col("n") >= min_n).sort("hb")
    )
    if g.height < 5:
        return g.with_columns(line=pl.lit(None, pl.Float64)), None, None
    a1v, hb, n = g["a1"].to_numpy(), g["hb"].to_numpy(), g["n"].to_numpy().astype(float)
    z = (a1v >= 0.35) & (a1v <= 1.1)
    if z.sum() < 4:
        return g.with_columns(line=pl.lit(None, pl.Float64)), None, None
    b, a = np.polyfit(hb[z], a1v[z], 1, w=np.sqrt(n[z]))
    lo, hi = hb[z].min() - bin_bpm, hb[z].max() + bin_bpm
    cross = lambda lv: float((lv - a) / b) if b < 0 and lo <= (lv - a) / b <= hi else None  # noqa: E731
    return g.with_columns(line=pl.Series(np.where((hb >= lo) & (hb <= hi), a + b * hb, np.nan)).fill_nan(None)), cross(0.75), cross(0.5)


def a1_thresholds(a1_acts, a1_windows):
    a1_fresh = a1_windows.filter(pl.col("t_s") <= 3720).join(a1_acts.select("activity_id", "sport", "day"), on="activity_id")
    _rows = []
    for _sp in ("run", "ride", "ski"):
        _ws = a1_fresh.filter(pl.col("sport") == _sp)
        if _ws.height == 0:
            continue
        for _end in pl.date_range(_ws["day"].min(), _ws["day"].max() + pl.duration(days=6), "7d", eager=True):
            _win = _ws.filter(pl.col("day").is_between(_end - pl.duration(days=42), _end))
            if _win["activity_id"].n_unique() >= 3:
                _, _v1, _v2 = a1_curve(_win)
                _rows.append(dict(sport=_sp, date=_end, vt1=_v1, vt2=_v2, activities=_win["activity_id"].n_unique()))
    a1_history = pl.DataFrame(_rows)
    A1_NOW = {}
    for _sp in ("run", "ride", "ski"):
        _h = a1_history.filter((pl.col("sport") == _sp) & pl.col("vt1").is_not_null()).sort("date")
        _all = a1_curve(a1_fresh.filter(pl.col("sport") == _sp))
        A1_NOW[_sp] = dict(
            vt1=_h["vt1"][-1] if _h.height else None, vt2=_h.drop_nulls("vt2")["vt2"][-1] if _h.drop_nulls("vt2").height else None,
            as_of=_h["date"][-1].isoformat() if _h.height else None, vt1_all=_all[1], vt2_all=_all[2],
            activities=int(a1_acts.filter(pl.col("sport") == _sp).height),
        )
    return A1_NOW, a1_fresh, a1_history


def durability(a1_acts, a1_windows):
    _rows = []
    for (_aid,), _g in a1_windows.filter((pl.col("art") <= 0.05) & (pl.col("rep") <= 0.3)).group_by(["activity_id"]):
        if _g["t_s"].max() < 7200:
            continue
        _f = _g.filter(pl.col("t_s") <= 3720)
        if _f.height < 12 or _f["hr"].max() - _f["hr"].min() < 8:
            continue
        _b, _a = np.polyfit(_f["hr"].to_numpy(), _f["a1"].to_numpy(), 1)
        _rows.append(_g.with_columns(resid=pl.col("a1") - (_a + _b * pl.col("hr")), block=((pl.col("t_s") - 1) // 1800 * 0.5 + 0.5)).group_by("block").agg(resid=pl.col("resid").mean(), n=pl.len()).filter(pl.col("n") >= 8).with_columns(activity_id=pl.lit(_aid)))
    return pl.concat(_rows).join(a1_acts.select("activity_id", "sport", "name", "day"), on="activity_id") if _rows else pl.DataFrame()


def rr_sessions(q):
    rr_sessions = q("""
        with r as (
            select activity_id, round(rr_interval * 1000) as rr,
                   lead(round(rr_interval * 1000)) over (partition by activity_id order by idx, pos) as nx,
                   median(round(rr_interval * 1000)) over (partition by activity_id order by idx, pos rows between 5 preceding and 5 following) as med
            from garmin.fit_hrv where rr_interval between 0.3 and 2.0
        )
        select r.activity_id, any_value(a.activity_name) as name, any_value(a.start_time_local) as start,
               any_value(a.average_hr) as avg_hr, count(*) as beats,
               avg((nx = rr)::int) as repeat_share,
               avg((abs(rr - med) > 0.2 * med)::int) as artifact_share
        from r join garmin.activity a using (activity_id)
        group by r.activity_id
        having beats >= 500
    """).with_columns(
        quality=pl.when((pl.col("repeat_share") < 0.1) & (pl.col("artifact_share") < 0.03)).then(pl.lit("clean"))
        .when(pl.col("repeat_share") >= 0.1).then(pl.lit("smoothed"))
        .otherwise(pl.lit("noisy"))
    )
    return rr_sessions.sort([pl.col("quality").replace_strict({"clean": 0, "noisy": 1, "smoothed": 2}), "artifact_share"])


def beats_cells(q, activity_id):
    _raw = q(f"""
        select round(rr_interval * 1000) as rr from garmin.fit_hrv
        where activity_id = {activity_id}
        order by idx, pos
    """)
    _rr = _raw["rr"].to_numpy()
    _gap = _rr >= 4999  # the FIT file's "no beat" filler: the sensor lost contact
    _valid = ~_gap & (_rr >= 300) & (_rr <= 2000)
    _med = pl.Series(np.where(_valid, _rr, np.nan)).fill_nan(None).rolling_median(11, min_samples=3, center=True).to_numpy()
    _ectopic = _valid & (np.abs(_rr - _med) > 0.2 * _med)
    beats = (
        pl.DataFrame({"rr": _rr})
        .with_row_index("beat")
        .with_columns(
            t=pl.Series(np.cumsum(np.where(_gap, 0, _rr)) / 1000),
            status=pl.Series(np.select([_gap, ~_valid, _ectopic], ["sensor gap", "out of range", "artifact"], "kept")),
        )
        .filter(pl.col("status") != "sensor gap")
    )
    clean = beats.filter(pl.col("status") == "kept")
    _by_min = (
        clean.with_columns(minute=(pl.col("t") // 60).cast(pl.Int64), d=pl.col("rr").diff())
        .group_by("minute").agg(v=(pl.col("d") ** 2).mean(), n=pl.len())
        .filter(pl.col("n") >= 30)
        .sort("v", descending=True)
    )
    minute = int(_by_min["minute"][0]) if _by_min.height else 0
    _t0 = minute * 60
    win = (
        beats.filter(pl.col("t").is_between(_t0, _t0 + 60))
        .with_columns(
            d=pl.when((pl.col("status") == "kept") & (pl.col("status").shift(-1) == "kept"))
            .then(pl.col("rr").shift(-1) - pl.col("rr"))
            .otherwise(None)
        )
        .with_columns(d2=pl.col("d") ** 2, sec=pl.col("t") - _t0)
    )
    _ok = win.drop_nulls("d")
    win_rmssd = float(np.sqrt(_ok["d2"].mean())) if _ok.height else float("nan")
    _pairs = beats.with_columns(d=pl.col("rr").shift(-1) - pl.col("rr"), ok=(pl.col("status") == "kept") & (pl.col("status").shift(-1) == "kept")).filter(pl.col("ok"))
    session_rmssd = float(np.sqrt((_pairs["d"] ** 2).mean()))
    _k = clean.with_columns(nx=pl.col("rr").shift(-1)).drop_nulls("nx")
    _d = (_k["nx"] - _k["rr"]).to_numpy()
    _s = (_k["nx"] + _k["rr"]).to_numpy()
    _sd1, _sd2 = float(np.std(_d) / np.sqrt(2)), float(np.std(_s) / np.sqrt(2))
    _t = clean["t"].to_numpy()
    _r = clean["rr"].to_numpy()
    _f = np.linspace(0.01, 0.6, 300)
    _segs = []
    for _s0 in np.arange(_t.min(), _t.max() - 300, 150):  # 5-minute segments, half-overlapping, averaged (Welch-style)
        _m = (_t >= _s0) & (_t < _s0 + 300)
        if _m.sum() > 100:
            _tt, _xx = _t[_m], _r[_m]
            _xx = _xx - np.polyval(np.polyfit(_tt, _xx, 1), _tt)
            _segs.append(lombscargle(_tt, _xx, 2 * np.pi * _f, normalize=False))
    _p = np.mean(_segs, axis=0) if _segs else np.zeros_like(_f)
    spec = pl.DataFrame({"f": _f, "p": np.clip(_p / max(_p.max(), 1e-12), 1e-4, 1)})
    return dict(beats=beats, minute=minute, win=win, win_rmssd=win_rmssd, session_rmssd=session_rmssd, sd1=_sd1, sd2=_sd2, spec=spec)


def record(snapshot, out, parts):
    q = ref.connect(snapshot)
    ids = q("select distinct activity_id from garmin.fit_hrv")["activity_id"].to_list()
    if "a1" in parts:
        a1_acts, a1_windows = a1_build(q, sorted(ids))
        a1_acts.write_parquet(out / "a1_acts.parquet")
        a1_windows.write_parquet(out / "a1_windows.parquet")
        now, _fresh, hist = a1_thresholds(a1_acts, a1_windows)
        hist.write_parquet(out / "a1_history.parquet")
        (out / "A1_NOW.json").write_text(json.dumps(now))
        dur = durability(a1_acts, a1_windows)
        dur.write_parquet(out / "durability.parquet")
        print("a1:", a1_windows.height, "windows;", hist.height, "history rows")
    if "beats" in parts:
        s = rr_sessions(q)
        s.write_parquet(out / "rr_sessions.parquet")
        picks = [int(a) for a in s["activity_id"].head(3).to_list()]
        (out / "beats_ids.json").write_text(json.dumps(picks))
        for a in picks:
            r = beats_cells(q, a)
            r["beats"].write_parquet(out / f"beats_{a}.parquet")
            r["win"].write_parquet(out / f"window_{a}.parquet")
            r["spec"].write_parquet(out / f"spectrum_{a}.parquet")
            (out / f"scalars_{a}.json").write_text(json.dumps({k: r[k] for k in ("minute", "win_rmssd", "session_rmssd", "sd1", "sd2")}))
        print("beats:", picks)
