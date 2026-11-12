//! The tenant's SQLite database: labels, proposals, settings, experiments, adherence and the notification log.

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use wellness_lab_core::labels::{Label, Proposal, Settings};

pub const USER_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE labels (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  start_day INTEGER NOT NULL,
  end_day INTEGER NOT NULL,
  onset_day INTEGER,
  severity INTEGER,
  note TEXT NOT NULL DEFAULT '',
  exclude_baseline INTEGER NOT NULL,
  source TEXT NOT NULL,
  proposal_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);
CREATE TABLE proposals (
  id TEXT PRIMARY KEY,
  body TEXT NOT NULL,
  status TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE experiments (
  id TEXT PRIMARY KEY,
  def TEXT NOT NULL,
  created_at TEXT NOT NULL,
  deleted_at TEXT
);
CREATE TABLE adherence (
  experiment_id TEXT,
  day INTEGER,
  adherent INTEGER,
  PRIMARY KEY (experiment_id, day)
);
CREATE TABLE notifications (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  proposal_id TEXT,
  channel TEXT NOT NULL,
  ok INTEGER NOT NULL,
  error TEXT,
  sent_at TEXT NOT NULL
);
";

/// One sent (or failed) notification.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Notification {
    pub id: i64,
    pub proposal_id: Option<String>,
    pub channel: String,
    pub ok: bool,
    pub error: Option<String>,
    pub sent_at: String,
}

/// An experiment as stored: its definition (the engine's `ExperimentDef` JSON) and bookkeeping.
#[derive(Serialize, Clone, Debug)]
pub struct Experiment {
    pub id: String,
    pub def: Value,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

pub struct Store {
    conn: Mutex<Connection>,
}

fn enum_str<T: Serialize>(v: &T) -> Result<String> {
    match serde_json::to_value(v)? {
        Value::String(s) => Ok(s),
        other => Err(anyhow!("not a string enum: {other}")),
    }
}

fn enum_from<T: DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_value(Value::String(s.to_string()))
        .with_context(|| format!("unknown value {s:?}"))
}

fn label_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(Label, String, String)> {
    let kind: String = r.get("kind")?;
    let source: String = r.get("source")?;
    let l = Label {
        id: r.get("id")?,
        // filled in by the caller from the two strings
        kind: wellness_lab_core::labels::LabelKind::Other,
        start_day: r.get("start_day")?,
        end_day: r.get("end_day")?,
        onset_day: r.get("onset_day")?,
        severity: r.get::<_, Option<i64>>("severity")?.map(|s| s as u8),
        note: r.get("note")?,
        exclude_baseline: r.get::<_, i64>("exclude_baseline")? != 0,
        source: wellness_lab_core::labels::Source::User,
        proposal_id: r.get("proposal_id")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        deleted_at: r.get("deleted_at")?,
    };
    Ok((l, kind, source))
}

