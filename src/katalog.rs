// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The catalog file `katalog.db` (K3a).
//!
//! The catalog index is built from public sources and can always be built
//! again, so it lives in a file of its own next to the user store: it is not
//! part of backup and restore of the user data, and it never grows them.
//!
//! **One writer, whole runs.** Only the fetch program writes, one run per
//! source at a time, in one transaction: either the whole run is in the file
//! or none of it. A run raises the version of its source by one when it
//! changes anything; every row it changes takes that version, and a row the
//! source no longer names stays as a deletion mark with that version, so a
//! device that asks for "everything after version v" learns about it.
//!
//! **"Failed" must never look like "removed".** A run with no rows, or one
//! that would delete more than a fifth of the live rows of a larger source,
//! is refused unless the caller gives an explicit sign that the source
//! really answered so. A fetch that broke off after its first page would
//! otherwise empty the index.
//!
//! The service only reads (`open_read`); a reader always sees whole runs,
//! because SQLite in WAL mode gives each read transaction one snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use rusqlite::{params, Connection, ErrorCode, OpenFlags, OptionalExtension};

/// The format number in `meta`; a file with another number is refused.
pub const KATALOG_FORMAT: i64 = 1;
/// The largest page a reader may ask for.
pub const MAX_PAGE: usize = 5000;
/// Below this many live rows a source has no bound on deletions per run.
pub const MASS_DELETE_MIN_ROWS: usize = 50;
/// A run may delete at most this share of the live rows (1 in 5) without
/// the caller's sign.
pub const MASS_DELETE_SHARE_DENOMINATOR: usize = 5;
/// How long a writer waits for another writer by default.
pub const DEFAULT_WAIT_MS: u64 = 5000;

/// The sources of the index. The table checks the same list.
pub const SOURCES: [&str; 9] = [
    "librivox", "archive-buch", "netlabel", "schellack", "otr", "gutenberg", "musopen", "radio", "hoerspiel-feed",
];
/// The kinds of a row. The table checks the same list.
pub const KINDS: [&str; 4] = ["hoerbuch", "hoerspiel", "musik", "sender"];

const SCHEMA: &str = "
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE stand (
  quelle  TEXT PRIMARY KEY,
  version INTEGER NOT NULL,
  voll_ab INTEGER NOT NULL
);
CREATE TABLE eintrag (
  quelle         TEXT NOT NULL CHECK (quelle IN ('librivox','archive-buch','netlabel',
                   'schellack','otr','gutenberg','musopen','radio','hoerspiel-feed')),
  id             TEXT NOT NULL,
  art            TEXT NOT NULL CHECK (art IN ('hoerbuch','hoerspiel','musik','sender')),
  titel          TEXT,
  urheber        TEXT,
  sprache        TEXT,
  jahr           INTEGER,
  rechte_kennung TEXT NOT NULL,
  rechte_url     TEXT,
  laden          INTEGER NOT NULL CHECK (laden IN (0, 1)),
  reihen         TEXT NOT NULL DEFAULT '[]',
  auch           TEXT NOT NULL DEFAULT '[]',
  tief           TEXT,
  version        INTEGER NOT NULL,
  geloescht      INTEGER NOT NULL DEFAULT 0 CHECK (geloescht IN (0, 1)),
  PRIMARY KEY (quelle, id)
) WITHOUT ROWID;
CREATE INDEX eintrag_delta ON eintrag (quelle, version);
CREATE TABLE cover (
  sha256 TEXT PRIMARY KEY,
  breite INTEGER,
  hoehe  INTEGER,
  bytes  INTEGER,
  typ    TEXT
);
";

/// One row as a source run delivers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub quelle: String,
    pub id: String,
    pub art: String,
    pub titel: Option<String>,
    pub urheber: Option<String>,
    pub sprache: Option<String>,
    pub jahr: Option<i64>,
    pub rechte_kennung: String,
    pub rechte_url: Option<String>,
    pub laden: bool,
    pub reihen: Vec<String>,
    pub auch: Vec<String>,
    /// The deep part as JSON text; `None` while the row is flat.
    pub tief: Option<String>,
}

