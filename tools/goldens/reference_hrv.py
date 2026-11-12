"""The HRV deep-dive notebook's computations as plain Python, the reference the goldens are made from.

Transcribed on 2026-11-12 from
~/Projects/homelab/marimo/data/projects/proj-04vwegq2rf4cdtza/notebooks/nb-ycda538mxkv4h0ct/workspace/notebook.py
(cells connect, nightly_load, scored_base, scored, trend_scored, acute_calc, night_calc, model_fit, headline, spark;
cells a1_build, a1_thresholds, durability_chart, rr_sessions, beats, window_*, poincare_chart in the second half).
The cell bodies are verbatim; the marimo widgets are function parameters, and the snapshot is attached from a file.

One deliberate difference: the training load excludes deleted activities (`where deleted_at is null`), because
wellness-v1's activities table has none. A second: the five-minute values are de-duplicated (`select distinct`), because the same reading appears in two
wellness files for some nights (identical values) and wellness-v1's hrv5 series keeps one reading per instant. Nothing else is changed. Local-time casts use America/Denver, as in the notebook.
"""

from __future__ import annotations

import duckdb
import numpy as np
import polars as pl
from scipy.optimize import minimize
from scipy.signal import lombscargle


def connect(snapshot: str):
    con = duckdb.connect()
    con.execute(f"ATTACH '{snapshot}' AS garmin (READ_ONLY)")
    con.execute("SET TimeZone = 'America/Denver'")

    def q(sql):
        return con.sql(sql).pl()

    return q


def nightly_load(q):
    five = q("""
        select v.calendar_date as date, v.timestamp::timestamp as ts, v.value as rmssd,
               s.sleep_start_timestamp_gmt::timestamp as onset
        from (select distinct calendar_date, timestamp, value from garmin.fit_hrv_value) v
        left join garmin.sleep s using (calendar_date)
        where v.value > 0
    """).with_columns(min_in=(pl.col("ts") - pl.col("onset")).dt.total_seconds() / 60)

    _per = five.group_by("date").agg(
        rmssd=pl.col("rmssd").mean(),
        n5=pl.len(),
        rmssd_4h=pl.col("rmssd").filter(pl.col("min_in").is_between(30, 270)).mean(),
    )
    _g = q("""
        select h.calendar_date as date, h.last_night_avg as garmin, h.weekly_avg as garmin_week,
               h.baseline_balanced_low as g_lo, h.baseline_balanced_upper as g_hi, h.status as g_status,
               s.avg_heart_rate as sleep_hr, coalesce(s.resting_heart_rate, d.resting_heart_rate) as rhr,
               s.sleep_score, s.sleep_time_seconds / 3600 as sleep_h, s.average_respiration_value as resp,
               s.avg_sleep_stress as sleep_stress, s.sleep_start_timestamp_gmt::timestamp as onset
        from garmin.hrv h
        left join garmin.sleep s using (calendar_date)
        left join garmin.heart_rate_day d using (calendar_date)
    """)
    _load = q("""
        select start_time_local::date + 1 as date, sum(coalesce(activity_training_load, 0)) as load,
               sum(duration) / 3600 as train_h
        from garmin.activity where deleted_at is null group by 1
    """)
    _days = pl.DataFrame({"date": pl.date_range(_g["date"].min(), _g["date"].max(), "1d", eager=True)})
    _n = (
        _days.join(_g, on="date", how="left").join(_per, on="date", how="left")
        .join(_load, on="date", how="left")  # the night ending on `date` follows the training of the day before
        .with_columns(pl.col("load", "train_h").fill_null(0.0))
        .sort("date")
    )
    _pos = _n.filter(pl.col("load") > 5)["load"]
    LOAD_CUTS = (float(_pos.quantile(1 / 3)), float(_pos.quantile(2 / 3)))
    nightly = _n.with_columns(
        # the 5-minute values reach the snapshot a day after Garmin's summary: fall back to Garmin's number until then
        rmssd_src=pl.when(pl.col("rmssd").is_not_null()).then(pl.lit("5-min values")).when(pl.col("garmin").is_not_null()).then(pl.lit("Garmin")),
        rmssd=pl.coalesce("rmssd", pl.col("garmin").cast(pl.Float64)),
    ).with_columns(
        lnr=pl.col("rmssd").log(),
        load_class=pl.when(pl.col("load") <= 5).then(pl.lit("rest"))
        .when(pl.col("load") <= LOAD_CUTS[0]).then(pl.lit("easy"))
        .when(pl.col("load") <= LOAD_CUTS[1]).then(pl.lit("moderate"))
        .otherwise(pl.lit("hard")),
    )
    return LOAD_CUTS, five, nightly


