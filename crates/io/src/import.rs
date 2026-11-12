//! The Garmin account export zip to wellness-v1 (the builder lives in `wellness-lab-fit`).
use super::Result;
use std::path::Path;

pub fn import_zip(_zip: &Path, _out: &Path, _importer: &str) -> Result<()> {
    Ok(())
}
