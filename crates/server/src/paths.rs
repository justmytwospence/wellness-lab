//! Every file a tenant owns lives under `TenantPaths`, so more tenants later are a configuration change.

use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct TenantPaths {
    /// `<data>/tenants/<tenant>`
    pub root: PathBuf,
    /// the wellness-v1 directory the engine reads and `/data/wellness-v1` serves
    pub wellness: PathBuf,
    /// the SQLite database with labels, proposals, settings and experiments
    pub state: PathBuf,
    pub cache: PathBuf,
    /// downloads land here before they are verified and moved into `wellness`
    pub tmp: PathBuf,
}

impl TenantPaths {
    pub fn new(data_dir: &Path, tenant: &str) -> Self {
        let root = data_dir.join("tenants").join(tenant);
        TenantPaths {
            wellness: root.join("wellness-v1"),
            state: root.join("state.sqlite"),
            cache: root.join("cache"),
            tmp: root.join("tmp"),
            root,
        }
    }

    pub fn create(&self) -> io::Result<()> {
        for d in [&self.root, &self.wellness, &self.cache, &self.tmp] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    pub fn has_data(&self) -> bool {
        self.wellness.join("manifest.json").is_file()
    }
}
