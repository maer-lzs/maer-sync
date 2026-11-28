// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! One run of one source.
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

use std::collections::{BTreeMap, BTreeSet};

use maer_sync::katalog::{Katalog, KatalogError, Row, MAX_PAGE};
use serde_json::Value;

use crate::archive::{downloads_from_answer, merge_shelves, rows_from_answer, ReadError, Shelf};
use crate::musopen::{self, MusopenFehler};
use crate::gutenberg::{self, Rechte};
use crate::radio;
use crate::stufe::stufen;
use crate::netz::{encode, gewichtigster, HolFehler, Holer, Netz, Uhr};
use crate::plan::{Art, DELTA_S};

/// The most records one query of the advanced search is read for. A delta
/// and a shelf stay far below it (the largest shelf, jazz, has some 6 800); a
/// query above it is refused rather than read short.
pub const ADV_GRENZE: usize = 10_000;

/// Sources without a delta: a due delta runs as a full pass.
pub const OHNE_DELTA: [&str; 2] = ["musopen", "radio"];

const ADV: &str = "https://archive.org/advancedsearch.php";
const SCRAPE: &str = "https://archive.org/services/search/v1/scrape";
const FIELDS: [&str; 12] = [
    "identifier", "title", "creator", "date", "year", "licenseurl", "addeddate", "publicdate", "collection",
    "language", "subject", "downloads",
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
    /// The catalog refused the run as too large a loss (no rows, or too many
    /// live rows gone); `lebend` is the number of live rows it holds.
    Gesperrt { grund: String, lebend: usize },
    /// Another run holds the catalog: nothing was fetched or written.
    Belegt,
    /// The pages of a pass did not end: a cursor came back, or there were
    /// more pages than the count allows. Nothing is written.
    Seiten(String),
    /// More works than a run may ask the records of; asked before the first.
    Deckel(String),
    /// More than a tenth of the records failed: nothing is written.
    Ausfall { ausgefallen: usize, gesamt: usize },
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
    /// The operator confirmed this run (`--bestaetigt`).
    pub bestaetigt: bool,
    /// Rows the catalog held live before a full pass whose popularity
    /// step it changed; new rows do not count. Two regular runs side by
    /// side measure how often the tenths move.
    pub stufen_gewechselt: usize,
}

impl Laufbericht {
    /// One line for the log: source, kind, records read, versions, time and
    /// outcome with its reason. Never any text of an answer.
    pub fn zeile(&self) -> String {
        let ausgang = match &self.ausgang {
            Ok(()) => "ok".to_string(),
            // A refusal of the bound on losses names the live rows and the
            // command that confirms the run.
            Err(LaufFehler::Gesperrt { grund, lebend }) => format!(
                "abgewiesen grund=Gesperrt({}) lebend={lebend} bestaetigen=\"maer-sync-abruf --bestaetigt {}\"",
                format!("{grund:?}").replace(['\n', '\r'], " "), self.quelle),
            Err(LaufFehler::Belegt) =>
                "abgewiesen grund=Belegt (another run holds the catalog; nothing fetched or written, later again)".into(),
            Err(e) => format!("abgewiesen grund={}", format!("{e:?}").replace(['\n', '\r'], " ")),
        };
        let mut zeile = format!(
            "lauf quelle={} art={} gelesen={} version={}->{} dauer_ms={} ausgang={ausgang}",
            self.quelle, self.art.name(), self.gelesen, self.version_vorher, self.version_nachher, self.dauer_ms);
        zeile.push_str(&format!(" stufen_gewechselt={}", self.stufen_gewechselt));
        if self.bestaetigt {
            zeile.push_str(" bestaetigt=ja");
        }
        zeile
    }
}

/// The query of a source of the index; shellac has none (shelves only).
/// The catalog dump of Project Gutenberg, on the mirror the app takes.
pub const GUTENBERG_KATALOG: &str = "https://gutenberg.pglaf.org/cache/epub/feeds/pg_catalog.csv";

/// At most this many records of works in one run.
pub const GUTENBERG_RDF_DECKEL: usize = 2000;

/// The record of work `nummer` on the mirror.
pub fn gutenberg_rdf_url(nummer: &str) -> String {
    format!("https://gutenberg.pglaf.org/cache/epub/{nummer}/pg{nummer}.rdf")
}

