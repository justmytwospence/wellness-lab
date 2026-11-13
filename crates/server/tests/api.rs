//! The server end to end through `tower::ServiceExt::oneshot`, with stub garmin-mcp, ntfy and webhook servers.

use axum::body::Body;
use axum::extract::{Path as AxPath, State};
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use wellness_lab_core::data::Data;
use wellness_lab_core::labels::{Proposal, ProposalStatus, Source};
use wellness_lab_server::config::Config;
use wellness_lab_server::{api, lab, notify, sync, AppState};

const TOKEN: &str = "dev";
const START: i32 = 20_000;

fn cfg(dir: &Path) -> Config {
    Config {
        data_dir: dir.join("data"),
        static_dir: dir.join("static"),
        token: Some(TOKEN.into()),
        ..Default::default()
    }
}

struct Resp {
    status: StatusCode,
    headers: HeaderMap,
    bytes: Vec<u8>,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).unwrap_or(Value::Null)
    }
}

async fn send(app: &Router, req: Request<Body>) -> Resp {
    let r = app.clone().oneshot(req).await.unwrap();
    let status = r.status();
    let headers = r.headers().clone();
    let bytes = r.into_body().collect().await.unwrap().to_bytes().to_vec();
    Resp {
        status,
        headers,
        bytes,
    }
}

fn req(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    if body.is_some() {
        b = b.header(header::CONTENT_TYPE, "application/json");
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_default())
        .unwrap()
}

async fn call(app: &Router, method: Method, uri: &str, body: Option<Value>) -> Resp {
    send(app, req(method, uri, body)).await
}

fn synth_data(seed: u64, days: i32) -> Data {
    wellness_lab_core::synth::synth(seed, days, START).0
}

fn write(dir: &Path, data: &Data) -> wellness_lab_io::Manifest {
    wellness_lab_io::write_data(dir, data, "test", "synth").unwrap()
}

async fn serve(router: Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(l, router).await.unwrap();
    });
    format!("http://{addr}")
}

// ---------------------------------------------------------------- health, auth

#[tokio::test]
async fn health_and_info_are_public() {
    let d = tempfile::tempdir().unwrap();
    let s = AppState::open(cfg(d.path())).unwrap();
    let app = api::router(s);
    for p in ["/health", "/healthz"] {
        let r = send(&app, Request::get(p).body(Body::empty()).unwrap()).await;
        assert_eq!(r.status, StatusCode::OK);
        assert_eq!(r.json()["status"], "ok");
    }
    let r = send(&app, Request::get("/api/info").body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::OK);
    let i = r.json();
    assert_eq!(i["mode"], "server");
    assert_eq!(i["auth"], true);
    assert_eq!(i["authed"], false);
    assert_eq!(i["source"]["kind"], "none");
    assert_eq!(i["mcp"], true);
    assert!(r.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .contains("connect-src 'self';"));
}

