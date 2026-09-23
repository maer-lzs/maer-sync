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
pub const SCHEMA: i64 = 3;

/// Unter welchem Namen das Geheimnis des Dienstes fuer das Schein-Salz steht.
pub const SALZ_ROW: &str = "salt_secret";

/// Der Grund, den ein zurueckgezogenes Zeichen traegt, woertlich wie im
/// Vertrag (`X-Maer-Refusal`).
pub const GRUND_PASSWORT_GEWECHSELT: &str = "password-changed";

/// Was eine Anmeldung ergibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anmeldung {
    /// Ein frisches Zeichen fuer dieses Geraet.
    Zeichen(String),
    /// Falsch, unbekannt, oder eine Form, die dieses Konto nicht mehr nimmt:
    /// EINE Antwort fuer alles.
    Abgewiesen,
    /// Das Passwort stimmt, aber der Betreiber hat es gesetzt: es taugt nur
    /// zum Wechsel, und kein Zeichen entsteht daraus.
    NurWechsel,
}

/// Womit sich ein Wechsel ausweist.
#[derive(Debug, Clone, Copy)]
pub enum Aktuell<'a> {
    /// Der Beweis des geltenden Passworts.
    Beweis(&'a [u8]),
    /// Ein Passwort, das der Dienst kennt: das alte eines noch nicht
    /// umgezogenen Kontos, oder das vom Betreiber gesetzte.
    Geheimnis(&'a str),
}

/// Was `/v1/salt` fuer einen Namen sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Salz {
    pub salt: Vec<u8>,
    pub rounds: u64,
    pub form: &'static str,
}