/// The full pass of Gutenberg: the dump, then the record of every work of
/// type `Sound`, one by one (the net keeps them a second apart). A
/// copyrighted work stays out. A failed record leaves a living work as the
/// last run had it and keeps a new one out; more than a tenth failed
/// refuses the run.
fn gutenberg_pass<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, katalog: &Katalog) -> Result<Vec<Row>, LaufFehler> {
    let dump = netz.get(GUTENBERG_KATALOG)?;
    let rows = gutenberg::rows_from_catalog(&dump.koerper)?;
    if rows.len() > GUTENBERG_RDF_DECKEL {
        return Err(LaufFehler::Deckel(format!("{} works of type Sound, at most {GUTENBERG_RDF_DECKEL} records a run", rows.len())));
    }
    let gesamt = rows.len();
    let mut out = Vec::with_capacity(gesamt);
    let mut zahlen = BTreeMap::new();
    let mut behalten = BTreeMap::new();
    let mut ausgefallen = 0;
    for mut r in rows {
        let nummer = r.id.trim_start_matches("gut:").to_string();
        let gelesen = netz.get(&gutenberg_rdf_url(&nummer)).map_err(LaufFehler::from)
            .and_then(|a| gutenberg::rdf_lesen(&a.koerper, &nummer).map_err(LaufFehler::from));
        match gelesen {
            Ok(rdf) => {
                match rdf.rechte {
                    Rechte::Copyrighted => continue,
                    Rechte::Pd => { r.rechte_kennung = "pd".into(); r.laden = true; }
                    Rechte::Andere => {}
                }
                if let Some(d) = rdf.downloads {
                    zahlen.insert(r.id.clone(), d);
                }
                out.push(r);
            }
            Err(_) => {
                ausgefallen += 1;
                let alt = katalog.stored("gutenberg", &r.id).map_err(|e| LaufFehler::Katalog(e.to_string()))?;
                if let Some(s) = alt.filter(|s| !s.geloescht) {
                    r.rechte_kennung = s.row.rechte_kennung;
                    r.rechte_url = s.row.rechte_url;
                    r.laden = s.row.laden;
                    behalten.insert(r.id.clone(), s.row.beliebt_stufe);
                    out.push(r);
                }
            }
        }
    }
    if ausgefallen * 10 > gesamt {
        return Err(LaufFehler::Ausfall { ausgefallen, gesamt });
    }
    stufen(&mut out, &zahlen);
    for r in out.iter_mut() {
        if let Some(s) = behalten.get(&r.id) {
            r.beliebt_stufe = *s;
        }
    }
    Ok(out)
}

pub fn base_query(quelle: &str) -> Option<&'static str> {
    match quelle {
        "librivox" => Some("collection:librivoxaudio"),
        "archive-buch" => Some("collection:audio_bookspoetry AND mediatype:(audio)"),
        "netlabel" => Some("collection:netlabels AND mediatype:(audio)"),
        "otr" => Some("collection:oldtimeradio AND mediatype:(audio)"),
        _ => None,
    }
}

/// The list of the theme shelves the program reads, as the app names them.
/// It belongs to the program; the vectors keep a copy for the device sides.
pub const REGALE_JSON: &str = include_str!("../regale.json");