/// A row as the file holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub row: Row,
    pub version: i64,
    pub geloescht: bool,
}

/// One page of a reading. `weiter` is the last id of the page, and `None`
/// exactly on the last page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub version: i64,
    pub zeilen: Vec<Stored>,
    pub weiter: Option<String>,
}

/// The answer to "everything after version v".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Since {
    Page(Page),
    /// Deletion marks below `floor` were pruned: the reader has to start
    /// from the full state.
    TooOld { floor: i64 },
    /// The reader names a version the file never handed out.
    Unknown { version: i64 },
}

#[derive(Debug)]
pub enum KatalogError {
    Sql(rusqlite::Error),
    /// `meta` names another format; the found value is named.
    UnknownFormat(String),
    /// The file is the user store, not a catalog.
    UserStore,
    /// A run without rows and without the caller's sign.
    EmptyRun,
    /// A run that would delete too many live rows without the caller's sign.
    MassDelete { would_delete: usize, live: usize },
    Invalid(String),
    PageSize(usize),
    /// Another writer holds the file longer than this writer waits.
    Busy,
}

impl std::fmt::Display for KatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KatalogError::Sql(e) => write!(f, "sqlite: {e}"),
            KatalogError::UnknownFormat(v) => write!(f, "catalog format {v} is not {KATALOG_FORMAT}"),
            KatalogError::UserStore => write!(f, "this file is the user store, not a catalog"),
            KatalogError::EmptyRun => write!(f, "a run without rows needs the caller's sign"),
            KatalogError::MassDelete { would_delete, live } =>
                write!(f, "the run would delete {would_delete} of {live} live rows"),
            KatalogError::Invalid(why) => write!(f, "invalid: {why}"),
            KatalogError::PageSize(n) => write!(f, "page size {n} is not within 1 to {MAX_PAGE}"),
            KatalogError::Busy => write!(f, "another writer holds the catalog"),
        }
    }
}

impl std::error::Error for KatalogError {}

impl From<rusqlite::Error> for KatalogError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == ErrorCode::DatabaseBusy || f.code == ErrorCode::DatabaseLocked => KatalogError::Busy,
            _ => KatalogError::Sql(e),
        }
    }
}

pub struct Katalog {
    db: Connection,
    read_only: bool,
}

fn has_table(db: &Connection, name: &str) -> Result<bool, KatalogError> {
    let n: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1", [name], |r| r.get(0))?;
    Ok(n > 0)
}

/// Refuse the user store and any other format before anything is written.
fn check_file(db: &Connection) -> Result<bool, KatalogError> {
    if has_table(db, "documents")? {
        return Err(KatalogError::UserStore);
    }
    if !has_table(db, "meta")? {
        return Ok(false);
    }
    let found: Option<String> = db
        .query_row("SELECT value FROM meta WHERE key='katalog_format'", [], |r| r.get(0))
        .optional()?;
    match found {
        Some(v) if v == KATALOG_FORMAT.to_string() => Ok(true),
        Some(v) => Err(KatalogError::UnknownFormat(v)),
        None => Err(KatalogError::UnknownFormat(String::new())),
    }
}

fn page_size(anzahl: usize) -> Result<(), KatalogError> {
    if anzahl == 0 || anzahl > MAX_PAGE { Err(KatalogError::PageSize(anzahl)) } else { Ok(()) }
}

fn list_json(v: &[String]) -> String {
    serde_json::to_string(v).expect("a list of strings always serialises")
}

fn list_from(text: &str) -> Result<Vec<String>, KatalogError> {
    serde_json::from_str(text).map_err(|e| KatalogError::Invalid(format!("stored list: {e}")))
}

/// The kinds of run whose time `meta` keeps.
pub const RUN_KINDS: [&str; 2] = ["voll", "delta"];

