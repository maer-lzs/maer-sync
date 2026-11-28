// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The fetch program as a program: one step runs the source that is due
//! and answers how long to sleep. The catalog is open only during a run and
//! closed cleanly after it, so a stop while the program sleeps leaves no
//! side files. Against a fake network and a fake clock; no witness reaches
//! the network.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use maer_sync::katalog::{Katalog, MAX_PAGE};
use maer_sync_abruf::lauf::{lauf, Einstellungen, LaufFehler};
use maer_sync_abruf::netz::{decode_params, Anfrage, Antwort, HolFehler, Holer, Netz, Uhr};
use maer_sync_abruf::plan::{Art, BACKOFF_S};
use maer_sync_abruf::programm::{Programm, QUELLEN};
use serde_json::{json, Value};

type Handler = Box<dyn FnMut(&Anfrage, &BTreeMap<String, String>) -> Result<Antwort, HolFehler>>;

#[derive(Clone)]
struct Clock(Rc<RefCell<i64>>);

impl Uhr for Clock {
    fn jetzt_ms(&self) -> i64 { *self.0.borrow() }
    fn schlafe_ms(&mut self, ms: i64) { *self.0.borrow_mut() += ms.max(0); }
}

struct Fake {
    clock: Clock,
    handler: Rc<RefCell<Handler>>,
}

impl Holer for Fake {
    fn hole(&mut self, a: &Anfrage) -> Result<Antwort, HolFehler> {
        let p: BTreeMap<String, String> = decode_params(&a.url).into_iter().collect();
        *self.clock.0.borrow_mut() += 50;
        (self.handler.borrow_mut())(a, &p)
    }
}

const T0_MS: i64 = 1_764_000_000_000; // 2025-11-24

fn ok(v: Value) -> Result<Antwort, HolFehler> {
    Ok(Antwort { status: 200, koerper: v.to_string().into_bytes(), ..Default::default() })
}

fn adv(q: &str, total: usize, docs: Vec<Value>) -> Value {
    json!({"responseHeader": {"params": {"query": q}}, "response": {"numFound": total, "start": 0, "docs": docs}})
}

/// One Musopen file and one station, for the two sources beside archive.org.
fn commons_or_radio(a: &Anfrage, p: &BTreeMap<String, String>) -> Option<Result<Antwort, HolFehler>> {
    if a.url.starts_with("https://commons.wikimedia.org/") {
        if p.get("prop").map(String::as_str) == Some("categoryinfo") {
            return Some(ok(json!({"query": {"pages": [{"title": p["titles"], "categoryinfo": {"files": 1}}]}})));
        }
        let id = if p["gcmtitle"] == "Category:Musopen" { 1 } else { 2 };
        return Some(ok(json!({"query": {"pages": [{"pageid": id, "ns": 6, "title": "File:A - I. B.ogg",
            "videoinfo": [{"url": "https://upload.wikimedia.org/a.ogg", "mime": "application/ogg", "derivatives": [{"src": "https://upload.wikimedia.org/t/a.ogg.mp3", "type": "audio/mpeg", "transcodekey": "mp3"}], "extmetadata": {"License": {"value": "pd"}}}]}]}})));
    }
    if a.url.contains(".api.radio-browser.info/") {
        if a.url.contains("/json/stats") { return Some(ok(json!({"stations": 1, "stations_broken": 0}))); }
        let offset: usize = p["offset"].parse().unwrap();
        let v = if offset == 0 { json!([{"stationuuid": "00000000-0000-0000-0000-000000000001", "name": "S", "url": "http://s"}]) } else { json!([]) };
        return Some(ok(v));
    }
    // `www` answers as the mirror would; which one is asked: katalog_befunde.rs.
    if a.url == maer_sync_abruf::lauf::GUTENBERG_KATALOG || a.url == maer_sync_abruf::lauf::GUTENBERG_KATALOG_WWW {
        let csv = "Text#,Type,Issued,Title,Language,Authors,Subjects,LoCC,Bookshelves\n1,Sound,2003-01-01,T,en,A,,PR,\n";
        return Some(Ok(Antwort { status: 200, koerper: csv.as_bytes().to_vec(), ..Default::default() }));
    }
    if a.url == maer_sync_abruf::lauf::gutenberg_rdf_url("1") || a.url == maer_sync_abruf::lauf::gutenberg_rdf_url_www("1") {
        let rdf = r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pgterms="http://www.gutenberg.org/2009/pgterms/" xmlns:dcterms="http://purl.org/dc/terms/"><pgterms:ebook rdf:about="ebooks/1"><dcterms:rights>Public domain in the USA.</dcterms:rights></pgterms:ebook></rdf:RDF>"#;
        return Some(Ok(Antwort { status: 200, koerper: rdf.as_bytes().to_vec(), ..Default::default() }));
    }
    None
}

