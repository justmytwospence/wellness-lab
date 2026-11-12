//! Episode detection: unusual runs of nights (fused across signals), exposures that are certain (a change of time zone,
//! altitude), and the policy that decides which of them to ask the user about.
//! Methods: Hotelling T-squared on personal robust z-scores with a shrunk covariance; CUSUM (Page 1954; Mishra 2020) and
//! the two-night resting-heart-rate rule (Alavi 2022) on nocturnal heart rate; the classifier is in `classify`.

use crate::baseline::{baseline, BaselineSpec, Center, Scale, Transform};
use crate::classify::{self, Features, Learned, MODEL_VERSION};
use crate::data::{Data, Table};
use crate::labels::{
    excluded_days, Evidence, Label, LabelKind, Proposal, ProposalStatus, Settings,
};
use crate::math::Det;
use crate::nights::night_features;
use crate::stats::{self, chi2_sf, invert, mean, median};
use crate::time::{days_from_ymd, weekday, ymd};
use std::collections::{BTreeMap, BTreeSet};

/// (column, sign toward strain, smallest scale worth dividing by)
const SIGNALS: [(&str, f64, f64); 6] = [
    ("hr_min30", 1.0, 1.0),
    ("ln_rmssd", -1.0, 0.05),
    ("resp_mean", 1.0, 0.3),
    ("sleep_stress", 1.0, 2.0),
    ("skin_temp", 1.0, 0.1),
    ("rest_frac", -1.0, 0.05),
];

fn robust_spec(window: usize) -> BaselineSpec {
    BaselineSpec {
        transform: Transform::None,
        center: Center::Median,
        scale: Scale::Mad,
        window,
        min_n: 14,
        short: 3,
        short_min: 2,
        k_night: 2.0,
        k_short: 1.0,
        exclude_labelled: true,
    }
}

/// Days of a US (second Sunday of March, first of November) or EU (last Sunday of March and October) clock change.
fn is_dst_switch(day: i32) -> bool {
    let (y, m, d) = ymd(day);
    if weekday(day) != 6 {
        return false;
    }
    let nth_sunday = |month: u32, n: u32| -> i32 {
        let first = days_from_ymd(y, month, 1);
        let off = (6 - weekday(first) as i32).rem_euclid(7);
        first + off + 7 * (n as i32 - 1)
    };
    let last_sunday = |month: u32| -> i32 {
        let end = days_from_ymd(y, month, 31);
        end - ((weekday(end) as i32 - 6).rem_euclid(7))
    };
    let _ = (m, d);
    day == nth_sunday(3, 2)
        || day == nth_sunday(11, 1)
        || day == last_sunday(3)
        || day == last_sunday(10)
}

struct Ctx<'a> {
    nt: &'a Table,
    days: Vec<i32>,
    centers: BTreeMap<&'static str, Vec<f64>>,
    el_center: Vec<f64>,
}

fn episode_features(c: &Ctx, rows: &[usize]) -> Features {
    let m = |col: &str, base: &[f64]| -> f64 {
        let v = c.nt.col(col);
        let d: Vec<f64> = rows
            .iter()
            .map(|&i| v[i] - base[i])
            .filter(|x| x.is_finite())
            .collect();
        if d.is_empty() {
            f64::NAN
        } else {
            mean(&d)
        }
    };
    let none = vec![f64::NAN; c.nt.n];
    let cen = |k: &str| c.centers.get(k).unwrap_or(&none).clone();
    let first = rows[0];
    let class = c.nt.text("load_class");
    Features {
        d_rhr: m("hr_min30", &cen("hr_min30")),
        d_lnrmssd: m("ln_rmssd", &cen("ln_rmssd")),
        d_resp: m("resp_mean", &cen("resp_mean")),
        d_temp: m("skin_temp", &cen("skin_temp")),
        d_stress: m("sleep_stress", &cen("sleep_stress")),
        d_early_late: m("hr_early_late", &c.el_center),
        nights: rows.len(),
        hard_day_before: class[first].as_deref() == Some("hard"),
        weekend: weekday(c.days[first]) >= 5,
    }
}

fn rows_in(days: &[i32], a: i32, b: i32) -> Vec<usize> {
    (0..days.len())
        .filter(|&i| days[i] >= a && days[i] <= b)
        .collect()
}

