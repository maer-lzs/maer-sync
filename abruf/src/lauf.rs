// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! One run of one archive.org source (K3c).
//!
//! **Full pass:** first the number of records over the advanced search
//! (`rows=0`, echo of the query checked), then the scrape pages with
//! `count=10000` over the cursor. Written only when the pages carry exactly
//! that number of records, each once; a short, doubled or foreign pass
//! writes nothing. Measured 25.11.2026: with `count=100` the scrape API can
//! answer with the cached answer of another query, with `count=10000` it did
//! not, and LibriVox came complete in three pages.
//!
//! **Delta:** the advanced search for records added since the last run of
//! either kind minus a day (the day of overlap is deliberate), paged by
//! `rows` and `page`, the echo of the query checked on every page, written
//! through `write_delta`, which never deletes. The delta sees only added
//! records; changed rights and deletions come with the weekly full pass.
//!
//! **Shellac** is read from the nine theme shelves of the app only, each
//! over the advanced search, never the whole collection; a record on two
//! shelves becomes one row with both.
//!
//! A failed run writes nothing and does not set its run time, so the next
//! start begins it again. Every run leaves one line (`Laufbericht::zeile`)
//! without any answer text.

use std::collections::BTreeSet;

use maer_sync::katalog::{Katalog, Row};
use serde_json::Value;

use crate::archive::{merge_shelves, rows_from_answer, ReadError, Shelf};
use crate::netz::{encode, HolFehler, Holer, Netz, Uhr};
use crate::plan::{Art, DELTA_S};

