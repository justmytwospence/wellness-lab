//! The Wellness Lab server: serves the app, syncs wellness-v1 from garmin-mcp, keeps labels, proposals, settings and
//! experiments in SQLite, runs detection after each sync, sends notifications and answers read-only MCP tools.

pub mod api;
pub mod config;
pub mod files;
pub mod lab;
pub mod mcp;
pub mod notify;
pub mod paths;
pub mod state;
pub mod sync;

use anyhow::Result;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use wellness_lab_core::lab::Lab;
use wellness_lab_io::Manifest;

pub use config::Config;
pub use paths::TenantPaths;
pub use state::Store;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Now as RFC 3339 UTC with milliseconds, e.g. `2026-11-12T18:00:00.000Z` (sorts as text).
pub fn now() -> String {
    rfc3339(time::OffsetDateTime::now_utc())
}

pub fn rfc3339(t: time::OffsetDateTime) -> String {
    let t = t.to_offset(time::UtcOffset::UTC);
    t.format(time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
    ))
    .unwrap_or_default()
}

/// The time `days` days from now, formatted as `now()`.
pub fn days_from_now(days: i64) -> String {
    rfc3339(time::OffsetDateTime::now_utc() + time::Duration::days(days))
}

/// Today's calendar day (days since 1970-01-01, UTC).
pub fn today() -> i32 {
    (time::OffsetDateTime::now_utc()
        .unix_timestamp()
        .div_euclid(86_400)) as i32
}

/// What the last sync did.
#[derive(Default, Clone, Debug)]
pub struct SyncStatus {
    pub last_sync: Option<String>,
    pub last_error: Option<String>,
    /// garmin-mcp's ETag for the manifest, sent back as If-None-Match
    pub manifest_etag: Option<String>,
}

/// Everything a request or a background task needs.
pub struct AppState {
    pub cfg: Config,
    pub paths: TenantPaths,
    pub store: Store,
    pub lab: Mutex<Lab>,
    pub manifest: RwLock<Option<Manifest>>,
    pub status: Mutex<SyncStatus>,
    pub http: reqwest::Client,
    /// wakes the sync loop for `POST /api/sync`
    pub sync_now: tokio::sync::Notify,
    pub sync_lock: tokio::sync::Mutex<()>,
    pub detect_lock: tokio::sync::Mutex<()>,
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    /// Create the tenant's directories, open its database and load its data.
    pub fn open(cfg: Config) -> Result<Arc<AppState>> {
        let paths = TenantPaths::new(&cfg.data_dir, &cfg.tenant);
        paths.create()?;
        let store = Store::open(&paths.state)?;
        let http = reqwest::Client::builder()
            .user_agent(format!("wellness-lab/{VERSION}"))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()?;
        let s = Arc::new(AppState {
            cfg,
            paths,
            store,
            lab: Mutex::new(Lab::new()),
            manifest: RwLock::new(None),
            status: Mutex::new(SyncStatus::default()),
            http,
            sync_now: tokio::sync::Notify::new(),
            sync_lock: tokio::sync::Mutex::new(()),
            detect_lock: tokio::sync::Mutex::new(()),
        });
        lab::reload_blocking(&s)?;
        Ok(s)
    }

    pub fn manifest(&self) -> Option<Manifest> {
        self.manifest
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn sync_status(&self) -> SyncStatus {
        lock(&self.status).clone()
    }
}
