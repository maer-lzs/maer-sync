// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Speicher: dieselben Tabellen, dieselbe Datei.
//!
//! Invariante 4 des Auftrags verlangt, dass der Bestand ohne Verlust umzieht,
//! und der billigste Weg dahin ist, ihn gar nicht umzuziehen: dieselbe
//! SQLite-Datei, dieselben Tabellen, dieselben Spaltennamen. Deshalb steht
//! hier die DDL der Python-Fassung woertlich, und deshalb ist `bundled`
//! entschieden worden, also echtes SQLite als C und keine Nachbildung.
//!
//! Zwei Dinge sind keine Bequemlichkeit, sondern die Sache selbst:
//!
//! - **Die Fassungsnummer eines Dokuments IST sein Etikett.** Sie ist je
//!   Dokument schon eindeutig und schon geordnet; einen Hash daneben zu
//!   erfinden waere eine zweite Wahrheit, die man in Schritt halten muss.
//! - **Lesen und Schreiben eines Schriebs sind EIN Schritt** (`BEGIN
//!   IMMEDIATE`). Sonst ist das Vergleichen-und-Setzen eine Geschichte, die
//!   wir erzaehlen, und zwei Geraete schreiben sich gegenseitig zu.

use rusqlite::{params, Connection, OptionalExtension};

use crate::formats::serve_verdict;
use crate::versions::rotate;
use crate::writes::{decide, Write, WriteDecision};

/// Die Gestalt DIESER Tabellen. Wird erhoeht, wenn die Tabellen sich aendern,
/// nie wenn sich das Format eines Dokuments aendert: zwei Nummern mit zwei
/// Aufgaben.
pub const SCHEMA: i64 = 2;

/// Wie viele Fassungen eines Dokuments ueberleben.
pub const DEFAULT_KEEP: i64 = 10;

#[derive(Debug)]
pub enum StoreError {
    /// Der Speicher wurde von einem neueren Dienst geschrieben. Ein
    /// zurueckgerollter Dienst haelt hier an, statt rueckwaerts zu wandern.
    AheadOfService(String),
    Sql(rusqlite::Error),
    Gate(String),
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self { StoreError::Sql(e) }
}

pub struct Store {
    pub db: Connection,
    pub keep: i64,
    pub shrink_floor: Option<f64>,
}

/// Der neueste Stand eines Dokuments: Koerper, Format, Etikett.
pub struct Held {
    pub body: Vec<u8>,
    pub format: i64,
    pub etag: String,
}

