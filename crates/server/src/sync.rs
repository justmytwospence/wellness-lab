//! Pull garmin-mcp's wellness-v1 export into the tenant's directory, downloading only files whose sha256 changed.

use crate::files::is_servable;
use crate::{lab, lock, now, AppState};
use anyhow::{anyhow, bail, Context, Result};
use futures::{StreamExt, TryStreamExt};
use reqwest::{header, StatusCode};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use wellness_lab_io::Manifest;

const TIMEOUT: Duration = Duration::from_secs(30);
const RETRIES: u32 = 6;
const PARALLEL: usize = 4;
const MAX_BACKOFF: Duration = Duration::from_secs(2 * 3600);
pub const EXPORT_PATH: &str = "/export/wellness/v1/";

#[derive(Serialize, Debug, Default, Clone)]
pub struct SyncReport {
    /// garmin-mcp said the manifest was unchanged (304)
    pub not_modified: bool,
    pub files: usize,
    pub downloaded: usize,
    pub removed: usize,
    pub changed: bool,
}

fn retry_after(r: &reqwest::Response, attempt: u32) -> Duration {
    r.headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(1 << attempt.min(5)))
        .min(Duration::from_secs(120))
}

/// GET with the bearer token, a 30 s timeout, and up to 6 retries of 429 and 503 honoring Retry-After.
async fn get(s: &AppState, url: &str, etag: Option<&str>) -> Result<reqwest::Response> {
    let token = s.cfg.garmin_mcp_token.as_deref().unwrap_or_default();
    let mut attempt = 0;
    loop {
        let mut rq = s.http.get(url).timeout(TIMEOUT).bearer_auth(token);
        if let Some(e) = etag {
            rq = rq.header(header::IF_NONE_MATCH, e);
        }
        let r = rq.send().await.with_context(|| format!("GET {url}"))?;
        let st = r.status();
        if (st == StatusCode::TOO_MANY_REQUESTS || st == StatusCode::SERVICE_UNAVAILABLE)
            && attempt < RETRIES
        {
            let wait = retry_after(&r, attempt);
            attempt += 1;
            tracing::info!(%url, status = %st, ?wait, attempt, "retrying");
            tokio::time::sleep(wait).await;
            continue;
        }
        return Ok(r);
    }
}

fn status_error(what: &str, st: StatusCode) -> anyhow::Error {
    if st == StatusCode::UNAUTHORIZED || st == StatusCode::FORBIDDEN {
        anyhow!("garmin-mcp refused {what} ({st}): check GARMIN_MCP_TOKEN")
    } else if st == StatusCode::NOT_FOUND {
        anyhow!("garmin-mcp has no {what} (404): set GARMIN_EXPORT=wellness on garmin-mcp")
    } else {
        anyhow!("garmin-mcp answered {st} for {what}")
    }
}

fn tmp_name(tmp: &Path, rel: &str) -> PathBuf {
    tmp.join(format!("{}.part", rel.replace('/', "__")))
}

/// Download one file to `tmp`, hashing as it streams; fails when the sha256 is not the manifest's.
async fn download(s: &AppState, base: &str, rel: &str, sha: &str) -> Result<PathBuf> {
    let url = format!("{base}{rel}");
    let r = get(s, &url, None).await?;
    if !r.status().is_success() {
        return Err(status_error(rel, r.status()));
    }
    let dest = tmp_name(&s.paths.tmp, rel);
    let mut f = tokio::fs::File::create(&dest).await?;
    let mut h = Sha256::new();
    let mut body = r.bytes_stream().map_err(std::io::Error::other);
    while let Some(chunk) = body.next().await {
        let chunk = chunk.with_context(|| format!("reading {rel}"))?;
        h.update(&chunk);
        f.write_all(&chunk).await?;
    }
    f.flush().await?;
    drop(f);
    let got = hex::encode(h.finalize());
    if !got.eq_ignore_ascii_case(sha) {
        let _ = tokio::fs::remove_file(&dest).await;
        bail!("{rel}: sha256 {got} is not the manifest's {sha}");
    }
    Ok(dest)
}