/// What the answers so far have taught the classifier.
pub fn learn(
    c_features: impl Fn(i32, i32) -> Option<Features>,
    labels: &[Label],
    existing: &[Proposal],
) -> Learned {
    let mut l = Learned::default();
    for lb in labels.iter().filter(|l| {
        l.live()
            && matches!(
                l.source,
                crate::labels::Source::Confirmed | crate::labels::Source::User
            )
    }) {
        if classify::CLASSES.contains(&lb.kind) {
            if let Some(f) = c_features(lb.start_day, lb.end_day) {
                l.confirmed.entry(lb.kind).or_default().push(f);
            }
        }
    }
    for p in existing
        .iter()
        .filter(|p| p.status == ProposalStatus::Denied && !p.auto)
    {
        if let Some(f) = c_features(p.start_day, p.end_day) {
            l.denied.push(f);
        }
    }
    l
}

pub fn detect(
    data: &Data,
    labels: &[Label],
    settings: &Settings,
    existing: &[Proposal],
) -> Vec<Proposal> {
    let nt = night_features(data, labels, settings);
    detect_with(data, &nt, labels, settings, existing)
}

/// As `detect`, with the night table already computed.
pub fn detect_with(
    data: &Data,
    nt: &Table,
    labels: &[Label],
    settings: &Settings,
    existing: &[Proposal],
) -> Vec<Proposal> {
    let n = nt.n;
    if n == 0 {
        return vec![];
    }
    let days: Vec<i32> = nt.col("day").iter().map(|&d| d as i32).collect();
    let excluded = excluded_days(labels);
    let spec = robust_spec(settings.baseline_window);
    let mut centers: BTreeMap<&'static str, Vec<f64>> = BTreeMap::new();
    let mut zs: Vec<(usize, Vec<f64>)> = vec![]; // signal index, strain z per night
    let last90 = n.saturating_sub(90);
    for (k, &(col, sign, floor)) in SIGNALS.iter().enumerate() {
        let v = nt.col(col);
        if (v[last90..].iter().filter(|x| x.is_finite()).count() as f64) < 0.6 * (n - last90) as f64
        {
            continue;
        }
        let b = baseline(&v, &days, &excluded, &spec);
        let z: Vec<f64> = (0..n)
            .map(|i| {
                if v[i].is_finite() && b.center[i].is_finite() {
                    sign * (v[i] - b.center[i]) / b.scale[i].max(floor)
                } else {
                    f64::NAN
                }
            })
            .collect();
        centers.insert(col, b.center.clone());
        zs.push((k, z));
    }
    let el = baseline(&nt.col("hr_early_late"), &days, &excluded, &spec);
    let ctx = Ctx {
        nt,
        days: days.clone(),
        centers,
        el_center: el.center,
    };
    let mut out: Vec<Proposal> = vec![];

    if zs.len() >= 3 && n >= 30 {
        // fusion
        let kk = zs.len();
        let mut pval = vec![f64::NAN; n];
        let mut strain = vec![false; n];
        for i in 20..n {
            let avail: Vec<usize> = (0..kk).filter(|&s| zs[s].1[i].is_finite()).collect();
            if avail.len() < 3 {
                continue;
            }
            let hist: Vec<usize> = (i.saturating_sub(120)..i)
                .filter(|&r| {
                    !excluded.contains(&days[r]) && avail.iter().all(|&s| zs[s].1[r].is_finite())
                })
                .collect();
            if hist.len() < 40 {
                continue;
            }
            let m = avail.len();
            let mut r = vec![vec![0.0; m]; m];
            for a in 0..m {
                for b in 0..m {
                    let xa: Vec<f64> = hist.iter().map(|&h| zs[avail[a]].1[h]).collect();
                    let xb: Vec<f64> = hist.iter().map(|&h| zs[avail[b]].1[h]).collect();
                    let c = if a == b { 1.0 } else { stats::corr(&xa, &xb) };
                    r[a][b] = 0.8 * c + if a == b { 0.2 } else { 0.0 };
                }
            }
            let Some(inv) = invert(&r) else { continue };
            let z: Vec<f64> = avail.iter().map(|&s| zs[s].1[i]).collect();
            let t2: f64 = (0..m)
                .map(|a| (0..m).map(|b| z[a] * inv[a][b] * z[b]).sum::<f64>())
                .sum();
            pval[i] = chi2_sf(t2, m);
            strain[i] = mean(&z) > 0.0;
        }
        // signals on nocturnal heart rate
        let hr_z = zs.iter().find(|(k, _)| *k == 0).map(|(_, z)| z.clone());
        let mut alarm: BTreeMap<usize, &'static str> = BTreeMap::new();
        if let Some(z) = &hr_z {
            let mut s = 0.0;
            for i in 0..n {
                if z[i].is_finite() {
                    s = (s + z[i] - 0.5).max(0.0);
                    if s > 4.0 {
                        alarm.insert(i, "cusum");
                        s = 0.0;
                    }
                }
            }
        }
        let hr = nt.col("hr_min30");
        for i in 1..n {
            let prev: Vec<f64> = (i.saturating_sub(28)..i)
                .filter(|&r| !excluded.contains(&days[r]))
                .map(|r| hr[r])
                .collect();
            let prev1: Vec<f64> = (i.saturating_sub(29)..i - 1)
                .filter(|&r| !excluded.contains(&days[r]))
                .map(|r| hr[r])
                .collect();
            if prev.len() >= 14
                && prev1.len() >= 14
                && hr[i] - median(&prev) >= 4.0
                && hr[i - 1] - median(&prev1) >= 4.0
                && days[i] - days[i - 1] == 1
            {
                alarm.insert(i, "night_signal");
            }
        }
        let flagged: BTreeSet<usize> = (0..n)
            .filter(|&i| strain[i] && pval[i] < 0.05)
            .chain(alarm.keys().copied())
            .collect();
        let mut groups: Vec<Vec<usize>> = vec![];
        for &i in &flagged {
            match groups.last_mut() {
                Some(g) if i - g.last().unwrap() <= 2 => g.push(i),
                _ => groups.push(vec![i]),
            }
        }
        // classifier knowledge from the user's answers
        let feat = |a: i32, b: i32| -> Option<Features> {
            let rows = rows_in(&days, a, b);
            (!rows.is_empty()).then(|| episode_features(&ctx, &rows))
        };
        let learned = learn(feat, labels, existing);
        for g in groups {
            let strong = g.iter().any(|&i| pval[i] < 0.01 && strain[i]);
            let has_alarm = g.iter().any(|i| alarm.contains_key(i));
            if g.len() < 2 && !strong && !has_alarm {
                continue;
            }
            let (a, b) = (days[g[0]], days[*g.last().unwrap()]);
            if labels
                .iter()
                .any(|l| l.live() && l.start_day <= b && l.end_day >= a)
            {
                continue;
            }
            let rows: Vec<usize> = (g[0]..=*g.last().unwrap()).collect();
            let f = episode_features(&ctx, &rows);
            let post = classify::posterior(&f, &learned);
            let (top, _) = post
                .iter()
                .max_by(|x, y| x.1.partial_cmp(y.1).unwrap())
                .map(|(k, v)| (*k, *v))
                .unwrap();
            let pmin = g
                .iter()
                .map(|&i| pval[i])
                .filter(|p| p.is_finite())
                .fold(f64::INFINITY, f64::min);
            let mut signals: Vec<String> = vec![];
            if g.iter().any(|&i| pval[i] < 0.05 && strain[i]) {
                signals.push("fusion".into());
            }
            for i in &g {
                if let Some(s) = alarm.get(i) {
                    if !signals.iter().any(|x| x == s) {
                        signals.push(s.to_string());
                    }
                }
            }
            let ev = Evidence {
                nights: f.nights,
                d_rhr: f.d_rhr.is_finite().then_some(f.d_rhr),
                d_lnrmssd: f.d_lnrmssd.is_finite().then_some(f.d_lnrmssd),
                d_resp: f.d_resp.is_finite().then_some(f.d_resp),
                d_temp: f.d_temp.is_finite().then_some(f.d_temp),
                d_stress: f.d_stress.is_finite().then_some(f.d_stress),
                d_early_late: f.d_early_late.is_finite().then_some(f.d_early_late),
                p_value: pmin.is_finite().then_some(pmin),
                hard_day_before: f.hard_day_before,
                weekend: f.weekend,
                tz_overlap: false,
                alt_overlap: false,
                signals,
            };
            let score = if has_alarm {
                1.0
            } else if pmin.is_finite() {
                1.0 - pmin
            } else {
                0.5
            };
            out.push(Proposal {
                id: format!("{}:{}", kind_name(top), a),
                kind: top,
                start_day: a,
                end_day: b,
                posterior: post,
                evidence: ev,
                score,
                priority: 0.0,
                status: ProposalStatus::Open,
                auto: false,
                asked: false,
                created_at: String::new(),
                answered_at: None,
                notified_at: None,
                snoozed_until: None,
                model_version: MODEL_VERSION.into(),
            });
        }
    }

    // lasting shifts: a medication, a new device, fitness; asked about rather than guessed at
    for col in ["ln_rmssd", "hr_min30"] {
        let v = nt.col(col);
        for t in changepoints(&v) {
            let day = days[t];
            let id = format!("medication_change:{day}");
            if out
                .iter()
                .any(|p| (p.start_day - day).abs() <= 7 && !is_exposure(p))
                || labels
                    .iter()
                    .any(|l| l.live() && (day - l.start_day).abs() <= 7)
                || out.iter().any(|p| p.id == id)
            {
                continue;
            }
            let mut post = BTreeMap::new();
            post.insert(LabelKind::MedicationChange, 0.5);
            post.insert(LabelKind::Other, 0.5);
            out.push(Proposal {
                id,
                kind: LabelKind::MedicationChange,
                start_day: day,
                end_day: day,
                posterior: post,
                evidence: Evidence {
                    nights: 1,
                    signals: vec!["changepoint".into(), col.into()],
                    ..Default::default()
                },
                score: 0.8,
                priority: 0.0,
                status: ProposalStatus::Open,
                auto: false,
                asked: false,
                created_at: String::new(),
                answered_at: None,
                notified_at: None,
                snoozed_until: None,
                model_version: MODEL_VERSION.into(),
            });
        }
    }

    // exposures that are certain
    let off: Vec<i32> = days.iter().map(|&d| data.offset_of_day(d)).collect();
    let tz = nt.col("tz_change");
    let mut exposures: Vec<Proposal> = vec![];
    if settings.auto_label_travel {
        for i in 0..n {
            if tz[i].is_finite() && tz[i] != 0.0 {
                let prior_const =
                    (i.saturating_sub(30)..i).all(|r| off[r] == off[i.saturating_sub(1)]);
                if tz[i].abs() == 60.0 && is_dst_switch(days[i]) && prior_const {
                    continue;
                }
                let (a, b) = (days[i], days[i] + 3.max((tz[i].abs() / 60.0) as i32));
                exposures.push(exposure(
                    LabelKind::Travel,
                    a,
                    b,
                    format!("{:+} min", tz[i] as i32),
                ));
            }
        }
    }
    if settings.auto_label_altitude {
        let alt = nt.col("altitude");
        let home = settings
            .home_altitude_m
            .unwrap_or_else(|| median(&alt[n.saturating_sub(90)..]));
        let thresh = (home + 700.0).max(1500.0);
        let mut run: Option<(i32, i32)> = None;
        for i in 0..n {
            if alt[i].is_finite() && alt[i] >= thresh {
                run = Some(match run {
                    Some((a, b)) if days[i] - b <= 1 => (a, days[i]),
                    Some((a, b)) => {
                        exposures.push(exposure(LabelKind::Altitude, a, b, String::new()));
                        (days[i], days[i])
                    }
                    None => (days[i], days[i]),
                });
            }
        }
        if let Some((a, b)) = run {
            exposures.push(exposure(LabelKind::Altitude, a, b, String::new()));
        }
    }
    exposures.retain(|e| {
        !labels.iter().any(|l| {
            l.live() && l.kind == e.kind && l.start_day <= e.end_day && l.end_day >= e.start_day
        })
    });
    // an episode that overlaps travel or altitude is flagged, and asked about less
    for p in out.iter_mut() {
        let tz_o = exposures.iter().any(|e| {
            e.kind == LabelKind::Travel && e.start_day <= p.end_day && e.end_day >= p.start_day
        });
        let alt_o = exposures.iter().any(|e| {
            e.kind == LabelKind::Altitude && e.start_day <= p.end_day && e.end_day >= p.start_day
        });
        p.evidence.tz_overlap = tz_o;
        p.evidence.alt_overlap = alt_o;
    }
    out.extend(exposures);
    let today = days.last().copied().unwrap_or(0);
    apply_policy(out, existing, settings, today)
}