impl Store {
    pub fn open(path: &str, keep: i64, shrink_floor: Option<f64>) -> Result<Self, StoreError> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        let s = Self { db, keep, shrink_floor };
        s.lay_out()?;
        Ok(s)
    }

    fn lay_out(&self) -> Result<(), StoreError> {
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)")?;
        let found: Option<i64> = self.db
            .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| {
                let s: String = r.get(0)?;
                Ok(s.parse::<i64>().unwrap_or(0))
            })
            .optional()?;

        let verdict = serve_verdict(found, Some(SCHEMA)).map_err(StoreError::Gate)?;
        if !verdict.accepted {
            return Err(StoreError::AheadOfService(verdict.reason));
        }

        // Woertlich die Tabellen der Python-Fassung. Wer hier eine Spalte
        // anders nennt, macht aus derselben Datei zwei.
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                owner      TEXT    NOT NULL,
                kind       TEXT    NOT NULL,
                scope      TEXT    NOT NULL DEFAULT '',
                version    INTEGER NOT NULL,
                body       BLOB    NOT NULL,
                format     INTEGER NOT NULL,
                written_at REAL    NOT NULL,
                PRIMARY KEY (owner, kind, scope, version)
            );
            CREATE TABLE IF NOT EXISTS accounts (
                owner  TEXT PRIMARY KEY,
                secret TEXT NOT NULL,
                made_at REAL NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tokens (
                digest  TEXT PRIMARY KEY,
                owner   TEXT NOT NULL,
                device  TEXT NOT NULL DEFAULT '',
                made_at REAL NOT NULL,
                FOREIGN KEY (owner) REFERENCES accounts(owner) ON DELETE CASCADE
            );")?;

        match found {
            None => {
                self.db.execute("INSERT INTO meta (key, value) VALUES ('schema', ?1)",
                                params![SCHEMA.to_string()])?;
            }
            Some(f) if f < SCHEMA => {
                // Die Tabellen oben entstehen, wenn sie fehlen, also liegt ein
                // aelterer Speicher hier schon in der neuen Gestalt. Es
                // aufzuschreiben ist das, was den NAECHSTEN Rueckweg merken
                // laesst.
                self.db.execute("UPDATE meta SET value=?1 WHERE key='schema'",
                                params![SCHEMA.to_string()])?;
            }
            Some(_) => {}
        }
        Ok(())
    }

    /// Der neueste Stand, oder nichts.
    pub fn current(&self, owner: &str, kind: &str, scope: &str) -> Result<Option<Held>, StoreError> {
        let held = self.db.query_row(
            "SELECT body, format, version FROM documents \
             WHERE owner=?1 AND kind=?2 AND scope=?3 ORDER BY version DESC LIMIT 1",
            params![owner, kind, scope],
            |r| Ok(Held { body: r.get(0)?, format: r.get(1)?, etag: r.get::<_, i64>(2)?.to_string() }),
        ).optional()?;
        Ok(held)
    }

    /// Jede gespeicherte Fassung, aeltestes zuerst: das Rueckweg-Material.
    pub fn versions(&self, owner: &str, kind: &str, scope: &str) -> Result<Vec<i64>, StoreError> {
        let mut s = self.db.prepare(
            "SELECT version FROM documents WHERE owner=?1 AND kind=?2 AND scope=?3 \
             ORDER BY version ASC")?;
        let rows = s.query_map(params![owner, kind, scope], |r| r.get(0))?;
        Ok(rows.collect::<Result<Vec<i64>, _>>()?)
    }

    /// Jeder Bereich, der ein Dokument dieser Sorte haelt: Name, Etikett,
    /// Format. Namen und Fassungen, nie Koerper.
    pub fn scopes(&self, owner: &str, kind: &str) -> Result<Vec<(String, String, i64)>, StoreError> {
        let mut s = self.db.prepare(
            "SELECT scope, version, format FROM documents AS d \
             WHERE owner=?1 AND kind=?2 AND version = ( \
               SELECT MAX(version) FROM documents \
               WHERE owner=d.owner AND kind=d.kind AND scope=d.scope) \
             ORDER BY scope")?;
        let rows = s.query_map(params![owner, kind], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?.to_string(), r.get::<_, i64>(2)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Schreibt, wenn die Tore es zulassen. Gibt das Urteil und, wenn
    /// geschrieben wurde, das neue Etikett.
    ///
    /// Alles darin passiert unter `BEGIN IMMEDIATE`: das Lesen, gegen das
    /// entschieden wird, und das Einfuegen danach sind EIN Schritt.
    pub fn write(
        &mut self,
        owner: &str,
        kind: &str,
        body: &[u8],
        writer_format: i64,
        scope: &str,
        if_match: Option<&str>,
        declared_shrink: bool,
    ) -> Result<(WriteDecision, Option<String>), StoreError> {
        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let held = tx.query_row(
            "SELECT body, format, version FROM documents \
             WHERE owner=?1 AND kind=?2 AND scope=?3 ORDER BY version DESC LIMIT 1",
            params![owner, kind, scope],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
        ).optional()?;

        let (stored_bytes, stored_format, stored_etag) = match &held {
            Some((b, f, v)) => (Some(b.len() as i64), Some(*f), Some(v.to_string())),
            None => (None, None, None),
        };

        let w = Write {
            stored_bytes,
            stored_format,
            stored_etag: stored_etag.as_deref(),
            incoming_bytes: body.len() as i64,
            writer_format: Some(writer_format),
            if_match,
            declared_shrink,
            shrink_floor: self.shrink_floor.unwrap_or(crate::admission::DEFAULT_SHRINK_FLOOR),
        };
        let decision = decide(&w).map_err(StoreError::Gate)?;
        if !decision.accepted() {
            // Kein Schrieb, also auch keine Spur: die Abweisung rollt zurueck.
            tx.rollback()?;
            return Ok((decision, None));
        }

        let version: i64 = stored_etag.as_deref()
            .and_then(|e| e.parse::<i64>().ok())
            .map(|v| v + 1)
            .unwrap_or(1);
        let jetzt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        tx.execute(
            "INSERT INTO documents (owner, kind, scope, version, body, format, written_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![owner, kind, scope, version, body, writer_format, jetzt],
        )?;

        let alle: Vec<i64> = {
            let mut s = tx.prepare(
                "SELECT version FROM documents WHERE owner=?1 AND kind=?2 AND scope=?3 \
                 ORDER BY version ASC")?;
            let rows = s.query_map(params![owner, kind, scope], |r| r.get(0))?;
            rows.collect::<Result<Vec<i64>, _>>()?
        };
        let als_text: Vec<String> = alle.iter().map(|v| v.to_string()).collect();
        let (_, dropped) = rotate(&als_text, self.keep).map_err(StoreError::Gate)?;
        for alt in dropped {
            tx.execute(
                "DELETE FROM documents WHERE owner=?1 AND kind=?2 AND scope=?3 AND version=?4",
                params![owner, kind, scope, alt.parse::<i64>().unwrap_or(-1)],
            )?;
        }

        tx.commit()?;
        Ok((decision, Some(version.to_string())))
    }
}
