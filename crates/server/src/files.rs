//! `/data/wellness-v1/*`: the tenant's wellness-v1 files for the browser, with ETags (the manifest's sha256) and byte
//! ranges; and the built app with an index.html fallback.

use crate::AppState;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::{Arc, LazyLock};
use tower::ServiceExt;
use tower_http::services::ServeFile;

/// garmin-mcp's `_SERVABLE` (Part 2), anchored.
static SERVABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:manifest\.json|(?:days|sleeps|stages|activities|tags|devices)\.parquet|series/[a-z0-9_]+/\d{4}-\d{2}\.parquet|rr/garmin-\d+\.parquet)$",
    )
    .unwrap()
});

pub fn is_servable(rel: &str) -> bool {
    SERVABLE.is_match(rel)
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({"error": "not found"})),
    )
        .into_response()
}

fn etag_matches(req: &Request, etag: &str) -> bool {
    req.headers()
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|v| v.trim().trim_start_matches("W/"))
        .any(|v| v == etag || v == "*")
}

pub async fn wellness_file(
    State(s): State<Arc<AppState>>,
    Path(rel): Path<String>,
    req: Request,
) -> Response {
    if !is_servable(&rel) {
        return not_found();
    }
    let path = s.paths.wellness.join(&rel);
    let sha = if rel == "manifest.json" {
        match tokio::fs::read(&path).await {
            Ok(b) => hex::encode(Sha256::digest(&b)),
            Err(_) => return not_found(),
        }
    } else {
        match s
            .manifest()
            .and_then(|m| m.files.into_iter().find(|f| f.path == rel))
        {
            Some(f) => f.sha256,
            None => return not_found(),
        }
    };
    let etag = format!("\"{sha}\"");
    let ctype = if rel == "manifest.json" {
        "application/json"
    } else {
        "application/octet-stream"
    };
    let mut res = if etag_matches(&req, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        match ServeFile::new(&path).oneshot(req).await {
            Ok(r) => {
                if r.status() == StatusCode::NOT_FOUND {
                    return not_found();
                }
                r.map(Body::new)
            }
            Err(e) => match e {},
        }
    };
    let h = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(header::ETAG, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if h.contains_key(header::CONTENT_TYPE) || res.status().is_success() {
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(ctype));
    }
    res
}
