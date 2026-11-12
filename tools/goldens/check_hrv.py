"""Compare the engine's HRV page numbers with the notebook goldens: `check_hrv.py [--version 1] [--dir ../../.local/wellness-v1-mcp]`.

Exit 1 on any failure. Nights whose five-minute window crosses a daylight-saving switch are left out of the comparisons that
depend on minutes since sleep onset: the notebook's local-time casts shift those by an hour, the engine's UTC arithmetic does not.
"""

import argparse
import json
import sys
from datetime import date
from pathlib import Path

import numpy as np
import polars as pl

import wellness_lab

ROOT = Path(__file__).resolve().parents[2]
EPOCH = date(1970, 1, 1)
fails = 0


def frame(t: dict) -> pl.DataFrame:
    cols = {**{k: pl.Series(k, v, dtype=pl.Float64) for k, v in t["num"].items()}, **{k: pl.Series(k, v, dtype=pl.String) for k, v in t["txt"].items()}}
    return pl.DataFrame(cols)


def to_day(df: pl.DataFrame, col: str = "date") -> pl.DataFrame:
    return df.with_columns(day=(pl.col(col) - pl.lit(EPOCH)).dt.total_days().cast(pl.Float64)).drop(col)


def report(name: str, bad: int, total: int, note: str = "") -> None:
    global fails
    ok = bad == 0
    fails += 0 if ok else 1
    print(f"{'PASS' if ok else 'FAIL'}  {name:34} {total - bad}/{total} {note}")


