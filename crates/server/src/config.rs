//! Settings from the command line and the environment.

use clap::Args;
use std::path::PathBuf;

pub const NO_TOKEN_WARNING: &str =
    "LAB_TOKEN is not set: anyone who can reach port 8080 can read and change this lab's data.";
pub const MCP_TOKEN_MISSING: &str =
    "GARMIN_MCP_URL needs GARMIN_MCP_TOKEN (garmin-mcp's EXPORT_TOKEN)";

#[derive(Args, Clone, Debug)]
pub struct Config {
    /// Where tenants keep their data, state and caches.
    #[arg(long, env = "LAB_DATA_DIR", default_value = "/data", global = true)]
    pub data_dir: PathBuf,
    /// The built app (served with an index.html fallback).
    #[arg(
        long,
        env = "LAB_STATIC_DIR",
        default_value = "/app/dist",
        global = true
    )]
    pub static_dir: PathBuf,
    #[arg(long, env = "LAB_PORT", default_value_t = 8080, global = true)]
    pub port: u16,
    /// Bearer token for the API, data files and /mcp; unset means no authentication.
    #[arg(long, env = "LAB_TOKEN", global = true, hide_env_values = true)]
    pub token: Option<String>,
    /// The address users reach the lab at, for links in notifications.
    #[arg(long, env = "LAB_PUBLIC_URL", global = true)]
    pub public_url: Option<String>,
    #[arg(long, env = "LAB_TENANT", default_value = "default", global = true)]
    pub tenant: String,
    /// garmin-mcp's base URL; unset means import mode (no sync).
    #[arg(long, env = "GARMIN_MCP_URL", global = true)]
    pub garmin_mcp_url: Option<String>,
    /// garmin-mcp's EXPORT_TOKEN (or an OIDC access token for client wellness-lab).
    #[arg(long, env = "GARMIN_MCP_TOKEN", global = true, hide_env_values = true)]
    pub garmin_mcp_token: Option<String>,
    #[arg(long, env = "LAB_SYNC_MINUTES", default_value_t = 15, global = true)]
    pub sync_minutes: u64,
    #[arg(long, env = "NTFY_URL", global = true)]
    pub ntfy_url: Option<String>,
    #[arg(long, env = "NTFY_TOKEN", global = true, hide_env_values = true)]
    pub ntfy_token: Option<String>,
    #[arg(long, env = "NOTIFY_WEBHOOK_URL", global = true)]
    pub webhook_url: Option<String>,
    /// Serve the read-only MCP endpoint at /mcp: on or off.
    #[arg(long, env = "LAB_MCP", default_value = "on", global = true)]
    pub mcp: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            data_dir: "/data".into(),
            static_dir: "/app/dist".into(),
            port: 8080,
            token: None,
            public_url: None,
            tenant: "default".into(),
            garmin_mcp_url: None,
            garmin_mcp_token: None,
            sync_minutes: 15,
            ntfy_url: None,
            ntfy_token: None,
            webhook_url: None,
            mcp: "on".into(),
        }
    }
}

fn nonempty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

impl Config {
    /// Empty environment values (compose's `${X:-}`) count as unset.
    pub fn normalized(mut self) -> Self {
        for f in [
            &mut self.token,
            &mut self.public_url,
            &mut self.garmin_mcp_url,
            &mut self.garmin_mcp_token,
            &mut self.ntfy_url,
            &mut self.ntfy_token,
            &mut self.webhook_url,
        ] {
            *f = nonempty(f).map(str::to_string);
        }
        if let Some(u) = &mut self.public_url {
            while u.ends_with('/') {
                u.pop();
            }
        }
        if let Some(u) = &mut self.garmin_mcp_url {
            while u.ends_with('/') {
                u.pop();
            }
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.garmin_mcp_url.is_some() && self.garmin_mcp_token.is_none() {
            return Err(MCP_TOKEN_MISSING.into());
        }
        if self.tenant.is_empty()
            || !self
                .tenant
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "LAB_TENANT must be letters, digits, '-' or '_', got \"{}\"",
                self.tenant
            ));
        }
        if !["on", "off", "true", "false", "1", "0"]
            .contains(&self.mcp.to_ascii_lowercase().as_str())
        {
            return Err(format!("LAB_MCP must be on or off, got \"{}\"", self.mcp));
        }
        Ok(())
    }

    pub fn mcp_enabled(&self) -> bool {
        matches!(self.mcp.to_ascii_lowercase().as_str(), "on" | "true" | "1")
    }

    pub fn secure_cookies(&self) -> bool {
        self.public_url
            .as_deref()
            .map(|u| u.starts_with("https://"))
            .unwrap_or(false)
    }

    pub fn inbox_url(&self) -> Option<String> {
        self.public_url.as_ref().map(|u| format!("{u}/inbox"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garmin_mcp_url_needs_a_token() {
        let c = Config {
            garmin_mcp_url: Some("http://garmin-mcp:8000/".into()),
            garmin_mcp_token: Some(" ".into()),
            ..Default::default()
        }
        .normalized();
        assert_eq!(c.garmin_mcp_url.as_deref(), Some("http://garmin-mcp:8000"));
        assert_eq!(c.validate().unwrap_err(), MCP_TOKEN_MISSING);
        let c = Config {
            garmin_mcp_token: Some("t".into()),
            ..c
        };
        assert!(c.validate().is_ok());
        assert!(Config {
            tenant: "../x".into(),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(!Config {
            mcp: "off".into(),
            ..Default::default()
        }
        .mcp_enabled());
    }
}
