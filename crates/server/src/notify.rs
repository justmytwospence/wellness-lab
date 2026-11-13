//! Questions to the user through ntfy and/or a generic webhook, within the weekly question budget.

use crate::{days_from_now, lock, now, AppState};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use wellness_lab_core::labels::{Label, LabelKind, Proposal, ProposalStatus};
use wellness_lab_core::time::ymd;

const TIMEOUT: Duration = Duration::from_secs(30);
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn kind_text(k: LabelKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.replace('_', " ")))
        .unwrap_or_default()
}

/// "Nov 3"
pub fn mon_d(day: i32) -> String {
    let (_, m, d) = ymd(day);
    format!("{} {d}", MONTHS[(m as usize).clamp(1, 12) - 1])
}

fn finite(x: Option<f64>) -> Option<f64> {
    x.filter(|v| v.is_finite())
}

/// The evidence clause: resting HR, HRV, breathing, skin temperature and sleep stress, whichever are known.
pub fn evidence_text(p: &Proposal) -> String {
    let e = &p.evidence;
    let mut parts: Vec<String> = vec![];
    if let Some(d) = finite(e.d_rhr) {
        parts.push(format!("resting HR {:+} bpm", d.round() as i64));
    }
    if let Some(d) = finite(e.d_lnrmssd) {
        parts.push(format!(
            "HRV {:+}%",
            (100.0 * (d.exp() - 1.0)).round() as i64
        ));
    }
    if let Some(d) = finite(e.d_resp) {
        parts.push(format!("breathing {d:+.1}/min"));
    }
    if let Some(d) = finite(e.d_temp) {
        parts.push(format!("skin temperature {d:+.1} °C"));
    }
    if let Some(d) = finite(e.d_stress) {
        parts.push(format!("sleep stress {:+}", d.round() as i64));
    }
    parts.join(", ")
}

/// "Possible illness, 3 nights from Nov 3: resting HR +5 bpm, HRV -18%. Confirm or deny: https://lab.example/inbox"
pub fn text(p: &Proposal, inbox_url: Option<&str>) -> String {
    let n = if p.evidence.nights > 0 {
        p.evidence.nights as i64
    } else {
        (p.end_day - p.start_day + 1).max(1) as i64
    };
    let nights = if n == 1 { "night" } else { "nights" };
    let mut s = format!(
        "Possible {}, {n} {nights} from {}",
        kind_text(p.kind),
        mon_d(p.start_day)
    );
    let ev = evidence_text(p);
    if !ev.is_empty() {
        s.push_str(": ");
        s.push_str(&ev);
    }
    s.push('.');
    if let Some(u) = inbox_url {
        s.push_str(&format!(" Confirm or deny: {u}"));
    }
    s
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ChannelResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

async fn check(r: reqwest::Result<reqwest::Response>) -> Result<(), String> {
    let r = r.map_err(|e| e.to_string())?;
    if r.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", r.status()))
    }
}

pub async fn send_ntfy(s: &AppState, body: &str, priority: u8) -> Result<(), String> {
    let url = s.cfg.ntfy_url.as_deref().ok_or("NTFY_URL is not set")?;
    let mut rq = s
        .http
        .post(url)
        .timeout(TIMEOUT)
        .header("Title", "Wellness Lab")
        .header("Priority", priority.to_string())
        .body(body.to_string());
    if let Some(u) = s.cfg.inbox_url() {
        rq = rq.header("Click", u);
    }
    if let Some(t) = &s.cfg.ntfy_token {
        rq = rq.bearer_auth(t);
    }
    check(rq.send().await).await
}

pub async fn send_webhook(s: &AppState, payload: &Value) -> Result<(), String> {
    let url = s
        .cfg
        .webhook_url
        .as_deref()
        .ok_or("NOTIFY_WEBHOOK_URL is not set")?;
    let mut rq = s.http.post(url).timeout(TIMEOUT).json(payload);
    if let Some(t) = &s.cfg.webhook_token {
        rq = rq.bearer_auth(t);
    }
    check(rq.send().await).await
}

/// Tell the webhook that a question it was sent has been answered, reopened, or retired by detection (a newer
/// question replaced it), so whatever showed it (a chat message, say) can say so. Questions that were never sent are not mentioned; failures are logged, never retried.
pub fn tell_answer(s: &Arc<AppState>, p: &Proposal, label: Option<&Label>) {
    if s.cfg.webhook_url.is_none() || p.notified_at.is_none() {
        return;
    }
    match s.store.settings() {
        Ok(st) if st.notify_webhook => {}
        _ => return,
    }
    let payload = json!({
        "type": match p.status {
            ProposalStatus::Open => "reopened",
            ProposalStatus::Expired => "expired",
            _ => "answered",
        },
        "proposal": p,
        "label": label,
        "url": s.cfg.inbox_url(),
    });
    let (s, id) = (s.clone(), p.id.clone());
    tokio::spawn(async move {
        let r = send_webhook(&s, &payload).await;
        log(&s, Some(&id), "webhook", &r, &now());
    });
}