def cmp_cols(name, gold: pl.DataFrame, ours: pl.DataFrame, keys, cols, rtol=1e-9, atol=1e-9, skip=None):
    j = gold.join(ours, on=keys, how="inner", suffix="_o")
    if skip is not None:
        j = j.filter(~skip(j))
    for c in cols:
        a, b = j[c], j[c + "_o"]
        if a.dtype == pl.String or b.dtype == pl.String:
            bad = int(((a != b) | (a.is_null() != b.is_null())).fill_null(False).sum()) + int((a.is_null() != b.is_null()).sum() - ((a != b) | (a.is_null() != b.is_null())).fill_null(False).sum()) * 0
            bad = int((a.fill_null("~") != b.fill_null("~")).sum())
        else:
            x = a.cast(pl.Float64).to_numpy()
            y = b.cast(pl.Float64).to_numpy()
            both = ~np.isnan(x) & ~np.isnan(y)
            bad = int((np.isnan(x) != np.isnan(y)).sum()) + int((~np.isclose(x[both], y[both], rtol=rtol, atol=atol)).sum())
        report(f"{name}.{c}", bad, j.height)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="1")
    ap.add_argument("--dir", default=str(ROOT / ".local" / "wellness-v1-mcp"))
    ap.add_argument("--parts", default="hrv")
    a = ap.parse_args()
    g = ROOT / ".local" / "goldens" / f"hrv-v{a.version}"
    lab = wellness_lab.Lab()
    lab.load_dir(a.dir)
    res, _ = lab.run({"kind": "hrv4t", "range_days": 60, "k_daily": 1.5, "k_swc": 0.5})

    nightly = frame(res["nightly"])
    days = nightly["day"].to_numpy()
    gold_n = to_day(pl.read_parquet(g / "nightly.parquet"))
    # DST nights: the UTC offset of the night's day differs from the one before
    tz = pl.read_parquet(Path(a.dir) / "days.parquet").select("day", "utc_offset_min").sort("day")
    dst = tz.with_columns(chg=pl.col("utc_offset_min") != pl.col("utc_offset_min").shift(1)).filter(pl.col("chg")).get_column("day").cast(pl.Float64).to_list()
    skip_dst = lambda j: j["day"].is_in(dst) | j["day"].is_in([d + 1 for d in dst]) | j["day"].is_in([d - 1 for d in dst])  # noqa: E731
    report("nightly rows", abs(gold_n.height - nightly.height), max(gold_n.height, nightly.height), f"(golden {gold_n.height}, engine {nightly.height})")
    cuts = json.loads((g / "LOAD_CUTS.json").read_text())
    report("LOAD_CUTS", int(not np.allclose(cuts, res["load_cuts"], rtol=1e-9)), 1, f"{cuts} vs {res['load_cuts']}")

    cmp_cols("nightly", gold_n, nightly, ["day"], ["n5", "garmin", "garmin_week", "g_lo", "g_hi", "g_status", "sleep_hr", "sleep_score", "sleep_h", "resp", "sleep_stress", "load", "rmssd_src", "rmssd", "lnr", "load_class"])
    cmp_cols("nightly(dst)", gold_n, nightly, ["day"], ["rmssd_4h"], skip=skip_dst)
    cmp_cols("rhr", gold_n.filter(pl.col("rhr").is_not_null()), nightly, ["day"], ["rhr"], rtol=0, atol=0.5)  # ours also falls back to daily_summary

    sc = frame(res["scored"])
    gold_s = to_day(pl.read_parquet(g / "scored.parquet"))
    cmp_cols("scored", gold_s, sc, ["day"], ["base7", "mu", "sd", "lo", "hi", "swc_lo", "swc_hi", "advice", "score", "base_score"])
    gold_t = to_day(pl.read_parquet(g / "trend_scored.parquet"))
    cmp_cols("trend", gold_t, sc, ["day"], ["cv7", "norm", "norm7", "hr7", "cv7s", "z_hrv", "z_hr", "z_cv", "z_norm", "trend"])

    lag = frame(res["acute_lag"]).with_columns(night=pl.col("night").cast(pl.Float64))
    gold_l = pl.read_parquet(g / "acute_lag.parquet").with_columns(night=pl.col("night").cast(pl.Float64))
    cmp_cols("acute_lag", gold_l, lag, ["klass", "night"], ["mean", "se", "n", "lo", "hi"])
    pts = frame(res["acute_pts"])
    gold_p = to_day(pl.read_parquet(g / "acute_pts.parquet"))
    cmp_cols("acute_pts", gold_p, pts, ["day"], ["dev"])

    bc = frame(res["barcode"])
    gold_b = to_day(pl.read_parquet(g / "barcode.parquet")).with_columns(h=pl.col("h").round(3))
    bc = bc.with_columns(h=pl.col("h").round(3))
    cmp_cols("barcode", gold_b, bc, ["day", "h"], ["rmssd"], skip=skip_dst)
    gb = gold_b.filter(~skip_dst(gold_b)).height
    ob = bc.filter(~skip_dst(bc)).height
    report("barcode rows (non-DST)", abs(gb - ob), gb, f"(golden {gb}, engine {ob})")

    sh = frame(res["stage_hrv"])
    cmp_cols("stage_hrv", pl.read_parquet(g / "stage_hrv.parquet").with_columns(n=pl.col("n").cast(pl.Float64)), sh, ["stage"], ["rel", "q1", "q3"], rtol=0.02, atol=0.5)
    ns = frame(res["night_sens"])
    cmp_cols("night_sens", pl.read_parquet(g / "night_sens.parquet"), ns, ["measure"], ["effect", "sd"], rtol=0.05, atol=0.02)

    mv = frame(res["model_view"])
    gold_m = to_day(pl.read_parquet(g / "model_view.parquet"))
    cmp_cols("model_view.pred", gold_m, mv, ["day"], ["pred", "pred_sd", "level", "z"], rtol=1e-3, atol=1e-3)
    gm = json.loads((g / "MODEL.json").read_text())
    om = res["model"]
    for k in ["beta", "tau", "sigma_eps", "sigma_eta", "level", "level_var", "g", "pct_per_100", "half_life_days"]:
        report(f"MODEL.{k}", int(not np.isclose(gm[k], om[k], rtol=1e-3, atol=1e-6)), 1, f"{gm[k]:.6g} vs {om[k]:.6g}")
    report("MODEL.lr", int(not np.isclose(gm["lr"], om["lr"], rtol=1e-3, atol=1e-3)), 1, f"{gm['lr']:.6g} vs {om['lr']:.6g}")
    report("MODEL.nights", int(gm["nights"] != om["nights"]), 1)
    parts = set(a.parts.split(","))
    if "a1" in parts:
        check_a1(lab, g)
    if "beats" in parts:
        check_beats(lab, g)
    print("FAILED" if fails else "ALL PASS", fails)
    return 1 if fails else 0


