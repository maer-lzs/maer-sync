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

/// Unter welchem Namen der Abdruck des statischen Schluessels im Speicher steht.
pub const KEY_ROW: &str = "static_key";

/// Wie viele Fassungen eines Dokuments ueberleben.
pub const DEFAULT_KEEP: i64 = 10;

#[derive(Debug)]
pub enum StoreError {
    /// Der Speicher wurde von einem neueren Dienst geschrieben. Ein
    /// zurueckgerollter Dienst haelt hier an, statt rueckwaerts zu wandern.
    AheadOfService(String),
    Sql(rusqlite::Error),
    Gate(String),
    /// Der Laden weigert sich, und sagt warum. Kein Fehler des Aufrufers im
    /// Sinne eines Absturzes: eine Antwort.
    Refused(String),
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self { StoreError::Sql(e) }
}

pub struct Store {
    pub db: Connection,
    pub keep: i64,
    pub shrink_floor: Option<f64>,
    /// Die Attrappe, einmal gemacht und behalten. Siehe `decoy`.
    attrappe: Option<String>,
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
        let s = Self { db, keep, shrink_floor, attrappe: None };
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

    // ------------------------------------------------ Der gemerkte Schluessel

    /// Der Abdruck, den dieser Speicher erwartet, oder nichts.
    pub fn remembered_key(&self) -> Result<Option<String>, StoreError> {
        Ok(self.db
            .query_row("SELECT value FROM meta WHERE key=?1", params![KEY_ROW], |r| r.get(0))
            .optional()?)
    }

    /// Festhalten, zu wem dieser Speicher gehoert.
    ///
    /// Mit Absicht stumpf: es ueberschreibt. Der Schutz vor dem versehentlichen
    /// Ueberschreiben steht nicht hier, sondern in `identity::decide`, das rein
    /// und pruefbar ist.
    pub fn remember_key(&mut self, printed: &str) -> Result<(), StoreError> {
        self.db.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![KEY_ROW, printed])?;
        Ok(())
    }

    // ------------------------------------------------ Konten und Zeichen

    /// Ein Konto anlegen. Ueberschreibt nie eines: ein Geheimnis zu aendern
    /// ist ein eigener Akt mit eigener Sorgfalt, und ein versehentliches
    /// Neuanlegen darf niemanden still von seinen Daten aussperren.
    pub fn create_account(&mut self, owner: &str, secret: &str) -> Result<(), StoreError> {
        if owner.is_empty() {
            return Err(StoreError::Refused("an account has an owner".into()));
        }
        let da: Option<i64> = self.db
            .query_row("SELECT 1 FROM accounts WHERE owner=?1", params![owner], |r| r.get(0))
            .optional()?;
        if da.is_some() {
            return Err(StoreError::Refused(format!("an account named '{owner}' is already here")));
        }
        let salz = crate::authorization::zufall(crate::authorization::SALT_BYTES)
            .map_err(StoreError::Refused)?;
        let form = crate::authorization::hash_secret(secret, &salz).map_err(StoreError::Refused)?;
        self.db.execute("INSERT INTO accounts (owner, secret, made_at) VALUES (?1, ?2, ?3)",
                        params![owner, form, jetzt()])?;
        Ok(())
    }

    /// Ein Konto und alles darunter nehmen: erst die Dokumente, dann das Konto
    /// selbst (die Zeichen fallen ueber den Fremdschluessel). Die Dokumente
    /// gehen mit Absicht mit: wer kein Konto hat, kann kein Zeichen halten,
    /// also waere das unerreichbarer Speicher, und ein spaeteres Konto
    /// desselben Namens muss leer anfangen statt zu erben.
    pub fn remove_account(&mut self, owner: &str) -> Result<(usize, usize), StoreError> {
        if owner.is_empty() {
            return Err(StoreError::Refused("an account has an owner".into()));
        }
        // EIN Schritt: eine Unterbrechung dazwischen darf kein Konto ohne
        // seine Dokumente hinterlassen.
        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let dokumente = tx.execute("DELETE FROM documents WHERE owner=?1", params![owner])?;
        let konten = tx.execute("DELETE FROM accounts WHERE owner=?1", params![owner])?;
        tx.commit()?;
        Ok((konten, dokumente))
    }

    /// Ein Geraetezeichen gegen das Geheimnis des Kontos, oder nichts.
    ///
    /// Nichts bei falschem Geheimnis UND bei einem Konto, das es nicht gibt:
    /// eine Antwort fuer beides, sonst wird das hier eine Liste, wer ein Konto
    /// hat. Die Arbeit wird in beiden Faellen getan, damit die beiden sich
    /// auch in der DAUER nicht unterscheiden.
    pub fn issue_token(&mut self, owner: &str, secret: &str, device: &str)
        -> Result<Option<String>, StoreError>
    {
        let zeile: Option<String> = self.db
            .query_row("SELECT secret FROM accounts WHERE owner=?1", params![owner], |r| r.get(0))
            .optional()?;
        let gab_es = zeile.is_some();
        let gespeichert = match zeile {
            Some(f) => f,
            None => self.decoy()?,
        };
        // Beide Bedingungen, und die teure ZUERST: ein `if !gab_es` davor
        // waere genau der Zeitunterschied, den die Attrappe verhindern soll.
        let passt = crate::authorization::secret_matches(&gespeichert, secret);
        if !passt || !gab_es {
            return Ok(None);
        }
        let zeichen = crate::authorization::new_token().map_err(StoreError::Refused)?;
        self.db.execute(
            "INSERT INTO tokens (digest, owner, device, made_at) VALUES (?1, ?2, ?3, ?4)",
            params![crate::authorization::token_digest(&zeichen), owner, device, jetzt()])?;
        Ok(Some(zeichen))
    }

    pub fn owner_of_token(&self, digest: &str) -> Result<Option<String>, StoreError> {
        Ok(self.db
            .query_row("SELECT owner FROM tokens WHERE digest=?1", params![digest], |r| r.get(0))
            .optional()?)
    }

    pub fn revoke_token(&mut self, digest: &str) -> Result<(), StoreError> {
        self.db.execute("DELETE FROM tokens WHERE digest=?1", params![digest])?;
        Ok(())
    }

    /// Eine gespeicherte Form, auf die nichts passt, einmal gemacht und
    /// behalten, damit eine Anfrage nach einem unbekannten Konto dasselbe
    /// kostet wie eine nach einem echten.
    fn decoy(&mut self) -> Result<String, StoreError> {
        if self.attrappe.is_none() {
            let z = crate::authorization::new_token().map_err(StoreError::Refused)?;
            let salz = crate::authorization::zufall(crate::authorization::SALT_BYTES)
                .map_err(StoreError::Refused)?;
            self.attrappe = Some(
                crate::authorization::hash_secret(&z, &salz).map_err(StoreError::Refused)?);
        }
        Ok(self.attrappe.clone().unwrap())
    }
}

/// Die Wanduhr als Sekunden seit 1970, wie Pythons `time.time()`.
fn jetzt() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
