// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The catalog file `katalog.db` (K3a). Signatures only: the witnesses in
//! `tests/katalog_db.rs` are written first and must fail on behaviour.

/// The format number in `meta`; a file with another number is refused.
pub const KATALOG_FORMAT: i64 = 1;
/// The largest page a reader may ask for.
pub const MAX_PAGE: usize = 5000;
/// Below this many live rows a source has no bound on deletions per run.
pub const MASS_DELETE_MIN_ROWS: usize = 50;

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
    pub tief: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub row: Row,
    pub version: i64,
    pub geloescht: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub version: i64,
    pub zeilen: Vec<Stored>,
    pub weiter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Since {
    Page(Page),
    TooOld { floor: i64 },
    Unknown { version: i64 },
}

#[derive(Debug)]
pub enum KatalogError {
    Sql(rusqlite::Error),
    UnknownFormat(String),
    UserStore,
    EmptyRun,
    MassDelete { would_delete: usize, live: usize },
    Invalid(String),
    PageSize(usize),
    Busy,
}

pub struct Katalog;

impl Katalog {
    pub fn open(_path: &str) -> Result<Self, KatalogError> { todo!("K3a") }
    pub fn open_read(_path: &str) -> Result<Self, KatalogError> { todo!("K3a") }
    pub fn set_wait_ms(&mut self, _ms: u64) { todo!("K3a") }
    pub fn version(&self, _quelle: &str) -> Result<i64, KatalogError> { todo!("K3a") }
    pub fn write_run(&mut self, _quelle: &str, _rows: &[Row], _confirmed: bool) -> Result<i64, KatalogError> { todo!("K3a") }
    pub fn full(&self, _quelle: &str, _nach: Option<&str>, _anzahl: usize) -> Result<Page, KatalogError> { todo!("K3a") }
    pub fn since(&self, _quelle: &str, _v: i64, _nach: Option<&str>, _anzahl: usize) -> Result<Since, KatalogError> { todo!("K3a") }
    pub fn prune(&mut self, _quelle: &str, _bound: i64) -> Result<usize, KatalogError> { todo!("K3a") }
}