def scored_base(nightly, range_days):
    _ln = nightly["lnr"].to_numpy().astype(float)
    _n, _w = len(_ln), range_days
    _base7, _mu, _sd = np.full(_n, np.nan), np.full(_n, np.nan), np.full(_n, np.nan)
    for _i in range(_n):
        _a = _ln[max(0, _i - 6): _i + 1]
        _a = _a[~np.isnan(_a)]
        if len(_a) >= 4:
            _base7[_i] = _a.mean()
        _h = _ln[max(0, _i - _w): _i]  # the nights before tonight
        _h = _h[~np.isnan(_h)]
        if len(_h) >= max(14, _w // 3):
            _mu[_i], _sd[_i] = _h.mean(), _h.std(ddof=1)
    return nightly.with_columns(
        base7=pl.Series(_base7).fill_nan(None), mu=pl.Series(_mu).fill_nan(None), sd=pl.Series(_sd).fill_nan(None)
    )


def scored(scored_base, k_daily, k_swc):
    _low = pl.col("lnr") < pl.col("lo")
    _high = pl.col("lnr") > pl.col("hi")
    _base_low = pl.col("base7") < pl.col("swc_lo")
    return scored_base.with_columns(
        lo=pl.col("mu") - k_daily * pl.col("sd"), hi=pl.col("mu") + k_daily * pl.col("sd"),
        swc_lo=pl.col("mu") - k_swc * pl.col("sd"), swc_hi=pl.col("mu") + k_swc * pl.col("sd"),
    ).with_columns(
        advice=pl.when(pl.col("lnr").is_null() | pl.col("mu").is_null()).then(pl.lit(None, pl.String))
        .when(_base_low & _low).then(pl.lit("back off"))
        .when(_base_low).then(pl.lit("keep it easy"))
        .when(_low).then(pl.lit("watch"))
        .when(_high).then(pl.lit("high"))
        .otherwise(pl.lit("normal")),
        score=20 * pl.col("lnr"),
        base_score=20 * pl.col("base7"),
    )


def trend_scored(scored_base):
    _d = scored_base.with_columns(
        cv7=pl.col("lnr").rolling_std(7, min_samples=4) / pl.col("lnr").rolling_mean(7, min_samples=4) * 100,
        norm=pl.col("rmssd") / (60000 / pl.col("sleep_hr")),
    ).with_columns(
        norm7=pl.col("norm").rolling_mean(7, min_samples=4),
        hr7=pl.col("sleep_hr").rolling_mean(7, min_samples=4),
        cv7s=pl.col("cv7").rolling_mean(7, min_samples=4),
    )
    _win = 14

    def _slopes(x):
        _t = np.arange(_win, dtype=float)
        out = np.full(len(x), np.nan)
        for i in range(_win - 1, len(x)):
            w = x[i - _win + 1: i + 1]
            ok = ~np.isnan(w)
            if ok.sum() >= _win * 0.6:
                tt = _t[ok] - _t[ok].mean()
                out[i] = (tt * (w[ok] - w[ok].mean())).sum() / (tt ** 2).sum()
        return out

    def _zs(s, warmup=60):
        z = np.full(len(s), np.nan)
        for i in range(len(s)):
            h = s[:i][~np.isnan(s[:i])]
            if len(h) >= warmup and not np.isnan(s[i]):
                z[i] = (s[i] - h.mean()) / h.std()
        return z

    _z = {k: _zs(_slopes(_d[c].to_numpy().astype(float))) for k, c in [("hrv", "base7"), ("hr", "hr7"), ("cv", "cv7s"), ("norm", "norm7")]}
    _up = {k: v > 1 for k, v in _z.items()}
    _dn = {k: v < -1 for k, v in _z.items()}
    _label = np.select(
        [_dn["hrv"] & _up["hr"], _dn["hrv"] & _dn["hr"], _up["hr"] & _up["cv"], ~_dn["hrv"] & ~_up["hr"] & _dn["cv"]],
        ["accumulated fatigue", "saturation", "maladaptation", "coping well"], "stable",
    )
    return _d.with_columns(
        z_hrv=pl.Series(_z["hrv"]).fill_nan(None), z_hr=pl.Series(_z["hr"]).fill_nan(None),
        z_cv=pl.Series(_z["cv"]).fill_nan(None), z_norm=pl.Series(_z["norm"]).fill_nan(None),
        trend=pl.Series(np.where(np.isnan(_z["hrv"]), "warming up", _label)),
    )


def acute_calc(trend_scored):
    acute_pts = trend_scored.with_columns(dev=pl.col("lnr") - pl.col("base7").shift(1))
    acute_lag = pl.concat([
        trend_scored.with_columns(dk=pl.col("lnr").shift(-(k - 1)) - pl.col("base7").shift(1), klass=pl.col("load_class"))
        .group_by("klass").agg(mean=pl.col("dk").mean(), se=pl.col("dk").std() / pl.col("dk").count().sqrt(), n=pl.col("dk").count())
        .with_columns(night=pl.lit(k))
        for k in (1, 2, 3)
    ]).with_columns(lo=pl.col("mean") - 1.96 * pl.col("se"), hi=pl.col("mean") + 1.96 * pl.col("se"))
    return acute_lag, acute_pts


def night_calc(five, nightly, q, trend_scored):
    _stage = q("""
        select timestamp::timestamp as ts, sleep_level as stage from garmin.fit_sleep_level order by ts
    """)
    _fv = five.sort("ts").join_asof(_stage.sort("ts"), on="ts", strategy="backward", tolerance="3h")
    barcode = (
        _fv.filter(pl.col("min_in").is_between(0, 600))
        .with_columns(h=(pl.col("min_in") // 10 * 10 / 60).round(3))
        .group_by("date", "h").agg(rmssd=pl.col("rmssd").mean())
        .sort("date", "h")
    )
    stage_hrv = (
        _fv.filter(pl.col("stage").is_in(["deep", "light", "rem", "awake"]))
        .join(nightly.select("date", "rmssd").rename({"rmssd": "night"}), on="date")
        .with_columns(rel=(pl.col("rmssd") / pl.col("night")).log() * 100)
        .group_by("stage").agg(rel=pl.col("rel").median(), q1=pl.col("rel").quantile(0.25), q3=pl.col("rel").quantile(0.75), n=pl.len())
    )
    _t = trend_scored.with_columns(
        l4=pl.col("rmssd_4h").log(), lg=pl.col("garmin").cast(pl.Float64).log()
    )
    _rows = []
    for _m, _c in [("whole night", "lnr"), ("first 4 h", "l4"), ("Garmin", "lg")]:
        _dev = _t.with_columns(dv=pl.col(_c) - pl.col(_c).rolling_mean(7, min_samples=4).shift(1))
        _sd = _dev["dv"].std()
        _h = _dev.filter(pl.col("load_class") == "hard")["dv"].mean()
        _r = _dev.filter(pl.col("load_class") == "rest")["dv"].mean()
        _rows.append(dict(measure=_m, effect=(_r - _h) / _sd, sd=_sd))
    night_sens = pl.DataFrame(_rows)
    return barcode, night_sens, stage_hrv


def model_fit(nightly):
    def kalman_ir(y, load, th):
        se, sn, beta, tau = np.exp(th[0]), np.exp(th[1]), th[2], np.exp(th[3])
        decay = np.exp(-1.0 / tau)
        mu, P, g, ll = np.nanmean(y[:14]), 1.0, 0.0, 0.0
        out = np.full((len(y), 6), np.nan)
        for t in range(len(y)):
            g = decay * g + load[t] / 100
            Pp = P + sn ** 2
            pred, F = mu + beta * g, Pp + se ** 2
            if np.isfinite(y[t]):
                v = y[t] - pred
                mu, P = mu + Pp / F * v, Pp * se ** 2 / F
                ll += -0.5 * (np.log(2 * np.pi * F) + v * v / F)
                z = v / np.sqrt(F)
            else:
                P, z = Pp, np.nan
            out[t] = (pred, F, mu, P, g, z)
        return ll, out

    _y = nightly["lnr"].to_numpy().astype(float)
    _L = nightly["load"].to_numpy().astype(float)
    _x0 = np.array([np.log(0.12), np.log(0.02), -0.03, np.log(2.0)])
    _r = minimize(lambda th: -kalman_ir(_y, _L, th)[0] if -3 < th[3] < 3.5 else 1e9, _x0, method="Nelder-Mead", options=dict(maxiter=4000, xatol=1e-5, fatol=1e-6))
    _r0 = minimize(lambda th: -kalman_ir(_y, _L * 0, np.array([th[0], th[1], 0.0, 0.0]))[0], _x0[:2], method="Nelder-Mead")
    _ll, _out = kalman_ir(_y, _L, _r.x)
    _se, _sn, _beta, _tau = np.exp(_r.x[0]), np.exp(_r.x[1]), _r.x[2], np.exp(_r.x[3])
    model_view = nightly.select("date", "lnr", "rmssd", "load", "load_class").with_columns(
        pred=pl.Series(_out[:, 0]), pred_sd=pl.Series(np.sqrt(_out[:, 1])), level=pl.Series(_out[:, 2]), z=pl.Series(_out[:, 5]).fill_nan(None),
    )
    MODEL = dict(
        beta=float(_beta), tau=float(_tau), sigma_eps=float(_se), sigma_eta=float(_sn),
        level=float(_out[-1, 2]), level_var=float(_out[-1, 3]), g=float(_out[-1, 4]),
        pct_per_100=float((np.exp(_beta) - 1) * 100), half_life_days=float(_tau * np.log(2)),
        lr=float(2 * (_ll + _r0.fun)), nights=int(np.isfinite(_y).sum()),
        last_date=nightly["date"].max().isoformat(),
    )
    return MODEL, model_view


def run_all(snapshot: str, range_days: int = 60, k_daily: float = 1.5, k_swc: float = 0.5):
    """Every cell the HRV page needs, for one setting of the controls."""
    q = connect(snapshot)
    LOAD_CUTS, five, nightly = nightly_load(q)
    sb = scored_base(nightly, range_days)
    sc = scored(sb, k_daily, k_swc)
    ts = trend_scored(sb)
    lag, pts = acute_calc(ts)
    barcode, sens, stage_hrv = night_calc(five, nightly, q, ts)
    MODEL, mv = model_fit(nightly)
    return dict(LOAD_CUTS=LOAD_CUTS, nightly=nightly, scored=sc, trend_scored=ts, acute_lag=lag, acute_pts=pts, barcode=barcode,
                night_sens=sens, stage_hrv=stage_hrv, model_view=mv, MODEL=MODEL)
