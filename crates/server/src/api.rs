//! The HTTP surface: authentication, the REST API, the wellness-v1 files, /mcp and the app.

use crate::state::Experiment;
use crate::{days_from_now, files, lab, mcp, notify, now, AppState, VERSION};
use axum::body::Bytes;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post, put};
use axum::{Json, Router};
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use wellness_lab_core::experiments::ExperimentDef;
use wellness_lab_core::labels::{Label, LabelKind, ProposalStatus, Settings, Source};

/// vercel.json's CSP with `connect-src 'self'`: the server-mode app talks only to this server.
pub const CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'; object-src 'none'";
pub const COOKIE: &str = "wl_session";
const COOKIE_MAX_AGE: u64 = 7_776_000;

// ---------------------------------------------------------------- errors

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %format!("{e:#}"), "request failed");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn missing(what: &str) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, format!("no {what}"))
}

fn parse<T: DeserializeOwned>(body: &Bytes) -> ApiResult<T> {
    serde_json::from_slice(body).map_err(|e| bad(format!("bad request body: {e}")))
}

// ---------------------------------------------------------------- auth

/// `hex(HMAC-SHA256(LAB_TOKEN, "wl-session-v1"))`
pub fn session_value(token: &str) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("hmac takes any key");
    m.update(b"wl-session-v1");
    hex::encode(m.finalize().into_bytes())
}

/// Constant-time equality (of the two strings' hashes, so lengths leak nothing either).
pub fn ct_eq(a: &str, b: &str) -> bool {
    let (x, y) = (Sha256::digest(a.as_bytes()), Sha256::digest(b.as_bytes()));
    x.iter()
        .zip(y.iter())
        .fold(0u8, |acc, (p, q)| acc | (p ^ q))
        == 0
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.trim().to_string())
}

/// Whether a request carries the bearer token or a valid session cookie (always, when no LAB_TOKEN is set).
pub fn authed(s: &AppState, headers: &HeaderMap) -> bool {
    let Some(token) = s.cfg.token.as_deref() else {
        return true;
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(|t| ct_eq(t.trim(), token))
        .unwrap_or(false);
    bearer
        || cookie(headers, COOKIE)
            .map(|c| ct_eq(&c, &session_value(token)))
            .unwrap_or(false)
}

async fn auth_mw(State(s): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    if authed(&s, req.headers()) {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response()
    }
}

fn secure(s: &AppState, headers: &HeaderMap) -> bool {
    s.cfg.secure_cookies()
        || headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("https"))
            .unwrap_or(false)
}

// ---------------------------------------------------------------- public routes

async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "version": VERSION}))
}

async fn info(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Json<Value> {
    let st = s.sync_status();
    let m = s.manifest();
    let source = match (&s.cfg.garmin_mcp_url, &m) {
        (Some(u), _) => json!({"kind": "garmin-mcp", "url": u}),
        (None, Some(_)) => json!({"kind": "import"}),
        (None, None) => json!({"kind": "none"}),
    };
    Json(json!({
        "mode": "server",
        "version": VERSION,
        "tenant": s.cfg.tenant,
        "auth": s.cfg.token.is_some(),
        "authed": authed(&s, &headers),
        "source": source,
        "last_sync": st.last_sync,
        "last_error": st.last_error,
        "files": m.map(|m| m.files.len()).unwrap_or(0),
        "mcp": s.cfg.mcp_enabled(),
    }))
}

#[derive(Deserialize)]
struct LoginIn {
    token: String,
}

async fn login(State(s): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(token) = s.cfg.token.as_deref() else {
        return Json(json!({"ok": true})).into_response();
    };
    let given: Option<LoginIn> = serde_json::from_slice(&body).ok();
    if !given.map(|g| ct_eq(&g.token, token)).unwrap_or(false) {
        tokio::time::sleep(Duration::from_secs(1)).await;
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }
    let mut c = format!(
        "{COOKIE}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={COOKIE_MAX_AGE}",
        session_value(token)
    );
    if secure(&s, &headers) {
        c.push_str("; Secure");
    }
    ([(header::SET_COOKIE, c)], Json(json!({"ok": true}))).into_response()
}

async fn logout(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let mut c = format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
    if secure(&s, &headers) {
        c.push_str("; Secure");
    }
    ([(header::SET_COOKIE, c)], Json(json!({"ok": true}))).into_response()
}

// ---------------------------------------------------------------- state

async fn get_state(State(s): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    let experiments: Vec<Value> = s.store.experiments()?.into_iter().map(|e| e.def).collect();
    Ok(Json(json!({
        "labels": s.store.labels()?,
        "proposals": s.store.proposals()?,
        "settings": s.store.settings()?,
        "experiments": experiments,
        "adherence": s.store.adherence()?,
    })))
}

async fn episodes(State(s): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.store.live_labels()?)))
}