/// The theme shelves of the app.
pub fn regale_der_app() -> Vec<Shelf> {
    let v: Value = serde_json::from_str(REGALE_JSON)
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

/// Whether the echo of a query is the query (a hull for now: compared as is).
pub fn echo_gleich(gestellt: &str, zurueck: &str) -> bool {
    gestellt == zurueck
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
fn adv_all<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, quelle: &str, q: &str, e: &Einstellungen, shelf: Option<&Shelf>,
                             zahlen: &mut BTreeMap<String, i64>) -> Result<Vec<Row>, LaufFehler> {
    let mut rows = vec![];
    let mut page = 1;
    loop {
        let (n, body) = adv_page(netz, q, e.adv_rows, page)?;
        if n > ADV_GRENZE {
            return Err(LaufFehler::Quelle(format!("advancedsearch answers {n} records, more than {ADV_GRENZE} are not read")));
        }
        let got = rows_from_answer(quelle, &body, e.kalenderjahr, shelf)?;
        zahlen.extend(downloads_from_answer(&body));
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

impl From<MusopenFehler> for LaufFehler {
    fn from(e: MusopenFehler) -> Self {
        match e {
            MusopenFehler::Unreadable(s) => LaufFehler::Lesen(ReadError::Unreadable(s)),
            MusopenFehler::Maxlag(s) => LaufFehler::Holen(HolFehler::SpaeterWieder(s)),
        }
    }
}

const COMMONS: &str = "https://commons.wikimedia.org/w/api.php";
/// Files per Commons page: `extmetadata` is heavy, Commons answers 50 well.
const COMMONS_SEITE: usize = 50;

fn commons_url(params: &[(&str, &str)]) -> String {
    let mut u = format!("{COMMONS}?action=query&format=json&formatversion=2&maxlag=5");
    for (k, v) in params {
        u.push_str(&format!("&{k}={}", encode(v)));
    }
    u
}

/// Both categories of Musopen in full; each counted against `categoryinfo`
/// (pages seen, media or not); a file in both is one row.
fn musopen_pass<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, e: &Einstellungen) -> Result<Vec<Row>, LaufFehler> {
    let mut rows: Vec<Row> = vec![];
    let mut ids = BTreeSet::new();
    let seite = e.adv_rows.min(COMMONS_SEITE).max(1);
    let limit = seite.to_string();
    for cat in musopen::KATEGORIEN {
        let a = netz.get(&commons_url(&[("prop", "categoryinfo"), ("titles", cat)]))?;
        let v = parse(&a.koerper)?;
        if v.pointer("/error/code").and_then(Value::as_str) == Some("maxlag") {
            musopen::rows_from_answer(&a.koerper)?;
        }
        let files = v.pointer("/query/pages/0/categoryinfo/files").and_then(Value::as_u64)
            .ok_or_else(|| LaufFehler::Lesen(ReadError::Unreadable(format!("no count for {cat}"))))? as usize;
        let mut seen = 0;
        let mut weiter: Option<String> = None;
        // The pages a category may take: its count in pages of `limit`, and
        // two more for a count that moved. A cursor that came back, or a page
        // above the cap, ends the pass.
        let deckel = files.div_ceil(seite) + 2;
        let mut seiten = 0;
        let mut cursors = BTreeSet::new();
        loop {
            seiten += 1;
            let mut p = vec![("generator", "categorymembers"), ("gcmtitle", cat), ("gcmtype", "file"),
                             ("gcmlimit", limit.as_str()), ("prop", "videoinfo"), ("viprop", "url|size|mime|derivatives|extmetadata")];
            if let Some(w) = weiter.as_deref() { p.push(("gcmcontinue", w)); }
            let a = netz.get(&commons_url(&p))?;
            let s = musopen::rows_from_answer(&a.koerper)?;
            seen += s.gesehen;
            if seen > files {
                return Err(LaufFehler::Zaehlung { erwartet: files, gelesen: seen, eindeutig: rows.len() });
            }
            for r in s.zeilen {
                if ids.insert(r.id.clone()) { rows.push(r); }
            }
            let Some(w) = s.weiter else { break };
            if !cursors.insert(w.clone()) {
                return Err(LaufFehler::Seiten(format!("{cat}: a cursor came back after {seiten} pages")));
            }
            if seiten > deckel {
                return Err(LaufFehler::Seiten(format!("{cat}: more than {deckel} pages for {files} files")));
            }
            weiter = Some(w);
        }
        if seen != files {
            return Err(LaufFehler::Zaehlung { erwartet: files, gelesen: seen, eindeutig: rows.len() });
        }
    }
    Ok(rows)
}

/// GET a path on the first server of the list that answers, and stay there.
fn radio_get<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, server: &mut Option<&'static str>, path: &str) -> Result<Vec<u8>, LaufFehler> {
    if let Some(s) = *server {
        return Ok(netz.get(&format!("{s}{path}"))?.koerper);
    }
    let mut fehler = vec![];
    for s in radio::SERVER {
        match netz.get(&format!("{s}{path}")) {
            Ok(a) => { *server = Some(s); return Ok(a.koerper); }
            Err(e) => fehler.push(e),
        }
    }
    Err(LaufFehler::Holen(gewichtigster(fehler)))
}

fn radio_count<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, server: &mut Option<&'static str>) -> Result<usize, LaufFehler> {
    let v = parse(&radio_get(netz, server, "/json/stats")?)?;
    let n = |k: &str| v.get(k).and_then(Value::as_u64).ok_or_else(|| LaufFehler::Lesen(ReadError::Unreadable(format!("no {k} in the statistics"))));
    // `stations` counts only the stations that are not broken: exactly what
    // the pages with `hidebroken=true` list.
    Ok(n("stations")? as usize)
}