/// Bayesian online changepoint detection (Adams and MacKay 2007) with a Normal-Gamma model and hazard 1/250: the nights where
/// a lasting shift most likely began, confirmed by the next 14 nights sitting more than half a standard deviation from the 28 before.
pub fn changepoints(x: &[f64]) -> Vec<usize> {
    use crate::math::ln_gamma;
    let n = x.len();
    let first: Vec<f64> = x
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .take(28)
        .collect();
    if first.len() < 20 {
        return vec![];
    }
    let (mu0, var0) = (mean(&first), stats::std(&first, 1).powi(2).max(1e-9));
    let hazard = 1.0 / 250.0;
    // parameters per run length
    let mut mu = vec![mu0];
    let mut kappa = vec![1.0];
    let mut alpha = vec![1.0];
    let mut beta = vec![var0];
    let mut probs = vec![1.0];
    let mut prev_best = 0usize;
    let mut out = vec![];
    for t in 0..n {
        if !x[t].is_finite() {
            continue;
        }
        let mut pred: Vec<f64> = Vec::with_capacity(mu.len());
        for r in 0..mu.len() {
            let df = 2.0 * alpha[r];
            let scale2 = beta[r] * (kappa[r] + 1.0) / (alpha[r] * kappa[r]);
            let z = (x[t] - mu[r]) / scale2.sqrt();
            let lp = ln_gamma((df + 1.0) / 2.0)
                - ln_gamma(df / 2.0)
                - 0.5 * (df * std::f64::consts::PI * scale2).det_ln()
                - (df + 1.0) / 2.0 * (1.0 + z * z / df).det_ln();
            pred.push(lp.det_exp());
        }
        let growth: Vec<f64> = (0..mu.len())
            .map(|r| probs[r] * pred[r] * (1.0 - hazard))
            .collect();
        let cp: f64 = (0..mu.len()).map(|r| probs[r] * pred[r] * hazard).sum();
        let mut next = vec![cp];
        next.extend(growth);
        let z: f64 = next.iter().sum();
        next.iter_mut().for_each(|p| *p /= z.max(1e-300));
        // update the sufficient statistics: a new run starts from the prior
        let (mut nm, mut nk, mut na, mut nb) = (vec![mu0], vec![1.0], vec![1.0], vec![var0]);
        for r in 0..mu.len() {
            nm.push((kappa[r] * mu[r] + x[t]) / (kappa[r] + 1.0));
            nk.push(kappa[r] + 1.0);
            na.push(alpha[r] + 0.5);
            nb.push(beta[r] + kappa[r] * (x[t] - mu[r]).powi(2) / (2.0 * (kappa[r] + 1.0)));
        }
        if next.len() > 400 {
            next.truncate(400);
            nm.truncate(400);
            nk.truncate(400);
            na.truncate(400);
            nb.truncate(400);
        }
        (mu, kappa, alpha, beta, probs) = (nm, nk, na, nb, next);
        let best = probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(r, _)| r)
            .unwrap_or(0);
        // the most likely run length collapses when a lasting shift begins
        let dropped = prev_best > best + 10 && best < 20;
        prev_best = best;
        if t > 28 && dropped && out.last().map(|&l| t - l > 14).unwrap_or(true) {
            // the shift began a few nights back: confirm against the 14 nights that follow
            let start = t.saturating_sub(3);
            let before: Vec<f64> = x[start.saturating_sub(28)..start]
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect();
            let after: Vec<f64> = x[start..(start + 14).min(n)]
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect();
            if before.len() >= 14
                && after.len() >= 7
                && (mean(&after) - mean(&before)).abs() > 0.5 * stats::std(&before, 1)
            {
                out.push(start);
            }
        }
    }
    out
}