async fn notifications(State(s): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(s.store.notifications(100)?)))
}

// ---------------------------------------------------------------- labels

#[derive(Deserialize)]
struct LabelIn {
    kind: LabelKind,
    start_day: i32,
    end_day: i32,
    #[serde(default)]
    onset_day: Option<i32>,
    #[serde(default)]
    severity: Option<u8>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    exclude_baseline: Option<bool>,
}

fn check_days(start: i32, end: i32, onset: Option<i32>) -> ApiResult<()> {
    if start > end {
        return Err(bad("start_day must not be after end_day"));
    }
    if let Some(o) = onset {
        if o < start - 14 || o > end {
            return Err(bad(
                "onset_day must fall within 14 days before start_day and end_day",
            ));
        }
    }
    Ok(())
}

fn label_from(i: LabelIn, id: String, source: Source, created_at: String) -> ApiResult<Label> {
    check_days(i.start_day, i.end_day, i.onset_day)?;
    Ok(Label {
        id,
        kind: i.kind,
        start_day: i.start_day,
        end_day: i.end_day,
        onset_day: i.onset_day,
        severity: i.severity,
        note: i.note.unwrap_or_default(),
        exclude_baseline: i.exclude_baseline.unwrap_or(i.kind.default_exclude()),
        source,
        proposal_id: None,
        updated_at: created_at.clone(),
        created_at,
        deleted_at: None,
    })
}

async fn create_label(State(s): State<Arc<AppState>>, body: Bytes) -> ApiResult<Json<Label>> {
    let i: LabelIn = parse(&body)?;
    let l = label_from(i, uuid::Uuid::new_v4().to_string(), Source::User, now())?;
    s.store.put_label(&l)?;
    lab::push_state(&s)?;
    Ok(Json(l))
}