fn finish_label((mut l, kind, source): (Label, String, String)) -> Result<Label> {
    l.kind = enum_from(&kind)?;
    l.source = enum_from(&source)?;
    Ok(l)
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match v {
            0 => {
                conn.execute_batch(&format!(
                    "BEGIN; {SCHEMA} PRAGMA user_version = {USER_VERSION}; COMMIT;"
                ))?;
            }
            USER_VERSION => {}
            v => bail!(
                "state.sqlite has schema version {v}, newer than this server understands ({USER_VERSION})"
            ),
        }
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    fn c(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn user_version(&self) -> Result<i64> {
        Ok(self
            .c()
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    pub fn journal_mode(&self) -> Result<String> {
        Ok(self
            .c()
            .pragma_query_value(None, "journal_mode", |r| r.get(0))?)
    }

    // ---- labels

    /// Every label, deleted ones included (with `deleted_at`), ordered by start day.
    pub fn labels(&self) -> Result<Vec<Label>> {
        let c = self.c();
        let mut st = c.prepare("SELECT * FROM labels ORDER BY start_day, created_at")?;
        let rows = st.query_map([], label_from_row)?;
        rows.map(|r| finish_label(r?)).collect()
    }

    pub fn live_labels(&self) -> Result<Vec<Label>> {
        Ok(self.labels()?.into_iter().filter(|l| l.live()).collect())
    }

    pub fn label(&self, id: &str) -> Result<Option<Label>> {
        let c = self.c();
        let r = c
            .query_row("SELECT * FROM labels WHERE id = ?1", [id], label_from_row)
            .optional()?;
        r.map(finish_label).transpose()
    }

    /// Insert or replace a label.
    pub fn put_label(&self, l: &Label) -> Result<()> {
        self.c().execute(
            "INSERT INTO labels (id, kind, start_day, end_day, onset_day, severity, note, exclude_baseline, source,
                                 proposal_id, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, start_day = excluded.start_day,
               end_day = excluded.end_day, onset_day = excluded.onset_day, severity = excluded.severity,
               note = excluded.note, exclude_baseline = excluded.exclude_baseline, source = excluded.source,
               proposal_id = excluded.proposal_id, created_at = excluded.created_at,
               updated_at = excluded.updated_at, deleted_at = excluded.deleted_at",
            params![
                l.id,
                enum_str(&l.kind)?,
                l.start_day,
                l.end_day,
                l.onset_day,
                l.severity.map(|s| s as i64),
                l.note,
                l.exclude_baseline as i64,
                enum_str(&l.source)?,
                l.proposal_id,
                l.created_at,
                l.updated_at,
                l.deleted_at,
            ],
        )?;
        Ok(())
    }

    // ---- proposals

    pub fn proposals(&self) -> Result<Vec<Proposal>> {
        let c = self.c();
        let mut st = c.prepare("SELECT body FROM proposals ORDER BY id")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        let mut out: Vec<Proposal> = vec![];
        for b in rows {
            out.push(serde_json::from_str(&b?)?);
        }
        out.sort_by(|a, b| a.start_day.cmp(&b.start_day).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    pub fn proposal(&self, id: &str) -> Result<Option<Proposal>> {
        let b: Option<String> = self
            .c()
            .query_row("SELECT body FROM proposals WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(b.map(|b| serde_json::from_str(&b)).transpose()?)
    }

    pub fn put_proposal(&self, p: &Proposal) -> Result<()> {
        put_proposal(&self.c(), p)
    }

    /// Make the stored proposals exactly `ps`, in one transaction.
    pub fn replace_proposals(&self, ps: &[Proposal]) -> Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM proposals", [])?;
        for p in ps {
            put_proposal(&tx, p)?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---- settings

    pub fn settings(&self) -> Result<Settings> {
        let v: Option<String> = self
            .c()
            .query_row(
                "SELECT value FROM settings WHERE key = 'settings'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match v {
            Some(v) => serde_json::from_str(&v)?,
            None => Settings::default(),
        })
    }

    pub fn put_settings(&self, s: &Settings) -> Result<()> {
        self.c().execute(
            "INSERT INTO settings (key, value) VALUES ('settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::to_string(s)?],
        )?;
        Ok(())
    }

    // ---- experiments

    pub fn experiments(&self) -> Result<Vec<Experiment>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT id, def, created_at, deleted_at FROM experiments WHERE deleted_at IS NULL ORDER BY created_at, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = vec![];
        for r in rows {
            let (id, def, created_at, deleted_at) = r?;
            out.push(Experiment {
                id,
                def: serde_json::from_str(&def)?,
                created_at,
                deleted_at,
            });
        }
        Ok(out)
    }

    pub fn experiment(&self, id: &str) -> Result<Option<Experiment>> {
        let r = self
            .c()
            .query_row(
                "SELECT id, def, created_at, deleted_at FROM experiments WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        r.map(|(id, def, created_at, deleted_at)| {
            Ok(Experiment {
                id,
                def: serde_json::from_str(&def)?,
                created_at,
                deleted_at,
            })
        })
        .transpose()
    }

    pub fn put_experiment(&self, e: &Experiment) -> Result<()> {
        self.c().execute(
            "INSERT INTO experiments (id, def, created_at, deleted_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET def = excluded.def, deleted_at = excluded.deleted_at",
            params![
                e.id,
                serde_json::to_string(&e.def)?,
                e.created_at,
                e.deleted_at
            ],
        )?;
        Ok(())
    }

    /// `{experiment_id: {day: adherent}}` for live experiments.
    pub fn adherence(&self) -> Result<BTreeMap<String, BTreeMap<i32, bool>>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT a.experiment_id, a.day, a.adherent FROM adherence a
             JOIN experiments e ON e.id = a.experiment_id WHERE e.deleted_at IS NULL",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, i64>(2)? != 0,
            ))
        })?;
        let mut out: BTreeMap<String, BTreeMap<i32, bool>> = BTreeMap::new();
        for r in rows {
            let (id, day, a) = r?;
            out.entry(id).or_default().insert(day, a);
        }
        Ok(out)
    }

    pub fn set_adherence(&self, experiment_id: &str, day: i32, adherent: bool) -> Result<()> {
        self.c().execute(
            "INSERT INTO adherence (experiment_id, day, adherent) VALUES (?1, ?2, ?3)
             ON CONFLICT(experiment_id, day) DO UPDATE SET adherent = excluded.adherent",
            params![experiment_id, day, adherent as i64],
        )?;
        Ok(())
    }

    // ---- notifications

    pub fn log_notification(
        &self,
        proposal_id: Option<&str>,
        channel: &str,
        ok: bool,
        error: Option<&str>,
        sent_at: &str,
    ) -> Result<()> {
        self.c().execute(
            "INSERT INTO notifications (proposal_id, channel, ok, error, sent_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![proposal_id, channel, ok as i64, error, sent_at],
        )?;
        Ok(())
    }

    /// The most recent notifications, newest first.
    pub fn notifications(&self, limit: usize) -> Result<Vec<Notification>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT id, proposal_id, channel, ok, error, sent_at FROM notifications ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit as i64], |r| {
            Ok(Notification {
                id: r.get(0)?,
                proposal_id: r.get(1)?,
                channel: r.get(2)?,
                ok: r.get::<_, i64>(3)? != 0,
                error: r.get(4)?,
                sent_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn put_proposal(c: &Connection, p: &Proposal) -> Result<()> {
    c.execute(
        "INSERT INTO proposals (id, body, status, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET body = excluded.body, status = excluded.status, updated_at = excluded.updated_at",
        params![
            p.id,
            serde_json::to_string(p)?,
            enum_str(&p.status)?,
            crate::now()
        ],
    )?;
    Ok(())
}