#[tokio::test]
async fn auth_by_bearer_and_session_cookie() {
    let d = tempfile::tempdir().unwrap();
    let s = AppState::open(cfg(d.path())).unwrap();
    let app = api::router(s);
    let anon = send(
        &app,
        Request::get("/api/state").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(anon.status, StatusCode::UNAUTHORIZED);
    assert_eq!(anon.json(), json!({"error": "unauthorized"}));
    let wrong = send(
        &app,
        Request::get("/api/state")
            .header(header::AUTHORIZATION, "Bearer nope")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        call(&app, Method::GET, "/api/state", None).await.status,
        StatusCode::OK
    );
    for p in ["/data/wellness-v1/manifest.json", "/mcp"] {
        let r = send(&app, Request::get(p).body(Body::empty()).unwrap()).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{p}");
    }

    let bad = send(
        &app,
        Request::post("/api/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"token": "nope"}).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    let ok = send(
        &app,
        Request::post("/api/login")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-proto", "https")
            .body(Body::from(json!({"token": TOKEN}).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    let set = ok.headers[header::SET_COOKIE].to_str().unwrap().to_string();
    let value = api::session_value(TOKEN);
    assert!(set.starts_with(&format!("wl_session={value};")), "{set}");
    for part in [
        "HttpOnly",
        "SameSite=Strict",
        "Path=/",
        "Max-Age=7776000",
        "Secure",
    ] {
        assert!(set.contains(part), "{set} lacks {part}");
    }
    let with_cookie = |uri: &str| {
        Request::get(uri)
            .header(header::COOKIE, format!("theme=dark; wl_session={value}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(&app, with_cookie("/api/state")).await.status,
        StatusCode::OK
    );
    let info = send(&app, with_cookie("/api/info")).await.json();
    assert_eq!(info["authed"], true);
    let out = send(
        &app,
        Request::post("/api/logout")
            .header(header::COOKIE, format!("wl_session={value}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(out.headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
}

#[tokio::test]
async fn no_token_means_open_access_and_spa_fallback() {
    let d = tempfile::tempdir().unwrap();
    let mut c = cfg(d.path());
    c.token = None;
    std::fs::create_dir_all(c.static_dir.join("assets")).unwrap();
    std::fs::write(c.static_dir.join("index.html"), "<html>lab</html>").unwrap();
    std::fs::write(c.static_dir.join("assets/app.js"), "console.log(1)").unwrap();
    let app = api::router(AppState::open(c).unwrap());
    let r = send(
        &app,
        Request::get("/api/state").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = send(
        &app,
        Request::get("/night/20100").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.bytes, b"<html>lab</html>");
    assert!(r.headers.contains_key(header::CONTENT_SECURITY_POLICY));
    let r = send(
        &app,
        Request::get("/assets/app.js").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.headers[header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .contains("immutable"));
    let r = send(
        &app,
        Request::get("/assets/missing.js")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(&app, Request::get("/api/nope").body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------- state database

#[test]
fn the_database_is_wal_at_version_1() {
    let d = tempfile::tempdir().unwrap();
    let st = wellness_lab_server::Store::open(&d.path().join("s.sqlite")).unwrap();
    assert_eq!(st.user_version().unwrap(), 1);
    assert_eq!(st.journal_mode().unwrap().to_lowercase(), "wal");
    drop(st);
    // reopening keeps the data and the version
    let st = wellness_lab_server::Store::open(&d.path().join("s.sqlite")).unwrap();
    assert_eq!(st.user_version().unwrap(), 1);
}

// ---------------------------------------------------------------- labels

#[tokio::test]
async fn label_crud() {
    let d = tempfile::tempdir().unwrap();
    let s = AppState::open(cfg(d.path())).unwrap();
    let app = api::router(s.clone());
    let r = call(
        &app,
        Method::POST,
        "/api/labels",
        Some(json!({"kind": "illness", "start_day": 20100, "end_day": 20103, "severity": 2, "note": "cold"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let l = r.json();
    let id = l["id"].as_str().unwrap().to_string();
    assert_eq!(l["source"], "user");
    assert_eq!(l["exclude_baseline"], true);
    assert_eq!(l["note"], "cold");
    assert!(l["deleted_at"].is_null());
    assert_eq!(s.lab.lock().unwrap().labels.len(), 1);

    let alcohol = call(
        &app,
        Method::POST,
        "/api/labels",
        Some(json!({"kind": "alcohol", "start_day": 20110, "end_day": 20110})),
    )
    .await
    .json();
    assert_eq!(alcohol["exclude_baseline"], false);

    let r = call(
        &app,
        Method::POST,
        "/api/labels",
        Some(json!({"kind": "illness", "start_day": 20105, "end_day": 20100})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = call(
        &app,
        Method::POST,
        "/api/labels",
        Some(json!({"kind": "afib", "start_day": 1, "end_day": 2})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.json()["error"].as_str().unwrap().contains("afib"));

    let r = call(
        &app,
        Method::PUT,
        &format!("/api/labels/{id}"),
        Some(json!({"kind": "illness", "start_day": 20100, "end_day": 20105, "note": "flu", "exclude_baseline": false})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let u = r.json();
    assert_eq!(u["end_day"], 20105);
    assert_eq!(u["note"], "flu");
    assert_eq!(u["exclude_baseline"], false);
    assert_eq!(u["created_at"], l["created_at"]);
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/api/labels/nope",
            Some(json!({"kind": "illness", "start_day": 1, "end_day": 2}))
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );

    let eps = call(&app, Method::GET, "/api/episodes", None).await.json();
    assert_eq!(eps.as_array().unwrap().len(), 2);
    let r = call(&app, Method::DELETE, &format!("/api/labels/{id}"), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.json()["deleted_at"].is_string());
    assert_eq!(
        call(&app, Method::DELETE, &format!("/api/labels/{id}"), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let eps = call(&app, Method::GET, "/api/episodes", None).await.json();
    assert_eq!(eps.as_array().unwrap().len(), 1);
    assert_eq!(eps[0]["kind"], "alcohol");
    // the soft-deleted label stays in the state (for undo)
    let st = call(&app, Method::GET, "/api/state", None).await.json();
    assert_eq!(st["labels"].as_array().unwrap().len(), 2);
    assert_eq!(st["settings"]["question_budget_per_week"], 2);
}

#[tokio::test]
async fn settings_and_experiments() {
    let d = tempfile::tempdir().unwrap();
    let s = AppState::open(cfg(d.path())).unwrap();
    let app = api::router(s.clone());
    let r = call(
        &app,
        Method::PUT,
        "/api/settings",
        Some(json!({"question_budget_per_week": 3, "ask_min": 0.4})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["question_budget_per_week"], 3);
    assert_eq!(r.json()["ask_max"], 0.95);
    assert_eq!(s.lab.lock().unwrap().settings.question_budget_per_week, 3);
    let r = call(
        &app,
        Method::PUT,
        "/api/settings",
        Some(json!({"ask_min": 0.9, "ask_max": 0.5})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let def = json!({"name": "No screens", "intervention": "other", "outcome": "ln_rmssd", "start_day": 20100});
    let r = call(&app, Method::POST, "/api/experiments", Some(def)).await;
    assert_eq!(r.status, StatusCode::OK);
    let e = r.json();
    let id = e["id"].as_str().unwrap().to_string();
    assert_eq!(e["period_nights"], 7);
    let r = call(
        &app,
        Method::PUT,
        &format!("/api/experiments/{id}/adherence/20101"),
        Some(json!({"adherent": true})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = call(
        &app,
        Method::PUT,
        &format!("/api/experiments/{id}"),
        Some(json!({"name": "No screens after 9", "intervention": "other", "outcome": "ln_rmssd", "start_day": 20100, "blocks": 2})),
    )
    .await;
    assert_eq!(r.json()["blocks"], 2);
    let st = call(&app, Method::GET, "/api/state", None).await.json();
    assert_eq!(st["experiments"][0]["name"], "No screens after 9");
    assert_eq!(st["adherence"][&id]["20101"], true);
    let r = call(
        &app,
        Method::POST,
        "/api/experiments",
        Some(json!({"name": "x"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        call(
            &app,
            Method::DELETE,
            &format!("/api/experiments/{id}"),
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    let st = call(&app, Method::GET, "/api/state", None).await.json();
    assert!(st["experiments"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------- proposals

#[tokio::test]
async fn answering_proposals() {
    let d = tempfile::tempdir().unwrap();
    let c = cfg(d.path());
    let paths = wellness_lab_server::TenantPaths::new(&c.data_dir, &c.tenant);
    paths.create().unwrap();
    write(&paths.wellness, &synth_data(11, 400));
    let s = AppState::open(c).unwrap();
    lab::redetect(&s).await.unwrap();
    // questions about older episodes have expired by now; answers are accepted all the same
    let mut qs: Vec<Proposal> = s
        .store
        .proposals()
        .unwrap()
        .into_iter()
        .filter(|p| !p.auto)
        .collect();
    qs.sort_by_key(|p| std::cmp::Reverse(p.start_day));
    assert!(
        qs.len() >= 3,
        "synthetic data should raise questions, got {}",
        qs.len()
    );
    let app = api::router(s.clone());

    // confirm: a label from the proposal, status confirmed, kept across detection
    let p = &qs[0];
    let r = call(
        &app,
        Method::POST,
        &format!("/api/proposals/{}/answer", p.id),
        Some(json!({"answer": "confirm", "label": {"note": "felt rough", "severity": 2}})),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&r.bytes)
    );
    let out = r.json();
    assert_eq!(out["proposal"]["status"], "confirmed");
    assert!(out["proposal"]["answered_at"].is_string());
    let l = &out["label"];
    assert_eq!(l["source"], "confirmed");
    assert_eq!(l["proposal_id"], p.id.as_str());
    assert_eq!(l["kind"], json!(p.kind));
    assert_eq!(l["start_day"], p.start_day);
    assert_eq!(l["end_day"], p.end_day);
    // a second confirm (a double click) updates that label rather than adding another
    let again = call(
        &app,
        Method::POST,
        &format!("/api/proposals/{}/answer", p.id),
        Some(json!({"answer": "confirm"})),
    )
    .await
    .json();
    assert_eq!(again["label"]["id"], l["id"]);
    assert_eq!(again["label"]["note"], "felt rough");
    assert_eq!(
        s.store
            .live_labels()
            .unwrap()
            .iter()
            .filter(|x| x.proposal_id.as_deref() == Some(p.id.as_str()))
            .count(),
        1
    );
    assert_eq!(l["note"], "felt rough");
    // detection labels certain exposures on its own; everything else came from answers
    let mine = || {
        s.store
            .live_labels()
            .unwrap()
            .into_iter()
            .filter(|l| l.source != Source::Inferred)
            .count()
    };
    let auto = s.store.live_labels().unwrap().len() - mine();
    assert!(
        auto > 0,
        "the synthetic travel and altitude should be labelled on their own"
    );
    assert_eq!(mine(), 1);
    assert_eq!(s.lab.lock().unwrap().labels.len(), 1 + auto);
    lab::redetect(&s).await.unwrap();
    assert_eq!(
        s.store.proposal(&p.id).unwrap().unwrap().status,
        ProposalStatus::Confirmed
    );

    // deny: stays denied after detection re-runs, and makes no label
    let q = &qs[1];
    let r = call(
        &app,
        Method::POST,
        &format!("/api/proposals/{}/answer", q.id),
        Some(json!({"answer": "deny"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["proposal"]["status"], "denied");
    assert!(r.json()["label"].is_null());
    lab::redetect(&s).await.unwrap();
    assert_eq!(
        s.store.proposal(&q.id).unwrap().unwrap().status,
        ProposalStatus::Denied
    );
    assert_eq!(mine(), 1);

    // snooze: a week out
    let z = &qs[2];
    let r = call(
        &app,
        Method::POST,
        &format!("/api/proposals/{}/answer", z.id),
        Some(json!({"answer": "snooze"})),
    )
    .await;
    assert_eq!(r.json()["proposal"]["status"], "snoozed");
    let until = r.json()["proposal"]["snoozed_until"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(until > wellness_lab_server::days_from_now(6));
    let st = call(&app, Method::GET, "/api/state", None).await.json();
    assert!(st["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == z.id.as_str() && x["status"] == "snoozed"));

    let r = call(
        &app,
        Method::POST,
        &format!("/api/proposals/{}/answer", p.id),
        Some(json!({"answer": "maybe"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = call(
        &app,
        Method::POST,
        "/api/proposals/nope/answer",
        Some(json!({"answer": "deny"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------- data files

#[tokio::test]
async fn data_files_with_etags_and_ranges() {
    let d = tempfile::tempdir().unwrap();
    let c = cfg(d.path());
    let paths = wellness_lab_server::TenantPaths::new(&c.data_dir, &c.tenant);
    paths.create().unwrap();
    let m = write(&paths.wellness, &synth_data(3, 40));
    std::fs::write(paths.root.join("secret.txt"), "x").unwrap();
    let app = api::router(AppState::open(c).unwrap());

    let r = call(&app, Method::GET, "/data/wellness-v1/manifest.json", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-cache");
    let msha = hex::encode(Sha256::digest(&r.bytes));
    assert_eq!(
        r.headers[header::ETAG].to_str().unwrap(),
        format!("\"{msha}\"")
    );

    let days = m.files.iter().find(|f| f.path == "days.parquet").unwrap();
    let r = call(&app, Method::GET, "/data/wellness-v1/days.parquet", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        r.headers[header::ETAG].to_str().unwrap(),
        format!("\"{}\"", days.sha256)
    );
    assert_eq!(hex::encode(Sha256::digest(&r.bytes)), days.sha256);
    assert_eq!(r.headers[header::CONTENT_TYPE], "application/octet-stream");

    let mut rq = req(Method::GET, "/data/wellness-v1/days.parquet", None);
    rq.headers_mut().insert(
        header::IF_NONE_MATCH,
        format!("\"{}\"", days.sha256).parse().unwrap(),
    );
    assert_eq!(send(&app, rq).await.status, StatusCode::NOT_MODIFIED);

    let mut rq = req(Method::GET, "/data/wellness-v1/days.parquet", None);
    rq.headers_mut()
        .insert(header::RANGE, "bytes=0-3".parse().unwrap());
    let r = send(&app, rq).await;
    assert_eq!(r.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(r.bytes, b"PAR1");
    assert!(r.headers.contains_key(header::ETAG));

    let series = m.files.iter().find(|f| f.table == "series").unwrap();
    let r = call(
        &app,
        Method::GET,
        &format!("/data/wellness-v1/{}", series.path),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);

    for bad in [
        "/data/wellness-v1/../secret.txt",
        "/data/wellness-v1/%2e%2e/secret.txt",
        "/data/wellness-v1/series/hr/../../../secret.txt",
        "/data/wellness-v1/manifest.json.tmp",
        "/data/wellness-v1/series/hr/2099-01.parquet",
    ] {
        let r = call(&app, Method::GET, bad, None).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{bad}");
    }
}

// ---------------------------------------------------------------- sync

#[derive(Clone, Default)]
struct Stub {
    dir: Arc<Mutex<PathBuf>>,
    hits: Arc<Mutex<Vec<String>>>,
}

async fn stub_export(
    State(st): State<Stub>,
    AxPath(rel): AxPath<String>,
    headers: HeaderMap,
) -> Response {
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        != Some("Bearer mcp-token")
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    st.hits.lock().unwrap().push(rel.clone());
    let dir = st.dir.lock().unwrap().clone();
    let Ok(bytes) = std::fs::read(dir.join(&rel)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let etag = format!("\"{}\"", hex::encode(Sha256::digest(&bytes)));
    if rel == "manifest.json"
        && headers
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(etag.as_str())
    {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    ([(header::ETAG, etag)], bytes).into_response()
}

fn local_sha(p: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(p).unwrap()))
}

#[tokio::test]
async fn sync_downloads_changes_and_deletes_removed_files() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("remote");
    let mut data = synth_data(5, 70);
    let m1 = write(&remote, &data);
    let stub = Stub {
        dir: Arc::new(Mutex::new(remote.clone())),
        ..Default::default()
    };
    let url = serve(
        Router::new()
            .route("/export/wellness/v1/{*path}", get(stub_export))
            .with_state(stub.clone()),
    )
    .await;

    let mut c = cfg(d.path());
    c.garmin_mcp_url = Some(url.clone());
    c.garmin_mcp_token = Some("mcp-token".into());
    let s = AppState::open(c).unwrap();

    // first sync: everything
    let r = sync::sync_once(&s).await.unwrap();
    assert!(r.changed);
    assert_eq!(r.downloaded, m1.files.len());
    assert_eq!(r.files, m1.files.len());
    for f in &m1.files {
        assert_eq!(
            local_sha(&s.paths.wellness.join(&f.path)),
            f.sha256,
            "{}",
            f.path
        );
    }
    assert_eq!(s.manifest().unwrap().files.len(), m1.files.len());
    assert!(s.lab.lock().unwrap().data.days.n > 0);
    assert!(std::fs::read_dir(&s.paths.tmp).unwrap().next().is_none());

    // unchanged: a 304 for the manifest, no file requests
    stub.hits.lock().unwrap().clear();
    let r = sync::sync_once(&s).await.unwrap();
    assert!(r.not_modified && !r.changed);
    assert_eq!(
        *stub.hits.lock().unwrap(),
        vec!["manifest.json".to_string()]
    );

    // one table changes and one series disappears
    let rhr = data.days.num.get_mut("rhr").unwrap();
    rhr[0] = if rhr[0].is_finite() {
        rhr[0] + 1.0
    } else {
        55.0
    };
    let gone = data.series.keys().next().unwrap().clone();
    data.series.remove(&gone);
    let m2 = write(&remote, &data);
    let gone_files: Vec<String> = m1
        .files
        .iter()
        .filter(|f| f.name.as_deref() == Some(gone.as_str()))
        .map(|f| f.path.clone())
        .collect();
    assert!(!gone_files.is_empty());
    stub.hits.lock().unwrap().clear();
    let r = sync::sync_once(&s).await.unwrap();
    assert!(r.changed);
    assert_eq!(r.downloaded, 1, "only days.parquet changed");
    assert_eq!(r.removed, gone_files.len());
    let hits = stub.hits.lock().unwrap().clone();
    assert_eq!(
        hits,
        vec!["manifest.json".to_string(), "days.parquet".to_string()]
    );
    let days = m2.files.iter().find(|f| f.path == "days.parquet").unwrap();
    assert_eq!(
        local_sha(&s.paths.wellness.join("days.parquet")),
        days.sha256
    );
    for g in &gone_files {
        assert!(!s.paths.wellness.join(g).exists(), "{g} should be deleted");
    }
    let local = wellness_lab_io::read_manifest(&s.paths.wellness).unwrap();
    assert_eq!(local.files.len(), m2.files.len());
    assert!(!s.lab.lock().unwrap().data.series.contains_key(&gone));

    // a corrupted download is refused and nothing local changes
    let before = local_sha(&s.paths.wellness.join("days.parquet"));
    let rhr = data.days.num.get_mut("rhr").unwrap();
    rhr[0] += 1.0;
    write(&remote, &data);
    std::fs::write(remote.join("days.parquet"), b"not parquet").unwrap();
    let e = sync::run(&s).await.unwrap_err();
    assert!(format!("{e:#}").contains("sha256"), "{e:#}");
    assert!(s.sync_status().last_error.unwrap().contains("sha256"));
    assert_eq!(local_sha(&s.paths.wellness.join("days.parquet")), before);

    // a wrong token is reported
    let mut c = cfg(d.path());
    c.tenant = "other".into();
    c.garmin_mcp_url = Some(url);
    c.garmin_mcp_token = Some("wrong".into());
    let s2 = AppState::open(c).unwrap();
    let e = sync::sync_once(&s2).await.unwrap_err();
    assert!(format!("{e:#}").contains("GARMIN_MCP_TOKEN"), "{e:#}");
}

// ---------------------------------------------------------------- notifications

#[derive(Clone, Default)]
struct Captured {
    ntfy: Arc<Mutex<Vec<(HeaderMap, String)>>>,
    hook: Arc<Mutex<Vec<Value>>>,
}

async fn stub_ntfy(State(c): State<Captured>, headers: HeaderMap, body: String) -> StatusCode {
    c.ntfy.lock().unwrap().push((headers, body));
    StatusCode::OK
}

async fn stub_hook(State(c): State<Captured>, body: String) -> StatusCode {
    c.hook
        .lock()
        .unwrap()
        .push(serde_json::from_str(&body).unwrap());
    StatusCode::OK
}

fn proposal(id: &str, kind: &str, start: i32, priority: f64, asked: bool, auto: bool) -> Proposal {
    serde_json::from_value(json!({
        "id": id, "kind": kind, "start_day": start, "end_day": start + 2,
        "posterior": {kind: 0.5, "other": 0.5},
        "evidence": {"nights": 3, "d_rhr": 5.2, "d_lnrmssd": (0.82f64).ln(), "d_resp": 1.24, "d_temp": null,
                     "d_stress": null, "d_early_late": null, "p_value": null, "hard_day_before": false,
                     "weekend": false, "tz_overlap": false, "alt_overlap": false, "signals": ["fusion"]},
        "score": 2.0, "priority": priority, "status": "open", "auto": auto, "asked": asked,
        "model_version": "test"
    }))
    .unwrap()
}

#[tokio::test]
async fn notifications_fire_within_the_budget() {
    let cap = Captured::default();
    let url = serve(
        Router::new()
            .route("/ntfy", post(stub_ntfy))
            .route("/hook", post(stub_hook))
            .with_state(cap.clone()),
    )
    .await;
    let d = tempfile::tempdir().unwrap();
    let mut c = cfg(d.path());
    c.ntfy_url = Some(format!("{url}/ntfy"));
    c.ntfy_token = Some("ntfy-secret".into());
    c.webhook_url = Some(format!("{url}/hook"));
    c.public_url = Some("https://lab.example".into());
    let s = AppState::open(c).unwrap();
    for p in [
        proposal("illness:20100", "illness", 20100, 3.0, true, false),
        proposal("alcohol:20120", "alcohol", 20120, 2.0, true, false),
        proposal("stress:20140", "stress", 20140, 1.0, true, false),
        proposal("travel:20150", "travel", 20150, 9.0, true, true),
        proposal("alcohol:20160", "alcohol", 20160, 9.0, false, false),
    ] {
        s.store.put_proposal(&p).unwrap();
    }

    let sent = notify::send_new(&s).await;
    assert_eq!(
        sent,
        vec!["illness:20100".to_string(), "alcohol:20120".to_string()]
    );
    let ntfy = cap.ntfy.lock().unwrap().clone();
    assert_eq!(ntfy.len(), 2);
    let (h, body) = &ntfy[0];
    assert_eq!(h["title"], "Wellness Lab");
    assert_eq!(h["priority"], "4");
    assert_eq!(h["click"], "https://lab.example/inbox");
    assert_eq!(h["authorization"], "Bearer ntfy-secret");
    assert_eq!(
        body,
        &format!(
            "Possible illness, 3 nights from {}: resting HR +5 bpm, HRV -18%, breathing +1.2/min. Confirm or deny: https://lab.example/inbox",
            notify::mon_d(20100)
        )
    );
    assert_eq!(ntfy[1].0["priority"], "3");
    assert!(ntfy[1].1.starts_with("Possible alcohol, 3 nights from"));
    let hook = cap.hook.lock().unwrap().clone();
    assert_eq!(hook.len(), 2);
    assert_eq!(hook[0]["type"], "proposal");
    assert_eq!(hook[0]["proposal"]["id"], "illness:20100");
    assert_eq!(hook[0]["text"], json!(ntfy[0].1));
    assert_eq!(hook[0]["url"], "https://lab.example/inbox");

    let stored = s.store.proposal("illness:20100").unwrap().unwrap();
    assert!(stored.notified_at.is_some());
    assert!(s
        .store
        .proposal("stress:20140")
        .unwrap()
        .unwrap()
        .notified_at
        .is_none());
    assert_eq!(s.store.notifications(10).unwrap().len(), 4);

    // the week's budget is spent: nothing more goes out
    assert!(notify::send_new(&s).await.is_empty());
    assert_eq!(cap.ntfy.lock().unwrap().len(), 2);

    // the test endpoint reaches both channels
    let app = api::router(s.clone());
    let r = call(&app, Method::POST, "/api/notify/test", None).await;
    assert_eq!(
        r.json(),
        json!({"ntfy": {"ok": true}, "webhook": {"ok": true}})
    );
    assert_eq!(
        cap.ntfy.lock().unwrap().last().unwrap().1,
        "Wellness Lab test notification"
    );
}

#[test]
fn notification_text_without_a_public_url() {
    let mut p = proposal(
        "illness:1",
        "hard_training",
        wellness_lab_core::time::days_from_ymd(2026, 11, 3),
        1.0,
        true,
        false,
    );
    p.evidence.nights = 1;
    p.evidence.d_lnrmssd = None;
    p.evidence.d_resp = None;
    p.evidence.d_stress = Some(-3.4);
    assert_eq!(
        notify::text(&p, None),
        "Possible hard training, 1 night from Nov 3: resting HR +5 bpm, sleep stress -3."
    );
}

// ---------------------------------------------------------------- mcp

async fn mcp(app: &Router, body: Value) -> Resp {
    send(
        app,
        Request::post("/mcp")
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header(header::HOST, "localhost")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

fn rpc_result(r: &Resp) -> Value {
    let text = String::from_utf8_lossy(&r.bytes).to_string();
    // JSON, or one SSE event carrying it
    let payload = text
        .lines()
        .find_map(|l| l.strip_prefix("data:").map(|x| x.trim().to_string()))
        .filter(|x| !x.is_empty())
        .unwrap_or(text);
    serde_json::from_str::<Value>(&payload).unwrap_or(Value::Null)["result"].clone()
}

#[tokio::test]
async fn mcp_lists_and_calls_read_only_tools() {
    let d = tempfile::tempdir().unwrap();
    let s = AppState::open(cfg(d.path())).unwrap();
    s.store
        .put_proposal(&proposal(
            "illness:20100",
            "illness",
            20100,
            3.0,
            true,
            false,
        ))
        .unwrap();
    let app = api::router(s);
    let init = mcp(
        &app,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}),
    )
    .await;
    assert_eq!(
        init.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&init.bytes)
    );
    assert_eq!(rpc_result(&init)["serverInfo"]["name"], "wellness-lab");
    let list = mcp(
        &app,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    let names: Vec<String> = rpc_result(&list)["tools"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for n in [
        "list_episodes",
        "get_night",
        "nights",
        "get_proposals",
        "get_bench",
        "get_hrv_status",
        "event_study",
    ] {
        assert!(names.contains(&n.to_string()), "{n} in {names:?}");
    }
    let call = mcp(
        &app,
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_proposals", "arguments": {}}}),
    )
    .await;
    let res = rpc_result(&call);
    let text = res["content"][0]["text"].as_str().unwrap_or_default();
    let v: Value = serde_json::from_str(text).unwrap();
    assert_eq!(v[0]["id"], "illness:20100");
    assert_eq!(v[0]["start_date"], wellness_lab_core::time::iso(20100));
}