fn likely_illness(p: &Proposal) -> bool {
    p.kind == LabelKind::Illness
        && p.posterior.get(&LabelKind::Illness).copied().unwrap_or(0.0) >= 0.6
}

fn log(s: &AppState, proposal: Option<&str>, channel: &str, r: &Result<(), String>, at: &str) {
    if let Err(e) = &r {
        tracing::warn!(channel, proposal, error = %e, "notification failed");
    }
    if let Err(e) = s.store.log_notification(
        proposal,
        channel,
        r.is_ok(),
        r.as_ref().err().map(|e| e.as_str()),
        at,
    ) {
        tracing::warn!(error = %e, "could not log a notification");
    }
}

/// Send the questions the policy marked as asked that nobody was told about yet, at most `question_budget_per_week`
/// in any 7 days (one more for a likely illness). Returns the ids notified; failures are retried at the next detection.
pub async fn send_new(s: &Arc<AppState>) -> Vec<String> {
    let settings = match s.store.settings() {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!(error = %e, "notify: settings");
            return vec![];
        }
    };
    let ntfy = s.cfg.ntfy_url.is_some() && settings.notify_ntfy;
    let hook = s.cfg.webhook_url.is_some() && settings.notify_webhook;
    if !ntfy && !hook {
        return vec![];
    }
    let props = match s.store.proposals() {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!(error = %e, "notify: proposals");
            return vec![];
        }
    };
    let week_ago = days_from_now(-7);
    let mut recent = props
        .iter()
        .filter(|p| {
            p.notified_at
                .as_deref()
                .map(|t| t >= week_ago.as_str())
                .unwrap_or(false)
        })
        .count();
    let mut todo: Vec<Proposal> = props
        .into_iter()
        .filter(|p| {
            p.status == ProposalStatus::Open && !p.auto && p.asked && p.notified_at.is_none()
        })
        .collect();
    todo.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let inbox = s.cfg.inbox_url();
    let mut sent = vec![];
    for mut p in todo {
        let limit = settings.question_budget_per_week + likely_illness(&p) as usize;
        if recent >= limit {
            continue;
        }
        let body = text(&p, inbox.as_deref());
        let at = now();
        let mut any = false;
        if ntfy {
            let prio = if p.kind == LabelKind::Illness { 4 } else { 3 };
            let r = send_ntfy(s, &body, prio).await;
            any |= r.is_ok();
            log(s, Some(&p.id), "ntfy", &r, &at);
        }
        if hook {
            let payload = json!({"type": "proposal", "proposal": p, "text": body, "url": inbox});
            let r = send_webhook(s, &payload).await;
            any |= r.is_ok();
            log(s, Some(&p.id), "webhook", &r, &at);
        }
        if any {
            p.notified_at = Some(at);
            if let Err(e) = s.store.put_proposal(&p) {
                tracing::warn!(error = %e, "notify: store");
                continue;
            }
            recent += 1;
            sent.push(p.id.clone());
        }
    }
    if !sent.is_empty() {
        if let Ok(ps) = s.store.proposals() {
            lock(&s.lab).set_proposals(ps);
        }
        tracing::info!(n = sent.len(), "notified");
    }
    sent
}

/// "Wellness Lab test notification" through every configured channel.
pub async fn send_test(s: &Arc<AppState>) -> serde_json::Map<String, Value> {
    const TEXT: &str = "Wellness Lab test notification";
    let mut out = serde_json::Map::new();
    let at = now();
    if s.cfg.ntfy_url.is_some() {
        let r = send_ntfy(s, TEXT, 3).await;
        log(s, None, "ntfy", &r, &at);
        out.insert("ntfy".into(), json!(result(r)));
    }
    if s.cfg.webhook_url.is_some() {
        let r = send_webhook(
            s,
            &json!({"type": "test", "text": TEXT, "url": s.cfg.inbox_url()}),
        )
        .await;
        log(s, None, "webhook", &r, &at);
        out.insert("webhook".into(), json!(result(r)));
    }
    out
}

fn result(r: Result<(), String>) -> ChannelResult {
    ChannelResult {
        ok: r.is_ok(),
        error: r.err(),
    }
}