async fn update_label(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Label>> {
    let old = s
        .store
        .label(&id)?
        .filter(|l| l.live())
        .ok_or_else(|| missing("label"))?;
    let i: LabelIn = parse(&body)?;
    let mut l = label_from(i, id, old.source, old.created_at.clone())?;
    l.proposal_id = old.proposal_id;
    l.updated_at = now();
    s.store.put_label(&l)?;
    lab::push_state(&s)?;
    Ok(Json(l))
}

async fn delete_label(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Label>> {
    let mut l = s
        .store
        .label(&id)?
        .filter(|l| l.live())
        .ok_or_else(|| missing("label"))?;
    let t = now();
    l.deleted_at = Some(t.clone());
    l.updated_at = t;
    s.store.put_label(&l)?;
    lab::push_state(&s)?;
    Ok(Json(l))
}

// ---------------------------------------------------------------- proposals

#[derive(Deserialize, Default)]
struct LabelPatch {
    kind: Option<LabelKind>,
    start_day: Option<i32>,
    end_day: Option<i32>,
    onset_day: Option<i32>,
    severity: Option<u8>,
    note: Option<String>,
    exclude_baseline: Option<bool>,
}

#[derive(Deserialize)]
struct AnswerIn {
    answer: String,
    #[serde(default)]
    label: Option<LabelPatch>,
}

async fn answer(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let a: AnswerIn = parse(&body)?;
    let mut p = s.store.proposal(&id)?.ok_or_else(|| missing("proposal"))?;
    let t = now();
    let mut label: Option<Label> = None;
    match a.answer.as_str() {
        "confirm" => {
            let lp = a.label.unwrap_or_default();
            // answering twice (a double click, a repeated key) updates the proposal's label instead of adding another;
            // what the second answer leaves out keeps the first answer's value
            let prev = s
                .store
                .live_labels()?
                .into_iter()
                .find(|l| l.proposal_id.as_deref() == Some(p.id.as_str()));
            let kind = lp.kind.or(prev.as_ref().map(|l| l.kind)).unwrap_or(p.kind);
            let (start, end) = (
                lp.start_day
                    .or(prev.as_ref().map(|l| l.start_day))
                    .unwrap_or(p.start_day),
                lp.end_day
                    .or(prev.as_ref().map(|l| l.end_day))
                    .unwrap_or(p.end_day),
            );
            let onset = lp.onset_day.or(prev.as_ref().and_then(|l| l.onset_day));
            check_days(start, end, onset)?;
            let l = Label {
                id: prev
                    .as_ref()
                    .map(|l| l.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                kind,
                start_day: start,
                end_day: end,
                onset_day: onset,
                severity: lp.severity.or(prev.as_ref().and_then(|l| l.severity)),
                note: lp
                    .note
                    .or(prev.as_ref().map(|l| l.note.clone()))
                    .unwrap_or_default(),
                exclude_baseline: lp
                    .exclude_baseline
                    .or(prev.as_ref().map(|l| l.exclude_baseline))
                    .unwrap_or(kind.default_exclude()),
                source: Source::Confirmed,
                proposal_id: Some(p.id.clone()),
                created_at: prev.map(|l| l.created_at).unwrap_or_else(|| t.clone()),
                updated_at: t.clone(),
                deleted_at: None,
            };
            s.store.put_label(&l)?;
            label = Some(l);
            p.status = ProposalStatus::Confirmed;
            p.snoozed_until = None;
        }
        "deny" => {
            p.status = ProposalStatus::Denied;
            p.snoozed_until = None;
        }
        "unsure" => {
            p.status = ProposalStatus::Unsure;
            p.snoozed_until = None;
        }
        "snooze" => {
            p.status = ProposalStatus::Snoozed;
            p.snoozed_until = Some(days_from_now(7));
        }
        other => {
            return Err(bad(format!(
                "answer must be confirm, deny, unsure or snooze, got {other:?}"
            )))
        }
    }
    p.answered_at = Some(t);
    p.asked = false;
    s.store.put_proposal(&p)?;
    lab::push_state(&s)?;
    // the answer teaches the classifier: re-run detection (its failure does not undo the answer)
    if let Err(e) = lab::redetect(&s).await {
        tracing::warn!(error = %format!("{e:#}"), "detection after an answer failed");
    }
    let p = s.store.proposal(&id)?.unwrap_or(p);
    notify::tell_answer(&s, &p, label.as_ref());
    Ok(Json(json!({"proposal": p, "label": label})))
}

#[derive(Deserialize, Default)]
struct UndoIn {
    #[serde(default)]
    label_id: Option<String>,
}

/// Take back an answer: the proposal reopens and the label it created (if any) is deleted.
async fn undo(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let u: UndoIn = if body.is_empty() {
        UndoIn::default()
    } else {
        parse(&body)?
    };
    let mut p = s.store.proposal(&id)?.ok_or_else(|| missing("proposal"))?;
    p.status = ProposalStatus::Open;
    p.answered_at = None;
    p.snoozed_until = None;
    p.asked = true;
    s.store.put_proposal(&p)?;
    if let Some(lid) = u.label_id {
        if let Some(mut l) = s.store.label(&lid)? {
            if l.proposal_id.as_deref() == Some(id.as_str()) {
                l.deleted_at = Some(now());
                s.store.put_label(&l)?;
            }
        }
    }
    lab::push_state(&s)?;
    notify::tell_answer(&s, &p, None);
    Ok(Json(json!({"proposal": p})))
}

// ---------------------------------------------------------------- settings

async fn put_settings(State(s): State<Arc<AppState>>, body: Bytes) -> ApiResult<Json<Settings>> {
    let v: Settings = parse(&body)?;
    if !(0.0..=1.0).contains(&v.ask_min)
        || !(0.0..=1.0).contains(&v.ask_max)
        || v.ask_min > v.ask_max
    {
        return Err(bad(
            "ask_min and ask_max must satisfy 0 <= ask_min <= ask_max <= 1",
        ));
    }
    s.store.put_settings(&v)?;
    lab::push_state(&s)?;
    Ok(Json(v))
}

// ---------------------------------------------------------------- experiments

fn experiment_def(mut v: Value, id: &str) -> ApiResult<Value> {
    let o = v
        .as_object_mut()
        .ok_or_else(|| bad("an experiment is a JSON object"))?;
    o.insert("id".into(), Value::String(id.to_string()));
    let def: ExperimentDef =
        serde_json::from_value(v).map_err(|e| bad(format!("bad experiment: {e}")))?;
    Ok(serde_json::to_value(def).map_err(anyhow::Error::from)?)
}

async fn create_experiment(State(s): State<Arc<AppState>>, body: Bytes) -> ApiResult<Json<Value>> {
    let v: Value = parse(&body)?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if s.store.experiment(&id)?.is_some() {
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!("experiment {id} exists"),
        ));
    }
    let def = experiment_def(v, &id)?;
    s.store.put_experiment(&Experiment {
        id,
        def: def.clone(),
        created_at: now(),
        deleted_at: None,
    })?;
    Ok(Json(def))
}

async fn update_experiment(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let def = experiment_def(parse(&body)?, &id)?;
    let created_at = s
        .store
        .experiment(&id)?
        .map(|e| e.created_at)
        .unwrap_or_else(now);
    s.store.put_experiment(&Experiment {
        id,
        def: def.clone(),
        created_at,
        deleted_at: None,
    })?;
    Ok(Json(def))
}

async fn delete_experiment(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let mut e = s
        .store
        .experiment(&id)?
        .filter(|e| e.deleted_at.is_none())
        .ok_or_else(|| missing("experiment"))?;
    e.deleted_at = Some(now());
    s.store.put_experiment(&e)?;
    Ok(Json(json!({"ok": true, "id": id})))
}

#[derive(Deserialize)]
struct AdherenceIn {
    adherent: bool,
}

async fn put_adherence(
    State(s): State<Arc<AppState>>,
    Path((id, day)): Path<(String, i32)>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    s.store
        .experiment(&id)?
        .filter(|e| e.deleted_at.is_none())
        .ok_or_else(|| missing("experiment"))?;
    let a: AdherenceIn = parse(&body)?;
    s.store.set_adherence(&id, day, a.adherent)?;
    Ok(Json(
        json!({"experiment_id": id, "day": day, "adherent": a.adherent}),
    ))
}

// ---------------------------------------------------------------- sync and notifications

async fn start_sync(State(s): State<Arc<AppState>>) -> ApiResult<Response> {
    if s.cfg.garmin_mcp_url.is_none() {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "GARMIN_MCP_URL is not set: this lab imports data instead of syncing".into(),
        ));
    }
    s.sync_now.notify_one();
    Ok((StatusCode::ACCEPTED, Json(json!({"status": "started"}))).into_response())
}