const ADV: &str = "https://archive.org/advancedsearch.php";
const SCRAPE: &str = "https://archive.org/services/search/v1/scrape";
const FIELDS: [&str; 11] = [
    "identifier", "title", "creator", "date", "year", "licenseurl", "addeddate", "publicdate", "collection",
    "language", "subject",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Einstellungen {
    pub scrape_count: usize,
    pub adv_rows: usize,
    pub kalenderjahr: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaufFehler {
    Holen(HolFehler),
    Lesen(ReadError),
    Zaehlung { erwartet: usize, gelesen: usize, eindeutig: usize },
    Echo { gestellt: String, zurueck: Option<String> },
    Katalog(String),
    Quelle(String),
}

impl From<HolFehler> for LaufFehler {
    fn from(e: HolFehler) -> Self { LaufFehler::Holen(e) }
}

impl From<ReadError> for LaufFehler {
    fn from(e: ReadError) -> Self { LaufFehler::Lesen(e) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laufbericht {
    pub quelle: String,
    pub art: Art,
    pub gelesen: usize,
    pub version_vorher: i64,
    pub version_nachher: i64,
    pub dauer_ms: i64,
    pub ausgang: Result<(), LaufFehler>,
}

impl Laufbericht {
    /// One line for the log: source, kind, records read, versions, time and
    /// outcome with its reason. Never any text of an answer.
    pub fn zeile(&self) -> String {
        let ausgang = match &self.ausgang {
            Ok(()) => "ok".to_string(),
            Err(e) => format!("abgewiesen grund={}", format!("{e:?}").replace(['\n', '\r'], " ")),
        };
        format!("lauf quelle={} art={} gelesen={} version={}->{} dauer_ms={} ausgang={ausgang}",
                self.quelle, self.art.name(), self.gelesen, self.version_vorher, self.version_nachher, self.dauer_ms)
    }
}

/// The query of a source of the index; shellac has none (shelves only).
pub fn base_query(quelle: &str) -> Option<&'static str> {
    match quelle {
        "librivox" => Some("collection:librivoxaudio"),
        "archive-buch" => Some("collection:audio_bookspoetry AND mediatype:(audio)"),
        "netlabel" => Some("collection:netlabels AND mediatype:(audio)"),
        "otr" => Some("collection:oldtimeradio AND mediatype:(audio)"),
        _ => None,
    }
}

/// The theme shelves of the app, from the shared list the vectors carry.
pub fn regale_der_app() -> Vec<Shelf> {
    let v: Value = serde_json::from_str(include_str!("../../vektoren/katalog/schellack/regale.json"))
        .expect("regale.json is JSON");
    v["regale"].as_array().expect("regale.json lists shelves").iter()
        .map(|r| Shelf {
            schluessel: r["schluessel"].as_str().expect("a key").into(),
            schlagwort: r["schlagwort"].as_str().expect("a word").into(),
        })
        .collect()
}

fn shelf_query(s: &Shelf) -> String {
    format!("collection:georgeblood AND subject:\"{}\" AND mediatype:(audio)", s.schlagwort)
}

/// A UTC day as `YYYY-MM-DD` (civil from days, without a calendar crate).
pub fn datum(unix_s: i64) -> String {
    let z = unix_s.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn adv_url(q: &str, rows: usize, page: usize) -> String {
    let mut u = format!("{ADV}?q={}&rows={rows}&page={page}&output=json", encode(q));
    let fields: &[&str] = if rows == 0 { &["identifier"] } else { &FIELDS };
    for f in fields {
        u.push_str(&format!("&{}={}", encode("fl[]"), encode(f)));
    }
    u
}

fn scrape_url(q: &str, count: usize, cursor: Option<&str>) -> String {
    let mut u = format!("{SCRAPE}?q={}&fields={}&count={count}", encode(q), encode(&FIELDS.join(",")));
    if let Some(c) = cursor {
        u.push_str(&format!("&cursor={}", encode(c)));
    }
    u
}

fn parse(body: &[u8]) -> Result<Value, LaufFehler> {
    serde_json::from_slice(body).map_err(|e| LaufFehler::Lesen(ReadError::Unreadable(e.to_string())))
}

/// An advanced search page, its echo checked: (numFound, the answer bytes).
fn adv_page<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, q: &str, rows: usize, page: usize)
    -> Result<(usize, Vec<u8>), LaufFehler> {
    let a = netz.get(&adv_url(q, rows, page))?;
    let v = parse(&a.koerper)?;
    let echo = v.pointer("/responseHeader/params/query").and_then(Value::as_str).map(str::to_string);
    if echo.as_deref() != Some(q) {
        return Err(LaufFehler::Echo { gestellt: q.into(), zurueck: echo });
    }
    let n = v.pointer("/response/numFound").and_then(Value::as_u64)
        .ok_or_else(|| LaufFehler::Lesen(ReadError::Unreadable("no numFound".into())))?;
    Ok((n as usize, a.koerper))
}

/// Every record of an advanced search query, page by page, with the count.
fn adv_all<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, quelle: &str, q: &str, e: &Einstellungen, shelf: Option<&Shelf>)
    -> Result<Vec<Row>, LaufFehler> {
    let mut rows = vec![];
    let mut page = 1;
    loop {
        let (n, body) = adv_page(netz, q, e.adv_rows, page)?;
        let got = rows_from_answer(quelle, &body, e.kalenderjahr, shelf)?;
        let empty = got.is_empty();
        rows.extend(got);
        if rows.len() >= n || empty {
            check_count(&rows, n)?;
            return Ok(rows);
        }
        page += 1;
    }
}

fn check_count(rows: &[Row], erwartet: usize) -> Result<(), LaufFehler> {
    let eindeutig = rows.iter().map(|r| r.id.as_str()).collect::<BTreeSet<_>>().len();
    if rows.len() != erwartet || eindeutig != erwartet {
        return Err(LaufFehler::Zaehlung { erwartet, gelesen: rows.len(), eindeutig });
    }
    Ok(())
}

fn full_pass<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, quelle: &str, e: &Einstellungen) -> Result<Vec<Row>, LaufFehler> {
    if quelle == "schellack" {
        let mut parts = vec![];
        for s in regale_der_app() {
            parts.push(adv_all(netz, quelle, &shelf_query(&s), e, Some(&s))?);
        }
        return Ok(merge_shelves(parts));
    }
    let q = base_query(quelle).ok_or_else(|| LaufFehler::Quelle(quelle.into()))?;
    let (n, _) = adv_page(netz, q, 0, 1)?;
    let mut rows = vec![];
    let mut cursor: Option<String> = None;
    loop {
        let a = netz.get(&scrape_url(q, e.scrape_count, cursor.as_deref()))?;
        rows.extend(rows_from_answer(quelle, &a.koerper, e.kalenderjahr, None)?);
        cursor = parse(&a.koerper)?.get("cursor").and_then(Value::as_str).map(str::to_string);
        if cursor.is_none() {
            break;
        }
        if rows.len() > n {
            break; // more than counted: the check below refuses it
        }
    }
    check_count(&rows, n)?;
    Ok(rows)
}

fn delta<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, katalog: &Katalog, quelle: &str, e: &Einstellungen)
    -> Result<Vec<Row>, LaufFehler> {
    let k = |e: maer_sync::katalog::KatalogError| LaufFehler::Katalog(e.to_string());
    let last = [katalog.last_run(quelle, "voll").map_err(k)?, katalog.last_run(quelle, "delta").map_err(k)?]
        .into_iter().flatten().max()
        .ok_or_else(|| LaufFehler::Quelle(format!("{quelle}: a delta without a full pass")))?;
    let since = format!(" AND addeddate:[{} TO null]", datum(last - DELTA_S));
    if quelle == "schellack" {
        let mut parts = vec![];
        for s in regale_der_app() {
            parts.push(adv_all(netz, quelle, &format!("{}{since}", shelf_query(&s)), e, Some(&s))?);
        }
        return Ok(merge_shelves(parts));
    }
    let q = base_query(quelle).ok_or_else(|| LaufFehler::Quelle(quelle.into()))?;
    adv_all(netz, quelle, &format!("{q}{since}"), e, None)
}

/// Run one source once and report it. Nothing is written unless the whole
/// run succeeded.
pub fn lauf<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, katalog: &mut Katalog, quelle: &str, art: Art,
                              e: &Einstellungen) -> Laufbericht {
    let start = netz.uhr.jetzt_ms();
    let vorher = katalog.version(quelle).unwrap_or(-1);
    let mut gelesen = 0;
    let ausgang = (|| -> Result<(), LaufFehler> {
        let rows = match art {
            Art::Voll => full_pass(netz, quelle, e)?,
            Art::Delta => delta(netz, katalog, quelle, e)?,
        };
        gelesen = rows.len();
        let k = |e: maer_sync::katalog::KatalogError| LaufFehler::Katalog(e.to_string());
        match art {
            Art::Voll => katalog.write_run(quelle, &rows, false).map_err(k)?,
            Art::Delta => katalog.write_delta(quelle, &rows).map_err(k)?,
        };
        katalog.set_last_run(quelle, art.name(), netz.uhr.jetzt_ms().div_euclid(1000)).map_err(k)?;
        Ok(())
    })();
    Laufbericht {
        quelle: quelle.into(),
        art,
        gelesen,
        version_vorher: vorher,
        version_nachher: katalog.version(quelle).unwrap_or(-1),
        dauer_ms: netz.uhr.jetzt_ms() - start,
        ausgang,
    }
}