/// The checks every writer makes before its transaction: a known source,
/// rows of that source only, known kinds, ids present and once each.
fn checked<'a>(quelle: &str, rows: &'a [Row]) -> Result<BTreeSet<&'a str>, KatalogError> {
    if !SOURCES.contains(&quelle) {
        return Err(KatalogError::Invalid(format!("unknown source {quelle:?}")));
    }
    let mut seen = BTreeSet::new();
    for r in rows {
        if r.quelle != quelle {
            return Err(KatalogError::Invalid(format!("row {} names source {:?}", r.id, r.quelle)));
        }
        if !KINDS.contains(&r.art.as_str()) {
            return Err(KatalogError::Invalid(format!("row {} has kind {:?}", r.id, r.art)));
        }
        if r.id.is_empty() || !seen.insert(r.id.as_str()) {
            return Err(KatalogError::Invalid(format!("id {:?} is empty or twice in the run", r.id)));
        }
    }
    Ok(seen)
}

/// Raise the version of a source by one inside the writer's transaction.
fn raise(tx: &Connection, quelle: &str, version: i64, voll_ab: i64) -> Result<i64, KatalogError> {
    let next = version + 1;
    tx.execute(
        "INSERT INTO stand (quelle, version, voll_ab) VALUES (?1, ?2, ?3)
         ON CONFLICT(quelle) DO UPDATE SET version = excluded.version",
        params![quelle, next, voll_ab])?;
    Ok(next)
}