/// Der Zustand eines Kontos, soweit Fassung 6 ihn braucht.
#[derive(Debug, Clone)]
pub struct Konto {
    pub form: String,
    pub betreiber: bool,
    pub umgezogen: Option<f64>,
}

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
    /// Das Geheimnis des Schein-Salzes, einmal gelesen und behalten.
    salzgeheimnis: Option<Vec<u8>>,
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
        let s = Self { db, keep, shrink_floor, attrappe: None, salzgeheimnis: None };
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
        self.ein_geheimnis_auslegen()?;

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

    /// Schema 3: was Fassung 6 zusaetzlich haelt.
    ///
    /// **Angefuegt, nicht neu angelegt.** Die Tabellen darueber stehen
    /// woertlich wie in Fassung 5, und ein vorhandener Laden zieht um, indem
    /// ihm Spalten wachsen: kein Kopieren, kein Umbenennen, nichts, was einen
    /// Bestand in der Mitte anhalten koennte. Jede Spalte wird nur angefuegt,
    /// wenn sie fehlt, also darf das hier bei jedem Oeffnen laufen.
    ///
    /// - `accounts.form`: `secret` (der Dienst kennt das Passwort: Fassung 5,
    ///   oder vom Betreiber gesetzt) oder `proof` (umgezogen).
    /// - `accounts.salt`, `accounts.rounds`: das Salz und die Runden, mit denen
    ///   die Geraete dehnen. Nur fuer `proof`.
    /// - `accounts.operator`: 1, wenn der Betreiber das Passwort gesetzt hat.
    /// - `accounts.moved_at`: wann das Konto umgezogen ist.
    /// - `tokens.withdrawn`: der Grund, aus dem ein Zeichen zurueckgezogen ist,
    ///   oder leer. Ein zurueckgezogenes Zeichen bleibt stehen, damit die 401
    ///   ihren Grund sagen kann.
    /// - `sign_ins`: je Konto und Geraet die Form der letzten Anmeldung, die
    ///   Messung fuer Bedingung (b).
    fn ein_geheimnis_auslegen(&self) -> Result<(), StoreError> {
        let spalten = |tabelle: &str| -> Result<Vec<String>, StoreError> {
            let mut s = self.db.prepare(&format!("PRAGMA table_info({tabelle})"))?;
            let namen = s.query_map([], |r| r.get::<_, String>(1))?;
            Ok(namen.collect::<Result<Vec<_>, _>>()?)
        };
        let konten = spalten("accounts")?;
        for (name, art) in [
            ("form", "TEXT NOT NULL DEFAULT 'secret'"),
            ("salt", "TEXT"),
            ("rounds", "INTEGER"),
            ("operator", "INTEGER NOT NULL DEFAULT 0"),
            ("moved_at", "REAL"),
        ] {
            if !konten.iter().any(|s| s == name) {
                self.db.execute_batch(&format!("ALTER TABLE accounts ADD COLUMN {name} {art}"))?;
            }
        }
        if !spalten("tokens")?.iter().any(|s| s == "withdrawn") {
            self.db.execute_batch("ALTER TABLE tokens ADD COLUMN withdrawn TEXT")?;
        }
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS sign_ins (
                owner  TEXT NOT NULL,
                device TEXT NOT NULL DEFAULT '',
                form   TEXT NOT NULL,
                at     REAL NOT NULL,
                PRIMARY KEY (owner, device),
                FOREIGN KEY (owner) REFERENCES accounts(owner) ON DELETE CASCADE
            );")?;
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

    // ------------------------------------------------ Fuer die Rueckkehr-Probe

    /// Die Byte-Laenge jedes Dokuments, nach Kennung.
    ///
    /// Groessen, nicht Koerper: die Probe vergleicht das, wofuer ein
    /// Byte-Laden ehrlich buergen kann, und jeden Koerper dafuer im Speicher
    /// zu halten waere die eine Verrichtung, die mit dem Bestand waechst.
    pub fn sizes(&self) -> Result<std::collections::BTreeMap<String, i64>, StoreError> {
        let mut s = self.db.prepare(
            "SELECT owner, kind, scope, version, length(body) FROM documents")?;
        let zeilen = s.query_map([], |r| {
            Ok((format!("{}/{}/{}/{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?, r.get::<_, i64>(3)?),
                r.get::<_, i64>(4)?))
        })?;
        let mut aus = std::collections::BTreeMap::new();
        for z in zeilen {
            let (k, v) = z?;
            aus.insert(k, v);
        }
        Ok(aus)
    }

    /// Eine in sich stimmige Kopie des ganzen Ladens nach `pfad`.
    ///
    /// **`VACUUM INTO` statt der Sicherungs-Schnittstelle**, und der
    /// Unterschied ist benannt: die Python-Fassung nimmt `sqlite3.backup`,
    /// diese hier nimmt `VACUUM INTO`. Beide geben einen Schnappschuss und
    /// keine zerrissene Datei; die Kopie ist hier zusaetzlich verdichtet, also
    /// sind die BYTES der Datei andere. Fuer die Probe ist das gleichgueltig,
    /// weil sie Groessen JE DOKUMENT vergleicht und nicht die Datei. Die
    /// Alternative waere das Merkmal `backup` von rusqlite, und das ist eine
    /// Aufnahme.
    ///
    /// `VACUUM INTO` weigert sich, wenn die Zieldatei schon da ist. Das ist
    /// die richtige Richtung: eine Sicherung, die eine aeltere ueberschreibt,
    /// ist der Verlust, den sie verhindern soll.
    pub fn backup(&self, pfad: &str) -> Result<(), StoreError> {
        // **Erst daneben schreiben, dann darueberlegen.** `VACUUM INTO`
        // weigert sich, wenn das Ziel schon da ist, und das waere hier der
        // falsche Schutz: ein Betreiber, der taeglich auf DENSELBEN Pfad
        // sichert, bekaeme ab dem zweiten Tag einen Fehlschlag, waehrend die
        // Python-Fassung ueberschreibt. Gemessen an `vektoren/befehle.json`.
        //
        // Das Umbenennen ist dabei strenger als Python: dort schreibt die
        // Sicherungs-Schnittstelle in die Zieldatei selbst, ein Abbruch
        // mitten darin laesst also eine halbe Sicherung zurueck, wo vorher
        // eine ganze lag. Hier bleibt die alte unangetastet, bis die neue
        // vollstaendig ist.
        let daneben = format!("{pfad}.im-entstehen-{}", std::process::id());
        let _ = std::fs::remove_file(&daneben);
        self.db.execute("VACUUM INTO ?1", params![&daneben])
            .map_err(|e| {
                let _ = std::fs::remove_file(&daneben);
                // Der Betreiber hat nach dem ZIEL gefragt, nicht nach meinem
                // Arbeitsnamen daneben. Der taucht sonst in der Meldung auf,
                // samt Prozessnummer, und fuehrt ihn an eine Datei, die es
                // gar nicht mehr gibt.
                StoreError::Refused(e.to_string().replace(&daneben, pfad))
            })?;
        std::fs::rename(&daneben, pfad).map_err(|e| {
            let _ = std::fs::remove_file(&daneben);
            StoreError::Refused(format!("unable to put the copy in place: {e}"))
        })?;
        Ok(())
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
        Ok(match self.anmelden_mit_geheimnis(owner, secret, device)? {
            Anmeldung::Zeichen(z) => Some(z),
            _ => None,
        })
    }

    /// Die Anmeldung mit einem Passwort, das der Dienst kennt.
    ///
    /// Drei Faelle, und nur der dritte ist die Anmeldung von Fassung 5:
    ///
    /// - **Umgezogenes Konto:** abgewiesen, ohne das Geheimnis zu dehnen, zu
    ///   vergleichen oder zu loggen. Die Zeit wird trotzdem verbraucht, an einem
    ///   festen Wert, damit die Dauer nicht verraet, dass das Konto umgezogen
    ///   ist.
    /// - **Vom Betreiber gesetzt:** stimmt es, `NurWechsel` (428), und kein
    ///   Zeichen entsteht. Stimmt es nicht, dieselbe Abweisung wie immer.
    /// - **Noch nicht umgezogen:** wie bisher, mit Attrappe fuer unbekannte
    ///   Konten.
    pub fn anmelden_mit_geheimnis(&mut self, owner: &str, secret: &str, device: &str)
        -> Result<Anmeldung, StoreError>
    {
        let zeile: Option<(String, String, i64)> = self.db
            .query_row("SELECT secret, form, operator FROM accounts WHERE owner=?1",
                       params![owner], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        if let Some((_, form, _)) = &zeile {
            if form == "proof" {
                crate::authorization::zeit_verbrauchen();
                return Ok(Anmeldung::Abgewiesen);
            }
        }
        let gab_es = zeile.is_some();
        let betreiber = zeile.as_ref().map(|z| z.2 != 0).unwrap_or(false);
        let gespeichert = match zeile {
            Some((f, _, _)) => f,
            None => self.decoy()?,
        };
        // Beide Bedingungen, und die teure ZUERST: ein `if !gab_es` davor
        // waere genau der Zeitunterschied, den die Attrappe verhindern soll.
        let passt = crate::authorization::secret_matches(&gespeichert, secret);
        if !passt || !gab_es {
            return Ok(Anmeldung::Abgewiesen);
        }
        if betreiber {
            return Ok(Anmeldung::NurWechsel);
        }
        let zeichen = self.zeichen_ausgeben(owner, device, "secret")?;
        Ok(Anmeldung::Zeichen(zeichen))
    }

    /// Die Anmeldung von Fassung 6: ein Beweis, nie ein Passwort.
    ///
    /// Nur ein umgezogenes Konto nimmt einen Beweis an. Ein unbekanntes Konto
    /// und eines in der Form `secret` bekommen dieselbe Abweisung, nach
    /// derselben Arbeit an der Attrappe.
    pub fn anmelden_mit_beweis(&mut self, owner: &str, proof: &[u8], device: &str)
        -> Result<Anmeldung, StoreError>
    {
        let zeile: Option<(String, String)> = self.db
            .query_row("SELECT secret, form FROM accounts WHERE owner=?1",
                       params![owner], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let (gespeichert, gilt) = match zeile {
            Some((f, form)) if form == "proof" => (f, true),
            _ => (self.decoy()?, false),
        };
        let passt = crate::authorization::proof_matches(&gespeichert, proof);
        if !passt || !gilt {
            return Ok(Anmeldung::Abgewiesen);
        }
        let zeichen = self.zeichen_ausgeben(owner, device, "proof")?;
        Ok(Anmeldung::Zeichen(zeichen))
    }

    /// Das Passwort wechseln, und damit auch der Umzug eines Kontos.
    ///
    /// **Alles in EINEM Schritt** (`BEGIN IMMEDIATE`): pruefen, neuen
    /// Pruefwert und neues Salz setzen, die Marke des Betreibers loeschen,
    /// JEDES Zeichen des Kontos zurueckziehen und das eine neue ausgeben. Eine
    /// Unterbrechung dazwischen darf kein Konto mit neuem Passwort und alten
    /// Zeichen hinterlassen, und keines ohne jedes Zeichen.
    ///
    /// Welche Form `current` hat, entscheidet das KONTO, nicht der Rufer: ein
    /// Beweis nur fuer ein umgezogenes Konto, ein Geheimnis nur fuer eines in
    /// der Form `secret`. Das Geheimnis an ein umgezogenes Konto wird wie bei
    /// der Anmeldung nicht angefasst.
    pub fn passwort_wechseln(&mut self, owner: &str, device: &str, aktuell: Aktuell<'_>,
                             neu_salz: &[u8], neu_runden: u64, neu_beweis: &[u8])
        -> Result<Anmeldung, StoreError>
    {
        if neu_salz.len() != crate::authorization::KONTO_SALZ_BYTES
            || neu_runden < crate::authorization::MIN_ROUNDS
            || neu_beweis.len() != crate::authorization::PROOF_BYTES
        {
            return Err(StoreError::Refused("the new password is not readable".into()));
        }
        let attrappe = self.decoy()?;
        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let zeile: Option<(String, String, i64)> = tx
            .query_row("SELECT secret, form, operator FROM accounts WHERE owner=?1",
                       params![owner], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;

        let (passt, war) = match (&zeile, aktuell) {
            (Some((f, form, op)), Aktuell::Beweis(b)) if form == "proof" =>
                (crate::authorization::proof_matches(f, b), if *op != 0 { "reset" } else { "change" }),
            (Some((f, form, op)), Aktuell::Geheimnis(s)) if form == "secret" =>
                (crate::authorization::secret_matches(f, s), if *op != 0 { "reset" } else { "move" }),
            (Some((_, form, _)), Aktuell::Geheimnis(_)) if form == "proof" => {
                crate::authorization::zeit_verbrauchen();
                (false, "")
            }
            (_, Aktuell::Beweis(b)) => {
                let _ = crate::authorization::proof_matches(&attrappe, b);
                (false, "")
            }
            (_, Aktuell::Geheimnis(s)) => {
                let _ = crate::authorization::secret_matches(&attrappe, s);
                (false, "")
            }
        };
        if !passt || zeile.is_none() {
            tx.rollback()?;
            return Ok(Anmeldung::Abgewiesen);
        }

        let dienst_salz = crate::authorization::zufall(crate::authorization::SALT_BYTES)
            .map_err(StoreError::Refused)?;
        let form_neu = crate::authorization::hash_proof(neu_beweis, &dienst_salz)
            .map_err(StoreError::Refused)?;
        let jetzt_s = jetzt();
        tx.execute(
            "UPDATE accounts SET secret=?2, salt=?3, rounds=?4, form='proof', operator=0, \
             moved_at=COALESCE(moved_at, ?5) WHERE owner=?1",
            params![owner, form_neu, crate::authorization::hex(neu_salz), neu_runden as i64, jetzt_s])?;
        tx.execute(
            "UPDATE tokens SET withdrawn=?2 WHERE owner=?1 AND withdrawn IS NULL",
            params![owner, GRUND_PASSWORT_GEWECHSELT])?;
        let zeichen = crate::authorization::new_token().map_err(StoreError::Refused)?;
        tx.execute(
            "INSERT INTO tokens (digest, owner, device, made_at) VALUES (?1, ?2, ?3, ?4)",
            params![crate::authorization::token_digest(&zeichen), owner, device, jetzt_s])?;
        tx.execute(
            "INSERT INTO sign_ins (owner, device, form, at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(owner, device) DO UPDATE SET form=excluded.form, at=excluded.at",
            params![owner, device, war, jetzt_s])?;
        tx.commit()?;
        Ok(Anmeldung::Zeichen(zeichen))
    }

    /// Ein Einmal-Passwort des Betreibers setzen, oder nichts, wenn es das
    /// Konto nicht gibt.
    ///
    /// Das Konto faellt dabei in die Form `secret` zurueck, mit der Marke des
    /// Betreibers, und JEDES seiner Zeichen wird zurueckgezogen: wer das
    /// Passwort neu setzen muss, soll keine Geraete behalten, die noch mit dem
    /// alten angemeldet sind. Aus dem Einmal-Passwort entsteht nie ein Zeichen
    /// (428), nur ein Wechsel.
    pub fn zuruecksetzen(&mut self, owner: &str) -> Result<Option<String>, StoreError> {
        if owner.is_empty() {
            return Err(StoreError::Refused("an account has an owner".into()));
        }
        let einmal = crate::authorization::einmal_passwort().map_err(StoreError::Refused)?;
        let salz = crate::authorization::zufall(crate::authorization::SALT_BYTES)
            .map_err(StoreError::Refused)?;
        let form = crate::authorization::hash_secret(&einmal, &salz).map_err(StoreError::Refused)?;
        let tx = self.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let geaendert = tx.execute(
            "UPDATE accounts SET secret=?2, salt=NULL, rounds=NULL, form='secret', operator=1 \
             WHERE owner=?1", params![owner, form])?;
        if geaendert == 0 {
            tx.rollback()?;
            return Ok(None);
        }
        tx.execute(
            "UPDATE tokens SET withdrawn=?2 WHERE owner=?1 AND withdrawn IS NULL",
            params![owner, GRUND_PASSWORT_GEWECHSELT])?;
        tx.commit()?;
        Ok(Some(einmal))
    }

    /// Was `/v1/salt` fuer einen Namen sagt.
    ///
    /// Fuer ein umgezogenes Konto das echte Salz. Fuer jeden anderen Namen das
    /// Schein-Salz, und die Form sagt `secret` nur, wenn es ein Konto gibt,
    /// dessen Passwort der Dienst kennt: das ist die eine Unterscheidung, die
    /// der Vertrag zulaesst, solange sie gilt.
    pub fn salz_fuer(&mut self, owner: &str) -> Result<Salz, StoreError> {
        let zeile: Option<(String, Option<String>, Option<i64>)> = self.db
            .query_row("SELECT form, salt, rounds FROM accounts WHERE owner=?1",
                       params![owner], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        if let Some((form, Some(salz), Some(runden))) = &zeile {
            if form == "proof" {
                if let Some(roh) = crate::authorization::hex_lesen(salz) {
                    return Ok(Salz { salt: roh, rounds: *runden as u64, form: "proof" });
                }
            }
        }
        let geheimnis = self.salzgeheimnis()?;
        let schein = crate::authorization::schein_salz(&geheimnis, owner);
        let form = match &zeile {
            Some((f, _, _)) if f == "secret" => "secret",
            _ => "proof",
        };
        Ok(Salz { salt: schein, rounds: crate::authorization::MIN_ROUNDS, form })
    }

    /// Der Zustand eines Kontos, oder nichts.
    pub fn konto(&self, owner: &str) -> Result<Option<Konto>, StoreError> {
        Ok(self.db
            .query_row("SELECT form, operator, moved_at FROM accounts WHERE owner=?1",
                       params![owner], |r| Ok(Konto {
                           form: r.get(0)?, betreiber: r.get::<_, i64>(1)? != 0, umgezogen: r.get(2)?,
                       }))
            .optional()?)
    }

    /// Je Geraet die Form der letzten Anmeldung und ihre Zeit, aeltestes
    /// zuerst. Die Messung fuer Bedingung (b).
    pub fn geraete(&self, owner: &str) -> Result<Vec<(String, String, f64)>, StoreError> {
        let mut s = self.db.prepare(
            "SELECT device, form, at FROM sign_ins WHERE owner=?1 ORDER BY at ASC")?;
        let zeilen = s.query_map(params![owner], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(zeilen.collect::<Result<Vec<_>, _>>()?)
    }

    /// Ein Zeichen ausgeben und die Form der Anmeldung festhalten.
    fn zeichen_ausgeben(&mut self, owner: &str, device: &str, form: &str)
        -> Result<String, StoreError>
    {
        let zeichen = crate::authorization::new_token().map_err(StoreError::Refused)?;
        let jetzt_s = jetzt();
        self.db.execute(
            "INSERT INTO tokens (digest, owner, device, made_at) VALUES (?1, ?2, ?3, ?4)",
            params![crate::authorization::token_digest(&zeichen), owner, device, jetzt_s])?;
        self.db.execute(
            "INSERT INTO sign_ins (owner, device, form, at) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(owner, device) DO UPDATE SET form=excluded.form, at=excluded.at",
            params![owner, device, form, jetzt_s])?;
        Ok(zeichen)
    }

    /// Das Geheimnis des Schein-Salzes: einmal gezogen, im Laden gehalten,
    /// damit es jeden Neustart ueberlebt. Der Sitzungsschluessel taugt dafuer
    /// nicht, er lebt nur im Speicher; und der statische Schluessel hat eine
    /// Aufgabe, die er nicht mit einer zweiten teilen soll.
    fn salzgeheimnis(&mut self) -> Result<Vec<u8>, StoreError> {
        if let Some(g) = &self.salzgeheimnis { return Ok(g.clone()); }
        let da: Option<String> = self.db
            .query_row("SELECT value FROM meta WHERE key=?1", params![SALZ_ROW], |r| r.get(0))
            .optional()?;
        let roh = match da.as_deref().and_then(crate::authorization::hex_lesen) {
            Some(r) if r.len() == 32 => r,
            _ => {
                let neu = crate::authorization::zufall(32).map_err(StoreError::Refused)?;
                // `INSERT OR IGNORE` und danach LESEN: zwei Prozesse, die
                // gleichzeitig zum ersten Mal fragen, bekommen dasselbe.
                self.db.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
                                params![SALZ_ROW, crate::authorization::hex(&neu)])?;
                let gelesen: String = self.db.query_row(
                    "SELECT value FROM meta WHERE key=?1", params![SALZ_ROW], |r| r.get(0))?;
                crate::authorization::hex_lesen(&gelesen)
                    .filter(|r| r.len() == 32)
                    .ok_or_else(|| StoreError::Refused("the salt secret is unreadable".into()))?
            }
        };
        self.salzgeheimnis = Some(roh.clone());
        Ok(roh)
    }

    /// Wem ein Zeichen gehoert, solange es gilt. Ein zurueckgezogenes Zeichen
    /// gehoert niemandem mehr.
    pub fn owner_of_token(&self, digest: &str) -> Result<Option<String>, StoreError> {
        Ok(self.db
            .query_row("SELECT owner FROM tokens WHERE digest=?1 AND withdrawn IS NULL",
                       params![digest], |r| r.get(0))
            .optional()?)
    }

    /// Warum ein Zeichen zurueckgezogen ist, oder nichts (unbekannt oder gueltig).
    pub fn grund_des_rueckzugs(&self, digest: &str) -> Result<Option<String>, StoreError> {
        Ok(self.db
            .query_row("SELECT withdrawn FROM tokens WHERE digest=?1", params![digest],
                       |r| r.get::<_, Option<String>>(0))
            .optional()?
            .flatten())
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