/// One sync: fetch the manifest (conditionally), download what changed, move it in, delete what is gone, write the
/// manifest last; then reload and re-run detection when anything changed.
pub async fn sync_once(s: &Arc<AppState>) -> Result<SyncReport> {
    let _g = s.sync_lock.lock().await;
    let mcp = s
        .cfg
        .garmin_mcp_url
        .as_deref()
        .ok_or_else(|| anyhow!("GARMIN_MCP_URL is not set"))?;
    let base = format!("{mcp}{EXPORT_PATH}");
    let local: Option<Manifest> = wellness_lab_io::read_manifest(&s.paths.wellness).ok();
    let etag = local
        .as_ref()
        .and_then(|_| lock(&s.status).manifest_etag.clone());
    let r = get(s, &format!("{base}manifest.json"), etag.as_deref()).await?;
    if r.status() == StatusCode::NOT_MODIFIED {
        return Ok(SyncReport {
            not_modified: true,
            files: local.map(|m| m.files.len()).unwrap_or(0),
            ..Default::default()
        });
    }
    if !r.status().is_success() {
        return Err(status_error("the wellness export", r.status()));
    }
    let new_etag = r
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = r.bytes().await.context("reading manifest.json")?;
    let remote: Manifest = serde_json::from_slice(&bytes)
        .context("garmin-mcp's manifest.json is not a wellness-v1 manifest")?;
    if remote.schema_version != 1 {
        bail!(
            "garmin-mcp exports wellness schema version {}, this server reads version 1",
            remote.schema_version
        );
    }
    for f in &remote.files {
        if f.path == "manifest.json" || !is_servable(&f.path) {
            bail!(
                "garmin-mcp's manifest lists an unexpected path {:?}",
                f.path
            );
        }
    }
    let have: BTreeMap<&str, &str> = local
        .as_ref()
        .map(|m| {
            m.files
                .iter()
                .map(|f| (f.path.as_str(), f.sha256.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let wanted: Vec<(String, String)> = remote
        .files
        .iter()
        .filter(|f| {
            have.get(f.path.as_str()) != Some(&f.sha256.as_str())
                || !s.paths.wellness.join(&f.path).is_file()
        })
        .map(|f| (f.path.clone(), f.sha256.clone()))
        .collect();
    tokio::fs::create_dir_all(&s.paths.tmp).await?;
    let fetched: Vec<(String, PathBuf)> = futures::stream::iter(wanted.iter().cloned())
        .map(|(rel, sha)| {
            let base = base.clone();
            async move {
                let p = download(s, &base, &rel, &sha).await?;
                Ok::<_, anyhow::Error>((rel, p))
            }
        })
        .buffer_unordered(PARALLEL)
        .try_collect()
        .await?;
    // everything verified: move it in, delete what the export dropped, write the manifest last
    for (rel, tmp) in &fetched {
        let dest = s.paths.wellness.join(rel);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(tmp, &dest).await?;
    }
    let keep: std::collections::BTreeSet<&str> =
        remote.files.iter().map(|f| f.path.as_str()).collect();
    let mut removed = 0;
    for old in have.keys().filter(|p| !keep.contains(*p)) {
        if is_servable(old)
            && tokio::fs::remove_file(s.paths.wellness.join(old))
                .await
                .is_ok()
        {
            removed += 1;
        }
    }
    let tmp = s.paths.wellness.join("manifest.json.tmp");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, s.paths.wellness.join("manifest.json")).await?;
    lock(&s.status).manifest_etag = new_etag;
    let changed = !fetched.is_empty() || removed > 0 || local.is_none();
    let report = SyncReport {
        not_modified: false,
        files: remote.files.len(),
        downloaded: fetched.len(),
        removed,
        changed,
    };
    if changed {
        lab::reload(s).await?;
        lab::redetect(s).await?;
    } else {
        *s.manifest.write().unwrap_or_else(|e| e.into_inner()) = Some(remote);
    }
    Ok(report)
}

/// `sync_once` with the outcome recorded in `last_sync` / `last_error`.
pub async fn run(s: &Arc<AppState>) -> Result<SyncReport> {
    let r = sync_once(s).await;
    let mut st = lock(&s.status);
    match &r {
        Ok(rep) => {
            st.last_sync = Some(now());
            st.last_error = None;
            if rep.changed {
                tracing::info!(
                    downloaded = rep.downloaded,
                    removed = rep.removed,
                    files = rep.files,
                    "synced"
                );
            } else {
                tracing::debug!("sync: nothing changed");
            }
        }
        Err(e) => {
            st.last_error = Some(format!("{e:#}"));
            tracing::warn!(error = %format!("{e:#}"), "sync failed");
        }
    }
    r
}

/// The sync loop: now, then every `LAB_SYNC_MINUTES` (doubling up to 2 h after failures), or when `POST /api/sync`
/// asks. Without `GARMIN_MCP_URL` there is no loop (import mode).
pub fn spawn(
    s: Arc<AppState>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Option<tokio::task::JoinHandle<()>> {
    s.cfg.garmin_mcp_url.as_ref()?;
    let base = Duration::from_secs(s.cfg.sync_minutes.max(1) * 60);
    Some(tokio::spawn(async move {
        let mut delay = base;
        loop {
            let r = tokio::select! {
                r = run(&s) => r,
                _ = shutdown.changed() => break,
            };
            delay = match r {
                Ok(_) => base,
                Err(_) => (delay * 2).min(MAX_BACKOFF).max(base),
            };
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = s.sync_now.notified() => {}
                _ = shutdown.changed() => break,
            }
        }
    }))
}
