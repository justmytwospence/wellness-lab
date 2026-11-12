//! The Garmin account export zip to wellness-v1 (the builder lives in `wellness-lab-fit`).
//!
//! The export nests its FIT files in `DI-Connect-Uploaded-Files/UploadedFiles_*.zip` archives. The zip is read twice:
//! every JSON entry first (at any depth), then every FIT entry, because activity FITs are matched to the JSON
//! summaries and local offsets come from the daily summaries.

use super::{write_data, IoError, Result};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;
use wellness_lab_fit::{ExportBuilder, FitError, ImportReport, Tables};
use zip::ZipArchive;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Json,
    Fit,
}

/// Problems the builder never sees: unreadable entries and nested archives, activity FITs refused for lack of JSON.
#[derive(Default)]
struct Notes {
    errors: Vec<String>,
    json_first: u32,
}

fn zerr(e: zip::result::ZipError) -> IoError {
    IoError::Other(format!("zip: {e}"))
}

fn walk<R: Read + Seek>(
    ar: &mut ZipArchive<R>,
    prefix: &str,
    pass: Pass,
    b: &mut ExportBuilder,
    notes: &mut Notes,
) -> Result<()> {
    for i in 0..ar.len() {
        let mut f = match ar.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                notes.errors.push(format!("{prefix}#{i}: {e}"));
                continue;
            }
        };
        if f.is_dir() {
            continue;
        }
        let name = format!(
            "{prefix}{}",
            f.name().map(|n| n.into_owned()).unwrap_or_default()
        );
        let lower = name.to_lowercase();
        let wanted = lower.ends_with(".zip")
            || (pass == Pass::Json && lower.ends_with(".json"))
            || (pass == Pass::Fit && lower.ends_with(".fit"));
        if !wanted {
            continue;
        }
        let mut bytes = Vec::with_capacity(f.size().min(1 << 30) as usize);
        if let Err(e) = f.read_to_end(&mut bytes) {
            notes.errors.push(format!("{name}: {e}"));
            continue;
        }
        drop(f);
        if lower.ends_with(".zip") {
            match ZipArchive::new(Cursor::new(bytes)) {
                Ok(mut inner) => walk(&mut inner, &format!("{name}/"), pass, b, notes)?,
                Err(e) => {
                    if pass == Pass::Json {
                        notes.errors.push(format!("{name}: {e}"));
                    }
                }
            }
        } else if pass == Pass::Json {
            // a JSON the builder cannot parse is recorded in its report
            let _ = b.add_json(&name, &String::from_utf8_lossy(&bytes));
        } else if let Err(FitError::JsonFirst) = b.add_fit(&name, &bytes) {
            // (decoding errors are recorded in the builder's report and never abort the import)
            notes.json_first += 1;
        }
    }
    Ok(())
}

/// Build the wellness-v1 tables of a Garmin account export zip in memory.
pub fn build_tables(zip: &Path) -> Result<(Tables, ImportReport)> {
    let mut b = ExportBuilder::new();
    let mut notes = Notes::default();
    for pass in [Pass::Json, Pass::Fit] {
        let mut ar = ZipArchive::new(BufReader::new(File::open(zip)?)).map_err(zerr)?;
        walk(&mut ar, "", pass, &mut b, &mut notes)?;
    }
    let (tables, mut report) = b.finish();
    if notes.json_first > 0 {
        report.skipped += notes.json_first;
        notes.errors.push(format!(
            "{} activity FIT files skipped: the export has no activity summaries or daily summaries",
            notes.json_first
        ));
    }
    for e in notes.errors {
        if report.errors.len() < 100 {
            report.errors.push(e);
        }
    }
    Ok((tables, report))
}

/// Import a Garmin account export zip into the wellness-v1 directory `out` (`source.kind = "garmin-export"`).
pub fn import_zip(zip: &Path, out: &Path, importer: &str) -> Result<ImportReport> {
    let (tables, report) = build_tables(zip)?;
    write_data(out, &tables.to_data(), importer, "garmin-export")?;
    Ok(report)
}

#[cfg(test)]
#[path = "../../fit/examples/make_fake_export.rs"]
mod fake;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_dir, read_manifest};

    #[test]
    fn imports_the_fake_export() {
        let tmp = std::env::temp_dir().join(format!("wl-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let zip = tmp.join("export.zip");
        fake::make_fake_export(&zip).unwrap();
        let out = tmp.join("wellness-v1");
        let rep = import_zip(&zip, &out, "test").unwrap();
        assert!(rep.errors.is_empty(), "{:?}", rep.errors);
        assert_eq!(rep.json_files["uds"], 1);
        assert_eq!(rep.json_files["activities_json"], 1);
        assert_eq!(rep.fit_files["monitoring_b"], 14);
        assert_eq!(rep.fit_files["49"], 14);
        assert_eq!(rep.fit_files["68"], 14);
        assert_eq!(rep.fit_files["activity"], 1);
        assert_eq!(rep.activities_with_rr, 1);

        let m = read_manifest(&out).unwrap();
        assert_eq!(m.source.kind, "garmin-export");
        assert!(m
            .files
            .iter()
            .any(|f| f.path == "series/hr/2026-10.parquet"));
        let d = read_dir(&out).unwrap();
        assert_eq!(d.days.n, 14);
        assert_eq!(d.sleeps.n, 14);
        assert_eq!(d.activities.n, 3);
        assert_eq!(d.offset_of_day(d.days.col("day")[0] as i32), -420);
        assert_eq!(d.series["hr"].len(), 14 * 120);
        assert_eq!(d.series["hrv5"].len(), 14 * 8);
        assert!(d.stages.n >= 14 * 4);
        let rr = d.rr.values().next().unwrap();
        assert_eq!(rr.len(), 600);
        let steps: f64 = d.series["steps"].v().iter().sum();
        assert_eq!(steps, 14.0 * 119.0 * 200.0);

        // a second import of the same export rewrites nothing
        let before = read_manifest(&out).unwrap();
        import_zip(&zip, &out, "test").unwrap();
        let after = read_manifest(&out).unwrap();
        for (a, b) in before.files.iter().zip(&after.files) {
            assert_eq!((&a.path, &a.sha256), (&b.path, &b.sha256));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