fn coll_of(q: &str) -> &'static str {
    ["librivoxaudio", "audio_bookspoetry", "netlabels", "georgeblood", "oldtimeradio"].into_iter()
        .find(|c| q.contains(&format!("collection:{c}"))).unwrap_or_else(|| panic!("the fake does not know {q}"))
}

/// Every archive collection with `n` records; shellac shelves with records
/// carrying their own word as subject and a year.
fn archive(n: usize, year: i64) -> Handler {
    Box::new(move |a, p| {
        if let Some(r) = commons_or_radio(a, p) { return r; }
        let q = p.get("q").cloned().unwrap_or_default();
        let coll = coll_of(&q);
        let word = q.split("subject:\"").nth(1).and_then(|r| r.split('"').next()).map(str::to_string);
        let rec = |i: usize| {
            let id = match &word { Some(w) => format!("78_{w}-{i}"), None => format!("{coll}-{i}") };
            json!({"identifier": id, "title": id, "collection": [coll],
                   "subject": ["78rpm", word.clone().unwrap_or_default()], "year": year})
        };
        if a.url.starts_with("https://archive.org/advancedsearch.php") {
            let rows: usize = p["rows"].parse().unwrap();
            let page: usize = p["page"].parse().unwrap();
            let docs = (0..n).skip((page - 1) * rows).take(rows).map(rec).collect();
            return ok(adv(&q, n, docs));
        }
        if a.url.starts_with("https://archive.org/services/search/v1/scrape") {
            return ok(json!({"items": (0..n).map(rec).collect::<Vec<_>>(), "count": n, "total": n}));
        }
        panic!("the fake does not know {}", a.url);
    })
}

struct Aufbau {
    programm: Programm<Fake, Clock>,
    clock: Clock,
    handler: Rc<RefCell<Handler>>,
    path: String,
    dir: std::path::PathBuf,
}

fn aufbau(name: &str, handler: Handler, start_ms: i64) -> Aufbau {
    let dir = std::env::temp_dir().join(format!("maer-sync-programm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("katalog.db").to_string_lossy().into_owned();
    let clock = Clock(Rc::new(RefCell::new(start_ms)));
    let handler = Rc::new(RefCell::new(handler));
    let netz = Netz::new(Fake { clock: clock.clone(), handler: handler.clone() }, clock.clone());
    let programm = Programm::new(netz, &path).unwrap();
    Aufbau { programm, clock, handler, path, dir }
}

fn side_files(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with("-wal") || n.ends_with("-shm") || n.ends_with("-journal")).collect();
    v.sort();
    v
}

#[test]
fn only_sources_with_a_reader_are_planned() {
    assert_eq!(QUELLEN, ["archive-buch", "gutenberg", "librivox", "musopen", "netlabel", "otr", "radio", "schellack"]);
}

#[test]
fn a_step_runs_the_due_source_and_answers_the_sleep() {
    let mut a = aufbau("step", archive(3, 1920), T0_MS);
    let (b, schlaf) = a.programm.schritt();
    let b = b.expect("a new catalog has a source due");
    assert_eq!((b.quelle.as_str(), b.art, b.ausgang.clone()), ("archive-buch", Art::Voll, Ok(())));
    assert!((60..=3_600).contains(&schlaf), "{schlaf}");
    // All eight run one after another, then nothing is due for a day.
    let mut done = vec![b.quelle];
    for _ in 0..7 {
        done.push(a.programm.schritt().0.expect("still due").quelle);
    }
    assert_eq!(done, ["archive-buch", "gutenberg", "librivox", "musopen", "netlabel", "otr", "radio", "schellack"]);
    let (none, schlaf) = a.programm.schritt();
    assert!(none.is_none());
    assert_eq!(schlaf, 3_600);
}

#[test]
fn between_runs_the_catalog_is_closed_clean() {
    let mut a = aufbau("clean", archive(2, 1920), T0_MS);
    a.programm.schritt();
    assert_eq!(side_files(&a.dir), Vec::<String>::new(), "side files beside the catalog between runs");
    let mode: String = rusqlite::Connection::open(&a.path).unwrap().query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode.to_lowercase(), "delete");
}