/// All stations in pages of `adv_rows`, ordered by uuid; counted against the
/// statistics (`stations`, which leaves the broken out). A uuid on two pages (a station moved
/// while paging) repeats the pass once, then refuses.
fn radio_pass<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, e: &Einstellungen, zahlen: &mut BTreeMap<String, i64>)
    -> Result<Vec<Row>, LaufFehler> {
    let mut server = None;
    let limit = e.adv_rows;
    for versuch in 0..2 {
        let erwartet = radio_count(netz, &mut server)?;
        let mut rows = vec![];
        let mut uuids = BTreeSet::new();
        let mut doppelt = false;
        let mut offset = 0;
        // The count in pages of `limit`, and two more for a count that moved;
        // a page above it ends the pass (a server that ignores the offset
        // would page on without end).
        let deckel = erwartet.div_ceil(limit.max(1)) + 2;
        let mut seiten = 0;
        loop {
            seiten += 1;
            let body = radio_get(netz, &mut server,
                &format!("/json/stations?offset={offset}&limit={limit}&hidebroken=true&order=stationuuid"))?;
            let s = radio::rows_from_page(&body)?;
            zahlen.extend(radio::votes_from_page(&body));
            for u in &s.uuids {
                if !uuids.insert(u.clone()) { doppelt = true; }
            }
            rows.extend(s.zeilen);
            if s.roh < limit { break; }
            if seiten > deckel {
                return Err(LaufFehler::Seiten(format!("more than {deckel} pages for {erwartet} stations")));
            }
            offset += limit;
        }
        if doppelt {
            if versuch == 0 { continue; }
            return Err(LaufFehler::Zaehlung { erwartet, gelesen: rows.len(), eindeutig: uuids.len() });
        }
        if uuids.len() != erwartet {
            let again = radio_count(netz, &mut server)?;
            if uuids.len() != again {
                return Err(LaufFehler::Zaehlung { erwartet: again, gelesen: rows.len(), eindeutig: uuids.len() });
            }
        }
        return Ok(rows);
    }
    unreachable!("the loop returns in its second pass")
}

/// A full pass sees every row of the source, so it alone sets the steps.
fn full_pass<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, quelle: &str, e: &Einstellungen) -> Result<Vec<Row>, LaufFehler> {
    let mut zahlen = BTreeMap::new();
    let mut rows = full_rows(netz, quelle, e, &mut zahlen)?;
    stufen(&mut rows, &zahlen);
    Ok(rows)
}

fn full_rows<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, quelle: &str, e: &Einstellungen,
                               zahlen: &mut BTreeMap<String, i64>) -> Result<Vec<Row>, LaufFehler> {
    match quelle {
        "musopen" => return musopen_pass(netz, e),
        "radio" => return radio_pass(netz, e, zahlen),
        _ => {}
    }
    if quelle == "schellack" {
        let mut parts = vec![];
        for s in regale_der_app() {
            parts.push(adv_all(netz, quelle, &shelf_query(&s), e, Some(&s), zahlen)?);
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
        zahlen.extend(downloads_from_answer(&a.koerper));
        cursor = parse(&a.koerper)?.get("cursor").and_then(Value::as_str).map(str::to_string);
        if cursor.is_none() {
            break;
        }
        if rows.len() > n {
            break; // more than counted: the check below refuses it
        }
    }
    // A record added while the pages were read: count again, and the pass
    // stands if it matches the second count.
    if rows.len() != n {
        let (again, _) = adv_page(netz, q, 0, 1)?;
        check_count(&rows, again)?;
        return Ok(rows);
    }
    check_count(&rows, n)?;
    Ok(rows)
}

/// A shellac delta sees a record only on the shelves it was added to again;
/// the shelves the catalog already knows for it stay (a full pass resets
/// them). `stored` answers the shelves the catalog holds for an id. A read
/// that fails refuses the delta: taken as a row that was never there, the
/// record would lose its known shelves.
pub fn keep_known_shelves<F>(rows: &mut [Row], mut stored: F) -> Result<(), LaufFehler>
where
    F: FnMut(&str) -> Result<Option<Vec<String>>, KatalogError>,
{
    for r in rows.iter_mut() {
        if let Some(old) = stored(&r.id).map_err(|e| LaufFehler::Katalog(e.to_string()))? {
            for k in old {
                if !r.reihen.contains(&k) {
                    r.reihen.push(k);
                }
            }
            r.reihen.sort();
        }
    }
    Ok(())
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
            parts.push(adv_all(netz, quelle, &format!("{}{since}", shelf_query(&s)), e, Some(&s), &mut BTreeMap::new())?);
        }
        let mut rows = merge_shelves(parts);
        keep_known_shelves(&mut rows, |id| katalog.stored(quelle, id).map(|s| s.map(|s| s.row.reihen)))?;
        return Ok(rows);
    }
    let q = base_query(quelle).ok_or_else(|| LaufFehler::Quelle(quelle.into()))?;
    adv_all(netz, quelle, &format!("{q}{since}"), e, None, &mut BTreeMap::new())
}

