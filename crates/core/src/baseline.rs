//! Personal baselines: a short-window level against a normal range built from the nights before it.

use crate::stats;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transform {
    None,
    Ln,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Center {
    Mean,
    Median,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// sample standard deviation
    Sd,
    /// median absolute deviation, scaled to a standard deviation
    Mad,
}

#[derive(Debug, Clone)]
pub struct BaselineSpec {
    pub transform: Transform,
    pub center: Center,
    pub scale: Scale,
    /// nights before tonight the normal range is built from
    pub window: usize,
    pub min_n: usize,
    /// nights of the short-window level
    pub short: usize,
    pub short_min: usize,
    /// band half-widths in scale units: for a night, and for the short level
    pub k_night: f64,
    pub k_short: f64,
    pub exclude_labelled: bool,
}

/// The HRV4Training-style settings of the deep-dive notebook: ln, mean and SD of the previous `range_days` nights,
/// a 7-night level needing 4.
pub fn notebook_spec(range_days: usize, k_daily: f64, k_swc: f64) -> BaselineSpec {
    BaselineSpec {
        transform: Transform::Ln,
        center: Center::Mean,
        scale: Scale::Sd,
        window: range_days,
        min_n: 14.max(range_days / 3),
        short: 7,
        short_min: 4,
        k_night: k_daily,
        k_short: k_swc,
        exclude_labelled: false,
    }
}

#[derive(Debug, Clone, Default)]
pub struct BaselineOut {
    /// the values after the transform
    pub x: Vec<f64>,
    pub short: Vec<f64>,
    pub center: Vec<f64>,
    pub scale: Vec<f64>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub slo: Vec<f64>,
    pub shi: Vec<f64>,
    pub z: Vec<f64>,
    /// 0 normal, 1 below the night band, 2 above it (255 when there is no band yet)
    pub state: Vec<u8>,
    /// 0 normal, 1 short level below its band, 2 above it (255 when there is no band yet)
    pub short_state: Vec<u8>,
}

pub fn baseline(
    values: &[f64],
    days: &[i32],
    excluded: &BTreeSet<i32>,
    spec: &BaselineSpec,
) -> BaselineOut {
    let n = values.len();
    let x: Vec<f64> = match spec.transform {
        Transform::None => values.to_vec(),
        Transform::Ln => values.iter().map(|v| v.ln()).collect(),
    };
    let mut out = BaselineOut {
        x: x.clone(),
        ..Default::default()
    };
    let nan = || vec![f64::NAN; n];
    out.short = nan();
    out.center = nan();
    out.scale = nan();
    out.lo = nan();
    out.hi = nan();
    out.slo = nan();
    out.shi = nan();
    out.z = nan();
    out.state = vec![255; n];
    out.short_state = vec![255; n];
    let cen = |v: &[f64]| match spec.center {
        Center::Mean => stats::mean(v),
        Center::Median => stats::median(v),
    };
    let mut buf = Vec::new();
    for i in 0..n {
        buf.clear();
        buf.extend(
            x[(i + 1).saturating_sub(spec.short)..=i]
                .iter()
                .copied()
                .filter(|v| !v.is_nan()),
        );
        if buf.len() >= spec.short_min {
            out.short[i] = cen(&buf);
        }
        buf.clear();
        for j in i.saturating_sub(spec.window)..i {
            if x[j].is_nan() || (spec.exclude_labelled && excluded.contains(&days[j])) {
                continue;
            }
            buf.push(x[j]);
        }
        if buf.len() >= spec.min_n {
            let c = cen(&buf);
            let s = match spec.scale {
                Scale::Sd => stats::std(&buf, 1),
                Scale::Mad => stats::mad(&buf),
            };
            out.center[i] = c;
            out.scale[i] = s;
            out.lo[i] = c - spec.k_night * s;
            out.hi[i] = c + spec.k_night * s;
            out.slo[i] = c - spec.k_short * s;
            out.shi[i] = c + spec.k_short * s;
            if x[i].is_finite() {
                out.z[i] = (x[i] - c) / s;
                out.state[i] = if x[i] < out.lo[i] {
                    1
                } else if x[i] > out.hi[i] {
                    2
                } else {
                    0
                };
            }
            if out.short[i].is_finite() {
                out.short_state[i] = if out.short[i] < out.slo[i] {
                    1
                } else if out.short[i] > out.shi[i] {
                    2
                } else {
                    0
                };
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_sd_band_from_previous_nights() {
        // 20 nights alternating 10 and 14 (ln taken by the spec), then a low night
        let mut v: Vec<f64> = (0..20)
            .map(|i| if i % 2 == 0 { 10.0 } else { 14.0 })
            .collect();
        v.push(5.0);
        let days: Vec<i32> = (0..21).collect();
        let b = baseline(&v, &days, &BTreeSet::new(), &notebook_spec(60, 1.5, 0.5));
        assert!(b.center[10].is_nan()); // fewer than max(14, 20) = 20 earlier nights
        assert!(b.center[19].is_nan());
        assert!(b.center[20].is_finite());
        assert_eq!(b.state[20], 1); // far below the band
        assert!((b.x[0] - 10f64.ln()).abs() < 1e-12);
        // the 7-night level needs 4 values
        assert!(b.short[2].is_nan() && b.short[3].is_finite());
    }

    #[test]
    fn labelled_nights_can_be_left_out() {
        let v: Vec<f64> = (0..30)
            .map(|i| if (10..15).contains(&i) { 100.0 } else { 50.0 })
            .collect();
        let days: Vec<i32> = (0..30).collect();
        let ex: BTreeSet<i32> = (10..15).collect();
        let spec = BaselineSpec {
            transform: Transform::None,
            center: Center::Median,
            scale: Scale::Mad,
            window: 28,
            min_n: 14,
            short: 3,
            short_min: 2,
            k_night: 2.0,
            k_short: 1.0,
            exclude_labelled: true,
        };
        let with = baseline(&v, &days, &ex, &spec);
        let without = baseline(
            &v,
            &days,
            &BTreeSet::new(),
            &BaselineSpec {
                exclude_labelled: false,
                ..spec.clone()
            },
        );
        assert_eq!(with.center[29], 50.0);
        assert_eq!(without.center[29], 50.0); // the median shrugs off five outliers either way
        assert!(with.scale[29] == 0.0 && without.scale[29] == 0.0);
    }
}
