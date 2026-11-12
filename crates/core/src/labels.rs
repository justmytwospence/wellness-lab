//! Labels (the user's record of what happened), proposals (what the app suspects) and settings.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LabelKind {
    Illness,
    Vaccination,
    Alcohol,
    Travel,
    Altitude,
    HardTraining,
    Stress,
    LateMeal,
    CaffeineLate,
    Sauna,
    ColdExposure,
    MedicationChange,
    Menstruation,
    PoorEnvironment,
    Other,
}

impl LabelKind {
    /// Whether nights of this kind are left out of the baselines unless the user says otherwise.
    pub fn default_exclude(self) -> bool {
        matches!(
            self,
            LabelKind::Illness
                | LabelKind::Vaccination
                | LabelKind::Travel
                | LabelKind::Altitude
                | LabelKind::MedicationChange
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    User,
    Confirmed,
    Inferred,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Label {
    pub id: String,
    pub kind: LabelKind,
    pub start_day: i32,
    pub end_day: i32,
    #[serde(default)]
    pub onset_day: Option<i32>,
    #[serde(default)]
    pub severity: Option<u8>,
    #[serde(default)]
    pub note: String,
    pub exclude_baseline: bool,
    pub source: Source,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub deleted_at: Option<String>,
}

impl Label {
    pub fn live(&self) -> bool {
        self.deleted_at.is_none()
    }
    pub fn onset(&self) -> i32 {
        self.onset_day.unwrap_or(self.start_day)
    }
}

/// The days covered by non-deleted labels that are to be left out of baselines.
pub fn excluded_days(labels: &[Label]) -> BTreeSet<i32> {
    let mut s = BTreeSet::new();
    for l in labels.iter().filter(|l| l.live() && l.exclude_baseline) {
        s.extend(l.start_day..=l.end_day);
    }
    s
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Open,
    Confirmed,
    Denied,
    Unsure,
    Snoozed,
    Expired,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Evidence {
    pub nights: usize,
    pub d_rhr: Option<f64>,
    pub d_lnrmssd: Option<f64>,
    pub d_resp: Option<f64>,
    pub d_temp: Option<f64>,
    pub d_stress: Option<f64>,
    pub d_early_late: Option<f64>,
    pub p_value: Option<f64>,
    pub hard_day_before: bool,
    pub weekend: bool,
    pub tz_overlap: bool,
    pub alt_overlap: bool,
    /// what raised it: "fusion", "cusum", "night_signal", "travel", "altitude", "changepoint", "cycle"
    pub signals: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Proposal {
    /// "<kind>:<start_day>"
    pub id: String,
    pub kind: LabelKind,
    pub start_day: i32,
    pub end_day: i32,
    pub posterior: BTreeMap<LabelKind, f64>,
    pub evidence: Evidence,
    pub score: f64,
    pub priority: f64,
    pub status: ProposalStatus,
    pub auto: bool,
    /// whether the question policy wants to ask the user now
    #[serde(default)]
    pub asked: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub answered_at: Option<String>,
    #[serde(default)]
    pub notified_at: Option<String>,
    #[serde(default)]
    pub snoozed_until: Option<String>,
    pub model_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub question_budget_per_week: usize,
    pub ask_min: f64,
    pub ask_max: f64,
    pub auto_label_travel: bool,
    pub auto_label_altitude: bool,
    pub cycle_tracking: bool,
    /// wake weekdays that are free days, Monday = 0
    pub free_days: Vec<u8>,
    pub sleep_need_h: Option<f64>,
    pub tst_bias_min: f64,
    pub baseline_window: usize,
    pub hrv_range_days: usize,
    pub k_daily: f64,
    pub k_swc: f64,
    pub home_altitude_m: Option<f64>,
    pub notify_ntfy: bool,
    pub notify_webhook: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            question_budget_per_week: 2,
            ask_min: 0.30,
            ask_max: 0.95,
            auto_label_travel: true,
            auto_label_altitude: true,
            cycle_tracking: false,
            free_days: vec![5, 6],
            sleep_need_h: None,
            tst_bias_min: 30.0,
            baseline_window: 28,
            hrv_range_days: 60,
            k_daily: 1.5,
            k_swc: 0.5,
            home_altitude_m: None,
            notify_ntfy: true,
            notify_webhook: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_default_exclusions() {
        let k: LabelKind = serde_json::from_str("\"hard_training\"").unwrap();
        assert_eq!(k, LabelKind::HardTraining);
        assert!(LabelKind::Illness.default_exclude() && !LabelKind::Alcohol.default_exclude());
        let s: Settings = serde_json::from_str("{\"question_budget_per_week\": 5}").unwrap();
        assert_eq!(s.question_budget_per_week, 5);
        assert_eq!(s.baseline_window, 28);
    }

    #[test]
    fn excluded_days_skip_deleted() {
        let mk = |a, b, ex, del: bool| Label {
            id: "x".into(),
            kind: LabelKind::Illness,
            start_day: a,
            end_day: b,
            onset_day: None,
            severity: None,
            note: String::new(),
            exclude_baseline: ex,
            source: Source::User,
            proposal_id: None,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: del.then(|| "t".to_string()),
        };
        let d = excluded_days(&[
            mk(10, 12, true, false),
            mk(20, 21, true, true),
            mk(30, 31, false, false),
        ]);
        assert_eq!(d.into_iter().collect::<Vec<_>>(), vec![10, 11, 12]);
    }
}