/// Run one source once and report it. Nothing is written unless the whole
/// run succeeded.
pub fn lauf<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, katalog: &mut Katalog, quelle: &str, art: Art,
                              e: &Einstellungen) -> Laufbericht {
    lauf_mit(netz, katalog, quelle, art, e, false)
}

/// A full pass of one source that the operator confirmed: it may empty the
/// source or delete more live rows than the catalog allows a run on its own.
/// The sign holds for this run only.
pub fn lauf_bestaetigt<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, katalog: &mut Katalog, quelle: &str,
                                         e: &Einstellungen) -> Laufbericht {
    lauf_mit(netz, katalog, quelle, Art::Voll, e, true)
}

fn lauf_mit<H: Holer, U: Uhr>(netz: &mut Netz<H, U>, katalog: &mut Katalog, quelle: &str, art: Art,
                              e: &Einstellungen, bestaetigt: bool) -> Laufbericht {
    let start = netz.uhr.jetzt_ms();
    let vorher = katalog.version(quelle).unwrap_or(-1);
    let mut gelesen = 0;
    let mut stufen_gewechselt = 0;
    let ausgang = (|| -> Result<(), LaufFehler> {
        // Musopen and Radio Browser have no delta: their day is a full pass.
        let voll = art == Art::Voll || OHNE_DELTA.contains(&quelle);
        // Gutenberg has no delta: a due one asks nothing and keeps its time.
        if quelle == "gutenberg" && !voll {
            let k = |e: maer_sync::katalog::KatalogError| LaufFehler::Katalog(e.to_string());
            katalog.set_last_run(quelle, art.name(), netz.uhr.jetzt_ms().div_euclid(1000)).map_err(k)?;
            return Ok(());
        }
        let rows = if quelle == "gutenberg" { gutenberg_pass(netz, katalog)? }
            else if voll { full_pass(netz, quelle, e)? } else { delta(netz, katalog, quelle, e)? };
        gelesen = rows.len();
        let k = |e: maer_sync::katalog::KatalogError| LaufFehler::Katalog(e.to_string());
        if voll {
            let vorher = katalog.stufen(quelle).map_err(k)?;
            if let Err(err) = katalog.write_run(quelle, &rows, bestaetigt) {
                return Err(sperre(katalog, quelle, err));
            }
            stufen_gewechselt = rows.iter()
                .filter(|r| vorher.get(&r.id).is_some_and(|s| *s != r.beliebt_stufe))
                .count();
        } else {
            katalog.write_delta(quelle, &rows).map_err(k)?;
        }
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
        bestaetigt,
        stufen_gewechselt,
    }
}

/// A refusal of the bound on losses becomes `Gesperrt` with the live rows
/// the catalog holds; any other error stays a catalog error.
fn sperre(katalog: &Katalog, quelle: &str, err: KatalogError) -> LaufFehler {
    let lebend = match &err {
        KatalogError::MassDelete { live, .. } => Ok(*live),
        KatalogError::EmptyRun => live_rows(katalog, quelle),
        _ => return LaufFehler::Katalog(err.to_string()),
    };
    match lebend {
        Ok(lebend) => LaufFehler::Gesperrt { grund: err.to_string(), lebend },
        Err(e) => LaufFehler::Katalog(format!("{err}; the live rows cannot be counted: {e}")),
    }
}

fn live_rows(katalog: &Katalog, quelle: &str) -> Result<usize, KatalogError> {
    let mut n = 0;
    let mut nach: Option<String> = None;
    loop {
        let p = katalog.full(quelle, nach.as_deref(), MAX_PAGE)?;
        n += p.zeilen.len();
        match p.weiter {
            Some(w) => nach = Some(w),
            None => return Ok(n),
        }
    }
}