#[test]
fn a_failed_step_backs_off() {
    let mut a = aufbau("backoff", Box::new(|_, _| Ok(Antwort { status: 503, ..Default::default() })), T0_MS);
    let (b, _) = a.programm.schritt();
    assert!(matches!(b.unwrap().ausgang, Err(LaufFehler::Holen(HolFehler::Status(503)))));
    // The failed source is not asked again before its backoff; the next one is.
    let (b2, _) = a.programm.schritt();
    assert_eq!(b2.unwrap().quelle, "gutenberg");
    *a.handler.borrow_mut() = archive(2, 1920);
    for _ in 0..6 { a.programm.schritt(); }
    assert!(a.programm.schritt().0.is_none(), "the failed source came back before its backoff");
    *a.clock.0.borrow_mut() += BACKOFF_S[0] * 1000;
    let (b3, _) = a.programm.schritt();
    assert_eq!(b3.unwrap().quelle, "archive-buch");
}

#[test]
fn retry_after_postpones_the_source() {
    let mut a = aufbau("later", Box::new(|_, _| Ok(Antwort { status: 429, retry_after_s: Some(7_200), ..Default::default() })), T0_MS);
    a.programm.schritt();
    *a.handler.borrow_mut() = archive(2, 1920);
    for _ in 0..7 { a.programm.schritt(); }
    *a.clock.0.borrow_mut() += BACKOFF_S[0] * 1000 + 1000;
    assert!(a.programm.schritt().0.is_none(), "asked again before Retry-After");
    *a.clock.0.borrow_mut() += 7_200 * 1000;
    assert_eq!(a.programm.schritt().0.unwrap().quelle, "archive-buch");
}

#[test]
fn run_times_come_back_from_the_catalog() {
    let mut a = aufbau("times", archive(2, 1920), T0_MS);
    for _ in 0..8 { a.programm.schritt(); }
    let path = a.path.clone();
    let clock = Clock(Rc::new(RefCell::new(*a.clock.0.borrow())));
    let handler: Rc<RefCell<Handler>> = Rc::new(RefCell::new(Box::new(|a, _| panic!("asked after a restart: {}", a.url))));
    let netz = Netz::new(Fake { clock: clock.clone(), handler }, clock);
    let mut again = Programm::new(netz, &path).unwrap();
    assert!(again.schritt().0.is_none(), "a restart fetched everything again");
}

#[test]
fn the_calendar_year_comes_from_the_clock() {
    // 2027-01-01 00:00:10 UTC: a record of 1926 is free by its year now.
    let mut a = aufbau("year", archive(1, 1926), 1_798_761_610_000);
    assert_eq!(a.programm.kalenderjahr(), 2027);
    for _ in 0..8 { a.programm.schritt(); }
    let k = Katalog::open_read(&a.path).unwrap();
    let rows = k.full("schellack", None, MAX_PAGE).unwrap().zeilen;
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|s| s.row.rechte_kennung == "pd" && !s.row.laden), "{:?}", rows[0].row);
}

