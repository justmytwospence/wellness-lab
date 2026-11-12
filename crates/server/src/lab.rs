//! The engine instance behind the server: loading the tenant's data and re-running detection.

use crate::{lock, notify, now, AppState};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;
use wellness_lab_core::data::Data;
use wellness_lab_core::lab::Lab;
use wellness_lab_core::labels::{Proposal, ProposalStatus};

/// Read the wellness-v1 directory and the state database into a fresh `Lab`.
pub fn reload_blocking(s: &AppState) -> Result<()> {
    let (data, manifest) = if s.paths.has_data() {
        (
            wellness_lab_io::read_dir(&s.paths.wellness)?,
            Some(wellness_lab_io::read_manifest(&s.paths.wellness)?),
        )
    } else {
        (Data::default(), None)
    };
    let mut lab = Lab::with_data(data);
    lab.set_labels(s.store.labels()?);
    lab.set_proposals(s.store.proposals()?);
    lab.set_settings(s.store.settings()?);
    *lock(&s.lab) = lab;
    *s.manifest.write().unwrap_or_else(|e| e.into_inner()) = manifest;
    Ok(())
}

pub async fn reload(s: &Arc<AppState>) -> Result<()> {
    let s2 = s.clone();
    tokio::task::spawn_blocking(move || reload_blocking(&s2)).await?
}

/// Push the stored labels and settings into the engine (after an edit).
pub fn push_state(s: &AppState) -> Result<()> {
    let labels = s.store.labels()?;
    let settings = s.store.settings()?;
    let mut lab = lock(&s.lab);
    lab.set_labels(labels);
    lab.set_settings(settings);
    Ok(())
}

/// Run a job on the engine (in the blocking pool) and return its JSON.
pub async fn run_job(s: &Arc<AppState>, job: serde_json::Value) -> Result<String> {
    let s2 = s.clone();
    tokio::task::spawn_blocking(move || {
        lock(&s2.lab)
            .run(&job.to_string())
            .map(|r| r.json)
            .map_err(|e| anyhow!(e))
    })
    .await?
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct DetectReport {
    pub proposals: usize,
    pub open: usize,
    pub asked: usize,
    pub notified: usize,
}

/// Merge a fresh detection with what is stored: the detector already carries answers over by id; answered proposals it
/// no longer returns (a confirmed episode is now a label) are kept, open ones that disappeared are dropped, or expired if
/// the user was already told about them.
pub fn merge(fresh: Vec<Proposal>, stored: &[Proposal], now: &str) -> Vec<Proposal> {
    let ids: BTreeSet<String> = fresh.iter().map(|p| p.id.clone()).collect();
    let mut out = fresh;
    for p in out.iter_mut() {
        if p.created_at.is_empty() {
            p.created_at = stored
                .iter()
                .find(|o| o.id == p.id)
                .map(|o| o.created_at.clone())
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| now.to_string());
        }
    }
    for o in stored.iter().filter(|o| !ids.contains(&o.id)) {
        match o.status {
            ProposalStatus::Open if o.notified_at.is_some() => {
                let mut e = o.clone();
                e.status = ProposalStatus::Expired;
                e.asked = false;
                out.push(e);
            }
            ProposalStatus::Open => {}
            _ => out.push(o.clone()),
        }
    }
    out.sort_by(|a, b| a.start_day.cmp(&b.start_day).then(a.id.cmp(&b.id)));
    out
}

fn detect_blocking(s: &AppState) -> Result<Vec<Proposal>> {
    let now = now();
    let mut stored = s.store.proposals()?;
    // a snooze ends after its week: the proposal is open again
    for p in stored.iter_mut() {
        if p.status == ProposalStatus::Snoozed
            && p.snoozed_until
                .as_deref()
                .map(|u| u <= now.as_str())
                .unwrap_or(true)
        {
            p.status = ProposalStatus::Open;
            p.snoozed_until = None;
            p.answered_at = None;
        }
    }
    let labels = s.store.labels()?;
    let settings = s.store.settings()?;
    let mut lab = lock(&s.lab);
    lab.set_labels(labels);
    lab.set_settings(settings);
    lab.set_proposals(stored.clone());
    let r = lab
        .run(r#"{"kind":"detect"}"#)
        .map_err(|e| anyhow!("detect: {e}"))?;
    #[derive(Deserialize)]
    struct Out {
        proposals: Vec<Proposal>,
    }
    let out: Out = serde_json::from_str(&r.json)?;
    let merged = merge(out.proposals, &stored, &now);
    s.store.replace_proposals(&merged)?;
    lab.set_proposals(merged.clone());
    Ok(merged)
}

/// Run detection, store the merged proposals, then notify about new questions.
pub async fn redetect(s: &Arc<AppState>) -> Result<DetectReport> {
    let _g = s.detect_lock.lock().await;
    let s2 = s.clone();
    let merged = tokio::task::spawn_blocking(move || detect_blocking(&s2)).await??;
    let sent = notify::send_new(s).await;
    Ok(DetectReport {
        proposals: merged.len(),
        open: merged
            .iter()
            .filter(|p| p.status == ProposalStatus::Open)
            .count(),
        asked: merged
            .iter()
            .filter(|p| p.status == ProposalStatus::Open && p.asked)
            .count(),
        notified: sent.len(),
    })
}
