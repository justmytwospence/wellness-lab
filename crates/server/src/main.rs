//! `wellness-lab-server`: serve (default), import-export, detect, synth and healthcheck.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use wellness_lab_server::config::{Config, NO_TOKEN_WARNING};
use wellness_lab_server::{api, lab, sync, today, AppState, TenantPaths, VERSION};

#[derive(Parser)]
#[command(name = "wellness-lab-server", version, about = "Wellness Lab server")]
struct Cli {
    #[command(flatten)]
    cfg: Config,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open the state, load the data, run detection, start syncing and serve (the default).
    Serve,
    /// Import a Garmin account export zip into the tenant's wellness-v1 directory (`--tenant`), then run detection.
    ImportExport { zip: PathBuf },
    /// Run detection once (and send due notifications).
    Detect,
    /// Write synthetic data into the tenant (for demos and end-to-end tests).
    Synth {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 730)]
        days: i32,
    },
    /// Exit 0 when the local server answers /health with 200.
    Healthcheck,
}

async fn shutdown_signal(tx: tokio::sync::watch::Sender<bool>) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("shutting down");
    let _ = tx.send(true);
}

async fn serve(cfg: Config) -> Result<()> {
    if cfg.token.is_none() {
        tracing::warn!("{NO_TOKEN_WARNING}");
    }
    let port = cfg.port;
    let s = AppState::open(cfg)?;
    tracing::info!(
        tenant = %s.cfg.tenant,
        files = s.manifest().map(|m| m.files.len()).unwrap_or(0),
        "loaded"
    );
    match lab::redetect(&s).await {
        Ok(r) => tracing::info!(proposals = r.proposals, asked = r.asked, "detection"),
        Err(e) => tracing::warn!(error = %format!("{e:#}"), "detection failed"),
    }
    let (tx, rx) = tokio::sync::watch::channel(false);
    let sync_task = sync::spawn(s.clone(), rx.clone());
    if sync_task.is_none() {
        tracing::info!("GARMIN_MCP_URL is not set: import mode, no sync");
    }
    let app = api::router(s.clone());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .with_context(|| format!("bind port {port}"))?;
    tracing::info!(port, version = VERSION, "listening");
    let mut done = rx.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown_signal(tx) => {},
                _ = done.changed() => {},
            }
        })
        .await?;
    if let Some(t) = sync_task {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), t).await;
    }
    Ok(())
}

async fn import_export(cfg: Config, zip: PathBuf) -> Result<()> {
    let paths = TenantPaths::new(&cfg.data_dir, &cfg.tenant);
    paths.create()?;
    let importer = format!("wellness-lab-import@{VERSION}");
    let out = paths.wellness.clone();
    tokio::task::spawn_blocking(move || {
        wellness_lab_io::import_zip(&zip, &out, &importer)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("import failed: {e}"))
    })
    .await??;
    let s = AppState::open(cfg)?;
    let r = lab::redetect(&s).await?;
    println!(
        "imported {} files; {} proposals ({} to ask)",
        s.manifest().map(|m| m.files.len()).unwrap_or(0),
        r.proposals,
        r.asked
    );
    Ok(())
}

async fn detect(cfg: Config) -> Result<()> {
    let s = AppState::open(cfg)?;
    let r = lab::redetect(&s).await?;
    println!("{}", serde_json::to_string(&r)?);
    Ok(())
}

async fn synth(cfg: Config, seed: u64, days: i32) -> Result<()> {
    let paths = TenantPaths::new(&cfg.data_dir, &cfg.tenant);
    paths.create()?;
    let out = paths.wellness.clone();
    let start = today() - days;
    tokio::task::spawn_blocking(move || {
        let (data, _) = wellness_lab_core::synth::synth(seed, days, start);
        wellness_lab_io::write_data(
            &out,
            &data,
            &format!("wellness-lab-synth@{VERSION}"),
            "synth",
        )
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("write failed: {e}"))
    })
    .await??;
    let s: Arc<AppState> = AppState::open(cfg)?;
    let r = lab::redetect(&s).await?;
    println!(
        "wrote {} synthetic days ({} files); {} proposals",
        days,
        s.manifest().map(|m| m.files.len()).unwrap_or(0),
        r.proposals
    );
    Ok(())
}

async fn healthcheck(port: u16) -> bool {
    let c = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    matches!(
        c.get(format!("http://127.0.0.1:{port}/health")).send().await,
        Ok(r) if r.status() == reqwest::StatusCode::OK
    )
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("LAB_LOG").unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,tower_http=warn,rmcp=warn")
            }),
        )
        .with_target(false)
        .init();
    let cfg = cli.cfg.normalized();
    if let Err(e) = cfg.validate() {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    let r = match cli.cmd.unwrap_or(Cmd::Serve) {
        Cmd::Serve => serve(cfg).await,
        Cmd::ImportExport { zip } => import_export(cfg, zip).await,
        Cmd::Detect => detect(cfg).await,
        Cmd::Synth { seed, days } => synth(cfg, seed, days).await,
        Cmd::Healthcheck => {
            return if healthcheck(cfg.port).await {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_is_consistent() {
        Cli::command().debug_assert();
        let c =
            Cli::try_parse_from(["x", "import-export", "/import/a.zip", "--tenant", "t1"]).unwrap();
        assert_eq!(c.cfg.tenant, "t1");
        assert!(matches!(c.cmd, Some(Cmd::ImportExport { .. })));
        let c = Cli::try_parse_from(["x"]).unwrap();
        assert!(c.cmd.is_none());
        assert_eq!(c.cfg.port, 8080);
    }
}