// ---------------------------------------------------------------- the run, from the reviewer's notes

fn settings() -> Einstellungen { Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: 2026 } }

#[test]
fn a_count_that_moves_during_the_pass_is_counted_again() {
    // The count says 5, a record is added before the pages are read: the
    // second count says 6 and the pass of 6 is written.
    let counts = Rc::new(RefCell::new(vec![5usize, 6]));
    let c2 = counts.clone();
    let handler: Handler = Box::new(move |a, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        if a.url.contains("advancedsearch") {
            let n = { let mut c = c2.borrow_mut(); if c.len() > 1 { c.remove(0) } else { c[0] } };
            return ok(adv(&q, n, vec![]));
        }
        ok(json!({"items": (0..6).map(|i| json!({"identifier": format!("ot-{i}"), "collection": ["oldtimeradio"]})).collect::<Vec<_>>()}))
    });
    let mut a = aufbau("moving", handler, T0_MS);
    let mut k = Katalog::open(&a.path).unwrap();
    let b = lauf(&mut a.programm.netz, &mut k, "otr", Art::Voll, &settings());
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!(b.gelesen, 6);
    // A count that matches neither the first nor the second is still refused.
    *counts.borrow_mut() = vec![4, 4];
    let b = lauf(&mut a.programm.netz, &mut k, "otr", Art::Voll, &settings());
    assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{:?}", b.ausgang);
}

#[test]
fn advancedsearch_above_its_bound_is_refused() {
    let handler: Handler = Box::new(|_, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        ok(adv(&q, maer_sync_abruf::lauf::ADV_GRENZE + 1, vec![]))
    });
    let mut a = aufbau("bound", handler, T0_MS);
    let mut k = Katalog::open(&a.path).unwrap();
    let b = lauf(&mut a.programm.netz, &mut k, "schellack", Art::Voll, &settings());
    assert!(matches!(&b.ausgang, Err(LaufFehler::Quelle(s)) if s.contains("advancedsearch")), "{:?}", b.ausgang);
    assert_eq!(k.version("schellack").unwrap(), 0);
}