async fn notify_test(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(Value::Object(notify::send_test(&s).await))
}

async fn api_not_found() -> ApiError {
    missing("such endpoint")
}

// ---------------------------------------------------------------- router

/// The whole app: the API, the data files, /mcp (when on) and the static app with an index.html fallback.
pub fn router(s: Arc<AppState>) -> Router {
    let auth = middleware::from_fn_with_state(s.clone(), auth_mw);
    let protected_api = Router::new()
        .route("/api/logout", post(logout))
        .route("/api/state", get(get_state))
        .route("/api/episodes", get(episodes))
        .route("/api/notifications", get(notifications))
        .route("/api/labels", post(create_label))
        .route("/api/labels/{id}", put(update_label).delete(delete_label))
        .route("/api/proposals/{id}/answer", post(answer))
        .route("/api/proposals/{id}/undo", post(undo))
        .route("/api/settings", put(put_settings))
        .route("/api/experiments", post(create_experiment))
        .route(
            "/api/experiments/{id}",
            put(update_experiment).delete(delete_experiment),
        )
        .route("/api/experiments/{id}/adherence/{day}", put(put_adherence))
        .route("/api/sync", post(start_sync))
        .route("/api/notify/test", post(notify_test))
        .route("/api/{*rest}", any(api_not_found))
        .route_layer(auth.clone());
    let public_api = Router::new()
        .route("/health", get(health))
        .route("/healthz", get(health))
        .route("/api/info", get(info))
        .route("/api/login", post(login));
    let api = public_api
        .merge(protected_api)
        .layer(CompressionLayer::new());
    let data = Router::new()
        .route("/data/wellness-v1/{*path}", get(files::wellness_file))
        .route_layer(auth.clone());
    let mut app = api.merge(data);
    if s.cfg.mcp_enabled() {
        app = app.merge(
            Router::new()
                .nest_service("/mcp", mcp::service(s.clone()))
                .route_layer(auth),
        );
    }
    let dir = s.cfg.static_dir.clone();
    let assets = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .layer(CompressionLayer::new())
        .service(ServeDir::new(dir.join("assets")));
    let spa = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .layer(CompressionLayer::new())
        .service(ServeDir::new(&dir).fallback(ServeFile::new(dir.join("index.html"))));
    app.nest_service("/assets", assets)
        .fallback_service(spa)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(s)
}