/// Write changed rows with a version; a row written here is live.
fn upsert(tx: &Connection, changed: &[&Row], version: i64) -> Result<(), KatalogError> {
    let mut up = tx.prepare(&format!(
        "INSERT INTO eintrag ({COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,0)
         ON CONFLICT(quelle, id) DO UPDATE SET
           art=excluded.art, titel=excluded.titel, urheber=excluded.urheber, sprache=excluded.sprache,
           jahr=excluded.jahr, rechte_kennung=excluded.rechte_kennung, rechte_url=excluded.rechte_url,
           laden=excluded.laden, reihen=excluded.reihen, auch=excluded.auch, tief=excluded.tief,
           version=excluded.version, geloescht=0"))?;
    for r in changed {
        up.execute(params![
            r.quelle, r.id, r.art, r.titel, r.urheber, r.sprache, r.jahr, r.rechte_kennung,
            r.rechte_url, r.laden as i64, list_json(&r.reihen), list_json(&r.auch), r.tief, version,
        ])?;
    }
    Ok(())
}

fn normalised(r: &Row) -> Result<Row, KatalogError> {
    let mut r = r.clone();
    for list in [&mut r.reihen, &mut r.auch] {
        list.sort();
        list.dedup();
    }
    if let Some(text) = &r.tief {
        let v: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| KatalogError::Invalid(format!("deep part of {} is not JSON: {e}", r.id)))?;
        r.tief = Some(v.to_string());
    }
    Ok(r)
}

const COLUMNS: &str =
    "quelle, id, art, titel, urheber, sprache, jahr, rechte_kennung, rechte_url, laden, reihen, auch, tief, version, geloescht";

fn stored_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<(Row, String, String, i64, bool)> {
    Ok((
        Row {
            quelle: r.get(0)?,
            id: r.get(1)?,
            art: r.get(2)?,
            titel: r.get(3)?,
            urheber: r.get(4)?,
            sprache: r.get(5)?,
            jahr: r.get(6)?,
            rechte_kennung: r.get(7)?,
            rechte_url: r.get(8)?,
            laden: r.get::<_, i64>(9)? == 1,
            reihen: vec![],
            auch: vec![],
            tief: r.get(12)?,
        },
        r.get(10)?,
        r.get(11)?,
        r.get(13)?,
        r.get::<_, i64>(14)? == 1,
    ))
}

fn read_rows(db: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<Stored>, KatalogError> {
    let mut s = db.prepare(sql)?;
    let raw = s.query_map(args, stored_from)?;
    let mut out = vec![];
    for item in raw {
        let (mut row, reihen, auch, version, geloescht) = item?;
        row.reihen = list_from(&reihen)?;
        row.auch = list_from(&auch)?;
        out.push(Stored { row, version, geloescht });
    }
    Ok(out)
}

/// The page of rows after `nach`, and whether another page follows: one
/// row more than asked is read and dropped, so a page that is exactly full
/// at the end still says it is the last.
fn paged(mut rows: Vec<Stored>, anzahl: usize, version: i64) -> Page {
    let more = rows.len() > anzahl;
    rows.truncate(anzahl);
    let weiter = if more { rows.last().map(|s| s.row.id.clone()) } else { None };
    Page { version, zeilen: rows, weiter }
}

fn stand_of(db: &Connection, quelle: &str) -> Result<(i64, i64), KatalogError> {
    Ok(db
        .query_row("SELECT version, voll_ab FROM stand WHERE quelle=?1", [quelle], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .unwrap_or((0, 0)))
}

impl Katalog {
    /// Open (and on first use lay down) the catalog for writing. Refuses
    /// the user store and any other format without writing to the file.
    pub fn open(path: &str) -> Result<Self, KatalogError> {
        let mut db = Connection::open(path)?;
        let laid_down = check_file(&db)?;
        db.busy_timeout(Duration::from_millis(DEFAULT_WAIT_MS))?;
        // WAL while the writer runs, at every open: a clean close switches to
        // DELETE (`close_clean`), so a reader without any write right finds
        // no side files it would have to create.
        db.pragma_update(None, "journal_mode", "WAL")?;
        if !laid_down {
            let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            // Another writer may have laid it down while this one waited.
            if !has_table(&tx, "meta")? {
                tx.execute_batch(SCHEMA)?;
                tx.execute("INSERT INTO meta (key, value) VALUES ('katalog_format', ?1)",
                           [KATALOG_FORMAT.to_string()])?;
            }
            tx.commit()?;
        }
        Ok(Katalog { db, read_only: false })
    }

    /// Open an existing catalog for reading only (the service).
    pub fn open_read(path: &str) -> Result<Self, KatalogError> {
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        if !check_file(&db)? {
            return Err(KatalogError::Invalid("no catalog in this file".into()));
        }
        db.busy_timeout(Duration::from_millis(DEFAULT_WAIT_MS))?;
        Ok(Katalog { db, read_only: true })
    }

    /// How long this writer waits for another one before `Busy`.
    pub fn set_wait_ms(&mut self, ms: u64) -> Result<(), KatalogError> {
        Ok(self.db.busy_timeout(Duration::from_millis(ms))?)
    }

    /// The highest version a source has handed out; 0 before its first run.
    pub fn version(&self, quelle: &str) -> Result<i64, KatalogError> {
        Ok(stand_of(&self.db, quelle)?.0)
    }

    /// Apply one whole run of a source and answer the version after it.
    /// `confirmed` is the caller's sign that an empty run or a large
    /// deletion is what the source really answered.
    ///
    /// The run holds the whole stock of its source in memory to compare it
    /// (up to some 190 000 rows for the shellac collection if it were read
    /// whole; read from its shelves it is some 30 000). That is meant for the
    /// fetch program, not the service; the memory of a full run is measured
    /// with the fetch program.
    ///
    /// Lists and the deep part are normalised before they are compared and
    /// stored: lists sorted and without doubles, the deep part as canonical
    /// JSON. Their order carries no meaning, and a run that only reorders
    /// them must not raise the version.
    pub fn write_run(&mut self, quelle: &str, rows: &[Row], confirmed: bool) -> Result<i64, KatalogError> {
        self.writable()?;
        let rows: Vec<Row> = rows.iter().map(normalised).collect::<Result<_, _>>()?;
        let seen = checked(quelle, &rows)?;
        if rows.is_empty() && !confirmed {
            return Err(KatalogError::EmptyRun);
        }

        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (version, voll_ab) = stand_of(&tx, quelle)?;
        let before: BTreeMap<String, Stored> = read_rows(
            &tx, &format!("SELECT {COLUMNS} FROM eintrag WHERE quelle=?1"), &[&quelle])?
            .into_iter().map(|s| (s.row.id.clone(), s)).collect();

        let live = before.values().filter(|s| !s.geloescht).count();
        let vanished: Vec<&str> = before.values()
            .filter(|s| !s.geloescht && !seen.contains(s.row.id.as_str()))
            .map(|s| s.row.id.as_str()).collect();
        if !confirmed && live >= MASS_DELETE_MIN_ROWS
            && vanished.len() * MASS_DELETE_SHARE_DENOMINATOR > live {
            return Err(KatalogError::MassDelete { would_delete: vanished.len(), live });
        }

        let changed: Vec<&Row> = rows.iter()
            .filter(|r| match before.get(&r.id) {
                Some(s) => s.geloescht || s.row != **r,
                None => true,
            })
            .collect();
        if changed.is_empty() && vanished.is_empty() {
            return Ok(version);
        }
        let next = raise(&tx, quelle, version, voll_ab)?;
        upsert(&tx, &changed, next)?;
        {
            let mut mark = tx.prepare("UPDATE eintrag SET geloescht=1, version=?3 WHERE quelle=?1 AND id=?2")?;
            for id in &vanished {
                mark.execute(params![quelle, id, next])?;
            }
        }
        tx.commit()?;
        Ok(next)
    }

    /// Apply the new and changed rows of a delta and answer the version
    /// after it. Unlike a run, a delta names only what is new: a row it does
    /// not name stays as it is and is never marked deleted, and a deletion
    /// mark it names again lives again. An empty delta is no change, not an
    /// empty run; the version rises only on a change.
    pub fn write_delta(&mut self, quelle: &str, rows: &[Row]) -> Result<i64, KatalogError> {
        self.writable()?;
        let rows: Vec<Row> = rows.iter().map(normalised).collect::<Result<_, _>>()?;
        checked(quelle, &rows)?;
        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (version, voll_ab) = stand_of(&tx, quelle)?;
        let mut changed: Vec<&Row> = vec![];
        for r in &rows {
            let found = read_rows(
                &tx, &format!("SELECT {COLUMNS} FROM eintrag WHERE quelle=?1 AND id=?2"), &[&quelle as &dyn rusqlite::ToSql, &r.id])?;
            if !matches!(found.first(), Some(s) if !s.geloescht && s.row == *r) {
                changed.push(r);
            }
        }
        if changed.is_empty() {
            return Ok(version);
        }
        let next = raise(&tx, quelle, version, voll_ab)?;
        upsert(&tx, &changed, next)?;
        tx.commit()?;
        Ok(next)
    }

    /// Keep the time of the last successful run of a kind (`voll` or
    /// `delta`) for a source, so a restart does not fetch everything again.
    pub fn set_last_run(&mut self, quelle: &str, art: &str, unix_s: i64) -> Result<(), KatalogError> {
        self.writable()?;
        if !SOURCES.contains(&quelle) || !RUN_KINDS.contains(&art) {
            return Err(KatalogError::Invalid(format!("no run time for {quelle:?} {art:?}")));
        }
        self.db.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![format!("lauf:{quelle}:{art}"), unix_s.to_string()])?;
        Ok(())
    }

    /// The time of the last successful run of a kind, if there was one.
    pub fn last_run(&self, quelle: &str, art: &str) -> Result<Option<i64>, KatalogError> {
        let v: Option<String> = self.db
            .query_row("SELECT value FROM meta WHERE key=?1", [format!("lauf:{quelle}:{art}")], |r| r.get(0))
            .optional()?;
        match v {
            None => Ok(None),
            Some(s) => s.parse().map(Some)
                .map_err(|_| KatalogError::Invalid(format!("run time {s:?} of {quelle} {art} is not a number"))),
        }
    }

    /// Close as the fetch program does at a clean end: switch the file to
    /// `journal_mode=DELETE`, so no `-wal` and `-shm` are left and the
    /// service, which mounts the catalog read only and cannot create them,
    /// still opens it. Measured 25.11.2026 (K3a-4): after a crash the side
    /// files stay and a read-only reader still reads the last committed
    /// state; the next writer open cleans up. A reader just drops.
    pub fn close_clean(self) -> Result<(), KatalogError> {
        if !self.read_only {
            let mode: String = self.db.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))?;
            if !mode.eq_ignore_ascii_case("delete") {
                return Err(KatalogError::Invalid(format!("journal_mode stayed {mode}")));
            }
        }
        Ok(())
    }

    fn writable(&self) -> Result<(), KatalogError> {
        if self.read_only { Err(KatalogError::Invalid("opened for reading".into())) } else { Ok(()) }
    }

    /// One row as the file holds it, deletion marks included.
    pub fn stored(&self, quelle: &str, id: &str) -> Result<Option<Stored>, KatalogError> {
        Ok(read_rows(&self.db, &format!("SELECT {COLUMNS} FROM eintrag WHERE quelle=?1 AND id=?2"),
                     &[&quelle as &dyn rusqlite::ToSql, &id])?.into_iter().next())
    }

    /// The live rows of a source in id order, one page after `nach`.
    pub fn full(&self, quelle: &str, nach: Option<&str>, anzahl: usize) -> Result<Page, KatalogError> {
        page_size(anzahl)?;
        let tx = self.db.unchecked_transaction()?;
        let (version, _) = stand_of(&tx, quelle)?;
        let rows = read_rows(&tx, &format!(
            "SELECT {COLUMNS} FROM eintrag WHERE quelle=?1 AND geloescht=0 AND (?2 IS NULL OR id > ?2)
             ORDER BY id LIMIT ?3"), &[&quelle, &nach, &((anzahl + 1) as i64)])?;
        tx.finish()?;
        Ok(paged(rows, anzahl, version))
    }

    /// Every row of a source with a version above `v`, deletion marks
    /// included, in id order, one page after `nach`.
    pub fn since(&self, quelle: &str, v: i64, nach: Option<&str>, anzahl: usize) -> Result<Since, KatalogError> {
        page_size(anzahl)?;
        let tx = self.db.unchecked_transaction()?;
        let (version, voll_ab) = stand_of(&tx, quelle)?;
        if v > version {
            return Ok(Since::Unknown { version });
        }
        if v < voll_ab {
            return Ok(Since::TooOld { floor: voll_ab });
        }
        let rows = read_rows(&tx, &format!(
            "SELECT {COLUMNS} FROM eintrag WHERE quelle=?1 AND version > ?2 AND (?3 IS NULL OR id > ?3)
             ORDER BY id LIMIT ?4"), &[&quelle, &v, &nach, &((anzahl + 1) as i64)])?;
        tx.finish()?;
        Ok(Since::Page(paged(rows, anzahl, version)))
    }

    /// Remove the deletion marks of a source up to and including version
    /// `bound` and raise its floor to `bound`: a reader at a version below
    /// it may have missed a removed mark and must start from the full
    /// state. Live rows are never removed. Answers how many marks went.
    pub fn prune(&mut self, quelle: &str, bound: i64) -> Result<usize, KatalogError> {
        if self.read_only {
            return Err(KatalogError::Invalid("opened for reading".into()));
        }
        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (version, voll_ab) = stand_of(&tx, quelle)?;
        if bound > version || bound < 0 {
            return Err(KatalogError::Invalid(format!("bound {bound} is not within 0 to {version}")));
        }
        let gone = tx.execute(
            "DELETE FROM eintrag WHERE quelle=?1 AND geloescht=1 AND version <= ?2", params![quelle, bound])?;
        tx.execute("UPDATE stand SET voll_ab=?2 WHERE quelle=?1", params![quelle, bound.max(voll_ab)])?;
        tx.commit()?;
        Ok(gone)
    }
}