#[test]
fn a_shellac_delta_keeps_the_other_shelves() {
    // A record on jazz and swing; the delta finds it again only on jazz.
    let handler: Handler = Box::new(|_, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        let word = q.split("subject:\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
        let on = (word == "jazz" || word == "swing") && !(q.contains("addeddate") && word == "swing");
        let docs = if on { vec![json!({"identifier": "78_both", "title": "Both", "collection": ["georgeblood"], "subject": [word], "year": 1940})] } else { vec![] };
        ok(adv(&q, docs.len(), docs))
    });
    let mut a = aufbau("shelves", handler, T0_MS);
    let mut k = Katalog::open(&a.path).unwrap();
    assert_eq!(lauf(&mut a.programm.netz, &mut k, "schellack", Art::Voll, &settings()).ausgang, Ok(()));
    let reihen = |k: &Katalog| k.full("schellack", None, MAX_PAGE).unwrap().zeilen[0].row.reihen.clone();
    assert_eq!(reihen(&k), vec!["shellac-jazz", "shellac-swing"]);
    assert_eq!(lauf(&mut a.programm.netz, &mut k, "schellack", Art::Delta, &settings()).ausgang, Ok(()));
    assert_eq!(reihen(&k), vec!["shellac-jazz", "shellac-swing"], "the delta dropped a shelf");
}
/// A catalog file laid down before the start; `fill` writes into it.
fn vor_dem_start(name: &str, fill: impl FnOnce(&str)) -> (String, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("maer-sync-programm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("katalog.db").to_string_lossy().into_owned();
    fill(&path);
    (path, dir)
}

fn netz_mit(handler: Handler) -> Netz<Fake, Clock> {
    let clock = Clock(Rc::new(RefCell::new(T0_MS)));
    Netz::new(Fake { clock: clock.clone(), handler: Rc::new(RefCell::new(handler)) }, clock)
}

#[test]
fn the_program_starts_beside_an_empty_catalog_file() {
    // An empty file is what a first run leaves when it ends between laying
    // the file down and writing the schema. It held the program off at every
    // start, and the container restarted into the same end.
    let (path, dir) = vor_dem_start("leer", |p| std::fs::write(p, b"").unwrap());
    let mut p = Programm::new(netz_mit(archive(2, 1920)), &path)
        .unwrap_or_else(|e| panic!("the program did not start beside an empty file: {e}"));
    let (b, _) = p.schritt();
    assert_eq!(b.expect("every source is due").ausgang, Ok(()));
    let k = Katalog::open_read(&path).expect("the first run lays the catalog down");
    assert!(k.version("archive-buch").unwrap() > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_confirmation_starts_beside_an_empty_catalog_file() {
    let (path, dir) = vor_dem_start("leer-bestaetigt", |p| std::fs::write(p, b"").unwrap());
    let mut p = Programm::new(netz_mit(archive(2, 1920)), &path)
        .unwrap_or_else(|e| panic!("the confirmation did not start beside an empty file: {e}"));
    let b = p.bestaetigt("archive-buch");
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert!(Katalog::open_read(&path).unwrap().version("archive-buch").unwrap() > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_program_starts_beside_a_file_without_tables() {
    let (path, dir) = vor_dem_start("ohne-tabellen", |p| {
        rusqlite::Connection::open(p).unwrap().execute_batch("PRAGMA user_version = 1;").unwrap();
    });
    let mut p = Programm::new(netz_mit(archive(2, 1920)), &path)
        .unwrap_or_else(|e| panic!("the program did not start beside a file without tables: {e}"));
    assert_eq!(p.schritt().0.expect("due").ausgang, Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_program_still_refuses_the_user_store() {
    let (path, dir) = vor_dem_start("nutzer", |p| {
        rusqlite::Connection::open(p).unwrap()
            .execute_batch("CREATE TABLE documents (x); CREATE TABLE meta (key TEXT, value TEXT);").unwrap();
    });
    let neu = Programm::new(netz_mit(archive(2, 1920)), &path);
    assert!(neu.is_err(), "the user store was taken as an empty catalog");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_program_still_refuses_an_unknown_format() {
    let (path, dir) = vor_dem_start("format", |p| {
        rusqlite::Connection::open(p).unwrap()
            .execute_batch("CREATE TABLE meta (key TEXT, value TEXT); INSERT INTO meta VALUES ('katalog_format', '99');").unwrap();
    });
    let neu = Programm::new(netz_mit(archive(2, 1920)), &path);
    assert!(neu.is_err(), "a catalog of an unknown format was taken as empty");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- the popularity step

/// archive.org with `n` records whose downloads are `base - 10 * i`; every
/// URL asked is kept.
fn archive_downloads(n: usize, base: i64, urls: Rc<RefCell<Vec<String>>>) -> Handler {
    let mut inner = archive(n, 1920);
    Box::new(move |a, p| {
        urls.borrow_mut().push(a.url.clone());
        let mut r = inner(a, p)?;
        let mut v: Value = serde_json::from_slice(&r.koerper).unwrap();
        let list = if v.get("items").is_some() { v["items"].as_array_mut() } else { v["response"]["docs"].as_array_mut() };
        if let Some(list) = list {
            for (i, it) in list.iter_mut().enumerate() {
                let id = it["identifier"].as_str().unwrap().to_string();
                let k: i64 = id.rsplit('-').next().unwrap().parse().unwrap_or(i as i64);
                it["downloads"] = json!(base - 10 * k);
            }
        }
        r.koerper = v.to_string().into_bytes();
        Ok(r)
    })
}

#[test]
fn a_full_pass_asks_for_downloads_and_sets_the_steps() {
    let urls = Rc::new(RefCell::new(vec![]));
    let mut a = aufbau("stufe-voll", archive_downloads(20, 1000, urls.clone()), T0_MS);
    let b = a.programm.ein_lauf("librivox", Art::Voll, false);
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    let asked: Vec<String> = urls.borrow().iter().filter(|u| u.contains("archive.org") && !u.contains("rows=0")).cloned().collect();
    assert!(!asked.is_empty());
    for u in &asked {
        assert!(u.contains("downloads"), "downloads not asked for: {u}");
    }
    let k = Katalog::open_read(&a.path).unwrap();
    let rows = k.full("librivox", None, MAX_PAGE).unwrap().zeilen;
    assert_eq!(rows.len(), 20);
    for s in &rows {
        let i: i64 = s.row.id.rsplit('-').next().unwrap().parse().unwrap();
        assert_eq!(s.row.beliebt_stufe, Some(1 + i / 2), "{}", s.row.id);
    }
}

#[test]
fn the_line_counts_the_rows_whose_step_changed() {
    let urls = Rc::new(RefCell::new(vec![]));
    let mut a = aufbau("stufe-zeile", archive_downloads(20, 1000, urls.clone()), T0_MS);
    let b = a.programm.ein_lauf("librivox", Art::Voll, false);
    assert!(b.zeile().contains(" stufen_gewechselt=0"), "{}", b.zeile());
    // The order turns round: every row moves to the mirrored tenth.
    *a.handler.borrow_mut() = Box::new({
        let mut inner = archive_downloads(20, 1000, urls.clone());
        move |r, p| {
            let mut x = inner(r, p)?;
            let mut v: Value = serde_json::from_slice(&x.koerper).unwrap();
            if let Some(list) = v.get_mut("items").and_then(Value::as_array_mut) {
                for it in list.iter_mut() {
                    it["downloads"] = json!(2000 - it["downloads"].as_i64().unwrap());
                }
            }
            x.koerper = v.to_string().into_bytes();
            Ok(x)
        }
    });
    let b = a.programm.ein_lauf("librivox", Art::Voll, false);
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert!(b.zeile().contains(" stufen_gewechselt=20"), "{}", b.zeile());
    let b = a.programm.ein_lauf("librivox", Art::Voll, false);
    assert!(b.zeile().contains(" stufen_gewechselt=0"), "{}", b.zeile());
}

// ---------------------------------------------------------------- gathering the numbers

/// Shellac on two shelves, two records to a page: jazz holds a, both and c,
/// opera holds both, d and e; every other shelf is empty. `umgekehrt` turns
/// the downloads round, for a delta that must not move a step.
fn zwei_regale(umgekehrt: bool) -> Handler {
    Box::new(move |_, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        let word = q.split("subject:\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
        let ids: Vec<(&str, i64)> = match word.as_str() {
            "jazz" => vec![("78_a", 50), ("78_both", 40), ("78_c", 30)],
            "opera" => vec![("78_both", 40), ("78_d", 20), ("78_e", 10)],
            _ => vec![],
        };
        let rows: usize = p["rows"].parse().unwrap();
        let page: usize = p["page"].parse().unwrap();
        let docs: Vec<Value> = ids.iter().skip((page - 1) * rows).take(rows).map(|(id, d)| {
            let d = if umgekehrt { 60 - d } else { *d };
            json!({"identifier": id, "title": id, "collection": ["georgeblood"], "subject": [word], "year": 1925, "downloads": d})
        }).collect();
        ok(adv(&q, ids.len(), docs))
    })
}

fn zwei_je_seite() -> Einstellungen { Einstellungen { scrape_count: 10_000, adv_rows: 2, kalenderjahr: 2026 } }

fn stufen_von(k: &Katalog, q: &str) -> BTreeMap<String, Option<i64>> {
    k.full(q, None, MAX_PAGE).unwrap().zeilen.into_iter().map(|s| (s.row.id, s.row.beliebt_stufe)).collect()
}

#[test]
fn shellac_gathers_the_numbers_of_every_shelf_and_page() {
    let mut a = aufbau("sammeln-regale", zwei_regale(false), T0_MS);
    let mut k = Katalog::open(&a.path).unwrap();
    let b = lauf(&mut a.programm.netz, &mut k, "schellack", Art::Voll, &zwei_je_seite());
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    // Five records; the one on two shelves is one row with one number and
    // one step. Ranks 0 to 4 of 5 give the steps 1, 3, 5, 7, 9.
    let want: BTreeMap<String, Option<i64>> = [("lva:78_a", 1), ("lva:78_both", 3), ("lva:78_c", 5), ("lva:78_d", 7), ("lva:78_e", 9)]
        .into_iter().map(|(id, s)| (id.to_string(), Some(s))).collect();
    assert_eq!(stufen_von(&k, "schellack"), want);
}

#[test]
fn a_shellac_delta_gathers_no_numbers() {
    let mut a = aufbau("sammeln-delta", zwei_regale(false), T0_MS);
    let mut k = Katalog::open(&a.path).unwrap();
    assert_eq!(lauf(&mut a.programm.netz, &mut k, "schellack", Art::Voll, &zwei_je_seite()).ausgang, Ok(()));
    let vorher = stufen_von(&k, "schellack");
    *a.handler.borrow_mut() = zwei_regale(true);
    let b = lauf(&mut a.programm.netz, &mut k, "schellack", Art::Delta, &zwei_je_seite());
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(stufen_von(&k, "schellack"), vorher, "a delta moved a step");
    assert!(b.zeile().contains(" stufen_gewechselt=0"), "{}", b.zeile());
}

/// LibriVox with seven records over scrape pages of `count`, the cursor the
/// next index; the downloads fall with the number.
fn scrape_seiten() -> Handler {
    Box::new(|a, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        let n = 7usize;
        if a.url.starts_with("https://archive.org/advancedsearch.php") {
            return ok(adv(&q, n, vec![]));
        }
        assert!(a.url.starts_with("https://archive.org/services/search/v1/scrape"), "{}", a.url);
        let count: usize = p["count"].parse().unwrap();
        let start: usize = p.get("cursor").map(|c| c.parse().unwrap()).unwrap_or(0);
        let items: Vec<Value> = (start..(start + count).min(n)).map(|i| json!({
            "identifier": format!("item-{i}"), "title": format!("Title {i}"),
            "collection": ["librivoxaudio"], "downloads": 100 - i as i64,
        })).collect();
        let mut v = json!({"items": items, "count": items.len(), "total": n});
        if start + count < n { v["cursor"] = json!((start + count).to_string()); }
        ok(v)
    })
}

#[test]
fn scrape_gathers_the_downloads_of_every_page() {
    let mut a = aufbau("sammeln-scrape", scrape_seiten(), T0_MS);
    let mut k = Katalog::open(&a.path).unwrap();
    let drei = Einstellungen { scrape_count: 3, adv_rows: 10_000, kalenderjahr: 2026 };
    let b = lauf(&mut a.programm.netz, &mut k, "librivox", Art::Voll, &drei);
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    let rows = k.full("librivox", None, MAX_PAGE).unwrap().zeilen;
    assert_eq!(rows.len(), 7, "the witness needs three pages of three");
    for s in rows {
        let i: i64 = s.row.id.rsplit('-').next().unwrap().parse().unwrap();
        assert_eq!(s.row.beliebt_stufe, Some(1 + 10 * i / 7), "{}", s.row.id);
    }
}

#[test]
fn the_program_does_not_start_beside_a_file_with_foreign_tables() {
    let (path, dir) = vor_dem_start("fremd", |p| {
        rusqlite::Connection::open(p).unwrap().execute_batch("CREATE TABLE fremd (x TEXT);").unwrap();
    });
    let neu = Programm::new(netz_mit(archive(2, 1920)), &path);
    assert!(neu.is_err(), "the program took a foreign file as one without a catalog");
    let _ = std::fs::remove_dir_all(&dir);
}