def check_a1(lab, g):
    res, _ = lab.run({"kind": "dfa"})
    acts = frame(res["acts"])
    ids = acts["id"].cast(pl.Int64)
    # the engine's window table refers to recordings by their row in `recordings`, listed in acts.act
    act_rows = acts["act"].to_list()
    rec = {int(r): int(i) for r, i in zip(act_rows, ids.to_list())}
    w = frame(res["windows"]).with_columns(activity_id=pl.col("act").cast(pl.Int64).replace_strict(rec)).drop("act")
    gold = pl.read_parquet(g / "a1_windows.parquet").filter(pl.col("activity_id").is_in(ids.to_list())).with_columns(t_s=pl.col("t_s").round(3))
    w = w.with_columns(t_s=pl.col("t_s").round(3))
    report("a1 windows (rows)", abs(gold.height - w.height), gold.height, f"(golden {gold.height}, engine {w.height})")
    cmp_cols("a1", gold.with_columns(pl.col("activity_id").cast(pl.Float64)), w.with_columns(pl.col("activity_id").cast(pl.Float64)), ["activity_id", "t_s"], ["hr", "a1", "art", "rep"], rtol=1e-6, atol=1e-9)
    gn = json.loads((g / "A1_NOW.json").read_text())
    for sp, v in gn.items():
        o = res["now"].get(sp, {})
        for k in ("vt1", "vt2", "vt1_all", "vt2_all"):
            a_, b_ = v.get(k), o.get(k)
            ok = (a_ is None and b_ is None) or (a_ is not None and b_ is not None and abs(a_ - b_) < 0.5)
            report(f"A1_NOW.{sp}.{k}", int(not ok), 1, f"{a_} vs {b_}")


def check_beats(lab, g):
    s, _ = lab.run({"kind": "sessions"})
    ours = frame(s).with_columns(activity_id=pl.col("id").cast(pl.Int64)).drop("id")
    gold = pl.read_parquet(g / "rr_sessions.parquet").with_columns(beats=pl.col("beats").cast(pl.Float64))
    report("sessions (rows)", abs(gold.height - ours.height), gold.height, f"(golden {gold.height}, engine {ours.height})")
    cmp_cols("sessions", gold, ours.with_columns(pl.col("activity_id")), ["activity_id"], ["beats", "repeat_share", "artifact_share", "quality"])
    for aid in json.loads((g / "beats_ids.json").read_text()):
        d, _ = lab.run({"kind": "beats", "id": str(aid)})
        sc = json.loads((g / f"scalars_{aid}.json").read_text())
        report(f"beats.{aid}.minute", int(sc["minute"] != d["window_minute"]), 1, f"{sc['minute']} vs {d['window_minute']}")
        for k, ko in (("win_rmssd", "window_rmssd"), ("session_rmssd", "session_rmssd")):
            report(f"beats.{aid}.{k}", int(not np.isclose(sc[k], d[ko], rtol=1e-9)), 1, f"{sc[k]:.6g} vs {d[ko]:.6g}")
        for k in ("sd1", "sd2"):
            report(f"beats.{aid}.{k}", int(not np.isclose(sc[k], d["poincare"][k], rtol=1e-9)), 1, f"{sc[k]:.6g} vs {d['poincare'][k]:.6g}")
        spec = pl.read_parquet(g / f"spectrum_{aid}.parquet")
        report(f"beats.{aid}.spectrum", int(not np.allclose(spec["p"].to_numpy(), np.array(d["spectrum"]["p"]), rtol=1e-6, atol=1e-9)), 300)
        gb = pl.read_parquet(g / f"beats_{aid}.parquet")
        report(f"beats.{aid}.rows", abs(gb.height - len(d["beats"]["num"]["rr"])), gb.height)
        gw = pl.read_parquet(g / f"window_{aid}.parquet")
        report(f"beats.{aid}.window rows", abs(gw.height - d["window"]["n"]), gw.height)


if __name__ == "__main__":
    sys.exit(main())