/// An exposure proposal (a change of time zone or altitude), certain rather than inferred from physiology.
pub fn is_exposure(p: &Proposal) -> bool {
    p.model_version == "exposure-1"
}

fn kind_name(k: LabelKind) -> String {
    serde_json::to_string(&k)
        .unwrap()
        .trim_matches('"')
        .to_string()
}

fn exposure(kind: LabelKind, a: i32, b: i32, note: String) -> Proposal {
    let mut post = BTreeMap::new();
    post.insert(kind, 1.0);
    Proposal {
        id: format!("{}:{}", kind_name(kind), a),
        kind,
        start_day: a,
        end_day: b,
        posterior: post,
        evidence: Evidence {
            nights: (b - a + 1) as usize,
            signals: vec![
                if kind == LabelKind::Travel {
                    "travel".into()
                } else {
                    "altitude".into()
                },
                note,
            ],
            ..Default::default()
        },
        score: 1.0,
        priority: 0.0,
        status: ProposalStatus::Open,
        auto: true,
        asked: false,
        created_at: String::new(),
        answered_at: None,
        notified_at: None,
        snoozed_until: None,
        model_version: "exposure-1".into(),
    }
}

/// Decide which proposals to ask about, which to label on its own, and which to leave alone; carry over what the user already answered.
pub fn apply_policy(
    mut props: Vec<Proposal>,
    existing: &[Proposal],
    settings: &Settings,
    today: i32,
) -> Vec<Proposal> {
    let old: BTreeMap<&str, &Proposal> = existing.iter().map(|p| (p.id.as_str(), p)).collect();
    // priorities
    for p in props.iter_mut() {
        let top = p.posterior.values().cloned().fold(0.0, f64::max);
        let w = match p.kind {
            LabelKind::Illness => 2.0,
            LabelKind::Alcohol | LabelKind::Stress => 1.0,
            _ => 0.5,
        };
        let damp = if p.evidence.tz_overlap || p.evidence.alt_overlap {
            0.3
        } else {
            1.0
        };
        p.priority = p.score * (1.0 - top) * w * damp;
        if p.auto {
            continue;
        }
        let illness_p = p.posterior.get(&LabelKind::Illness).copied().unwrap_or(0.0);
        p.asked = (top >= settings.ask_min && top <= settings.ask_max)
            || (p.kind == LabelKind::Illness
                && p.evidence.p_value.map(|v| v < 0.2).unwrap_or(false)
                && illness_p > 0.0);
        if top > settings.ask_max {
            p.auto = true; // labelled on its own, shown as inferred with Undo
        }
    }
    // carry over answers; a denied episode is asked again only if its score rose by half
    for p in props.iter_mut() {
        if let Some(o) = old.get(p.id.as_str()) {
            p.created_at = o.created_at.clone();
            p.notified_at = o.notified_at.clone();
            match o.status {
                ProposalStatus::Confirmed | ProposalStatus::Unsure | ProposalStatus::Snoozed => {
                    p.status = o.status;
                    p.answered_at = o.answered_at.clone();
                    p.snoozed_until = o.snoozed_until.clone();
                    p.asked = false;
                }
                ProposalStatus::Denied => {
                    if p.score <= o.score * 1.5 {
                        p.status = ProposalStatus::Denied;
                        p.answered_at = o.answered_at.clone();
                        p.asked = false;
                    }
                }
                _ => {}
            }
        }
        if p.status == ProposalStatus::Open && p.end_day < today - 60 && p.asked {
            p.status = ProposalStatus::Expired;
            p.asked = false;
        }
    }
    // the weekly budget: the highest priorities first, one extra for a likely illness
    let mut order: Vec<usize> = (0..props.len())
        .filter(|&i| props[i].asked && props[i].status == ProposalStatus::Open)
        .collect();
    order.sort_by(|&a, &b| props[b].priority.partial_cmp(&props[a].priority).unwrap());
    let mut kept: Vec<usize> = vec![];
    for i in order {
        let near = kept
            .iter()
            .filter(|&&k| (props[k].start_day - props[i].start_day).abs() < 7)
            .count();
        let likely_illness = props[i].kind == LabelKind::Illness
            && props[i]
                .posterior
                .get(&LabelKind::Illness)
                .copied()
                .unwrap_or(0.0)
                >= 0.6;
        if near < settings.question_budget_per_week
            || (likely_illness && near < settings.question_budget_per_week + 1)
        {
            kept.push(i);
        } else {
            props[i].asked = false;
        }
    }
    props.sort_by_key(|p| p.start_day);
    props
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::synth;

    fn overlaps(p: &Proposal, t: &Label) -> bool {
        p.start_day <= t.end_day && p.end_day >= t.start_day
    }

    #[test]
    fn a_lasting_shift_is_found() {
        let mut r = stats::Rng::new(4);
        let x: Vec<f64> = (0..200)
            .map(|i| 4.1 + if i >= 120 { 0.3 } else { 0.0 } + r.normal() * 0.1)
            .collect();
        let cps = changepoints(&x);
        assert!(cps.iter().any(|&c| (c as i32 - 120).abs() <= 6), "{cps:?}");
        let flat: Vec<f64> = (0..200).map(|_| 4.1 + r.normal() * 0.1).collect();
        assert!(changepoints(&flat).is_empty());
    }

    #[test]
    fn dst_switches_are_not_trips() {
        assert!(is_dst_switch(days_from_ymd(2026, 3, 8))); // second Sunday of March 2026
        assert!(is_dst_switch(days_from_ymd(2026, 11, 1)));
        assert!(!is_dst_switch(days_from_ymd(2026, 11, 2)));
        assert!(is_dst_switch(days_from_ymd(2026, 3, 29))); // last Sunday of March, EU
    }

    #[test]
    fn finds_the_injected_illnesses_and_trips() {
        let (d, truth) = synth(11, 730, 19_723);
        let s = Settings::default();
        let props = detect(&d, &[], &s, &[]);
        let ill: Vec<&Label> = truth
            .iter()
            .filter(|l| l.kind == LabelKind::Illness)
            .collect();
        let found = ill
            .iter()
            .filter(|t| props.iter().any(|p| !is_exposure(p) && overlaps(p, t)))
            .count();
        assert!(
            found as f64 >= 0.8 * ill.len() as f64,
            "illness recall {found}/{}",
            ill.len()
        );
        // precision: most proposals of the illness kind overlap a true illness or other injected episode
        let illness_props: Vec<&Proposal> = props
            .iter()
            .filter(|p| !is_exposure(p) && p.kind == LabelKind::Illness)
            .collect();
        if !illness_props.is_empty() {
            let good = illness_props
                .iter()
                .filter(|p| truth.iter().any(|t| overlaps(p, t)))
                .count();
            assert!(
                good as f64 >= 0.6 * illness_props.len() as f64,
                "precision {good}/{}",
                illness_props.len()
            );
        }
        let trips: Vec<&Label> = truth
            .iter()
            .filter(|l| l.kind == LabelKind::Travel)
            .collect();
        let tfound = trips
            .iter()
            .filter(|t| {
                props
                    .iter()
                    .any(|p| p.kind == LabelKind::Travel && overlaps(p, t))
            })
            .count();
        assert!(
            tfound as f64 >= 0.8 * trips.len() as f64,
            "travel recall {tfound}/{}",
            trips.len()
        );
        let alts: Vec<&Label> = truth
            .iter()
            .filter(|l| l.kind == LabelKind::Altitude)
            .collect();
        assert!(alts.iter().all(|t| props
            .iter()
            .any(|p| p.kind == LabelKind::Altitude && overlaps(p, t))));
    }

    #[test]
    fn labelled_episodes_are_not_proposed_again_and_denials_stick() {
        let (d, truth) = synth(11, 730, 19_723);
        let s = Settings::default();
        let props = detect(&d, &[], &s, &[]);
        let mut labels = truth.clone();
        labels.retain(|l| l.kind == LabelKind::Illness);
        let again = detect(&d, &labels, &s, &[]);
        assert!(again
            .iter()
            .filter(|p| !is_exposure(p))
            .all(|p| !labels.iter().any(|l| overlaps(p, l))));
        // a denied proposal stays denied
        let first = props
            .iter()
            .find(|p| !is_exposure(p) && p.asked)
            .cloned()
            .expect("something to ask");
        let mut denied = first.clone();
        denied.status = ProposalStatus::Denied;
        let next = detect(&d, &[], &s, &[denied]);
        let same = next.iter().find(|p| p.id == first.id);
        assert!(same
            .map(|p| p.status == ProposalStatus::Denied && !p.asked)
            .unwrap_or(true));
    }

    #[test]
    fn the_budget_limits_questions_per_week() {
        let mut ps: Vec<Proposal> = (0..5)
            .map(|i| {
                let mut p = exposure(LabelKind::Alcohol, 100 + i, 100 + i, String::new());
                p.auto = false;
                p.posterior.insert(LabelKind::Alcohol, 0.6);
                p.posterior.insert(LabelKind::Other, 0.4);
                p.score = 0.9;
                p
            })
            .collect();
        ps[0].id = "alcohol:100".into();
        let out = apply_policy(ps, &[], &Settings::default(), 110);
        assert_eq!(out.iter().filter(|p| p.asked).count(), 2);
    }
}
