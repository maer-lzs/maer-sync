// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The fetch program against a fake network. No witness here reaches
//! the network: the fake answers from what the witness gives it, records
//! every request with the time of a fake clock, and panics on an address it
//! does not know.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use maer_sync::katalog::{Katalog, KatalogError, Row, MAX_PAGE};
use maer_sync_abruf::netz::{
    decode_params, Anfrage, Antwort, HolFehler, Holer, Netz, Uhr, MAX_BYTES, MAX_REDIRECTS, MIN_GAP_MS, USER_AGENT,
};
use maer_sync_abruf::lauf::{
    base_query, datum, keep_known_shelves, lauf, lauf_bestaetigt, regale_der_app, Einstellungen, LaufFehler,
};
use maer_sync_abruf::programm::{befehl, Befehl};
use maer_sync_abruf::plan::{faellig, naechster, schlaf_s, wann, Art, Zustand, BACKOFF_S, DELTA_S, VOLL_S};
use serde_json::{json, Value};

// ---------------------------------------------------------------- the fake

type Handler = Box<dyn FnMut(&Anfrage, &BTreeMap<String, String>) -> Result<Antwort, HolFehler>>;

#[derive(Clone)]
struct Clock(Rc<RefCell<i64>>);

impl Uhr for Clock {
    fn jetzt_ms(&self) -> i64 { *self.0.borrow() }
    fn schlafe_ms(&mut self, ms: i64) { *self.0.borrow_mut() += ms.max(0); }
}

struct Fake {
    clock: Clock,
    log: Rc<RefCell<Vec<(i64, Anfrage)>>>,
    handler: Handler,
}

impl Holer for Fake {
    fn hole(&mut self, a: &Anfrage) -> Result<Antwort, HolFehler> {
        self.log.borrow_mut().push((self.clock.jetzt_ms(), a.clone()));
        let params: BTreeMap<String, String> = decode_params(&a.url).into_iter().collect();
        // Time passes with every request, as it does on a real network.
        *self.clock.0.borrow_mut() += 50;
        (self.handler)(a, &params)
    }
}

const START_MS: i64 = 1_764_000_000_000; // 2025-11-24, a fixed point

fn net(handler: Handler) -> (Netz<Fake, Clock>, Rc<RefCell<Vec<(i64, Anfrage)>>>, Clock) {
    let clock = Clock(Rc::new(RefCell::new(START_MS)));
    let log = Rc::new(RefCell::new(vec![]));
    let fake = Fake { clock: clock.clone(), log: log.clone(), handler };
    (Netz::new(fake, clock.clone()), log, clock)
}

fn ok(body: Value) -> Result<Antwort, HolFehler> {
    Ok(Antwort { status: 200, koerper: body.to_string().into_bytes(), ..Default::default() })
}

fn item(id: &str, coll: &str) -> Value {
    json!({"identifier": id, "title": format!("Title {id}"), "collection": [coll], "subject": ["78rpm", "Jazz"]})
}

fn ids(n: usize, prefix: &str) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}-{i:05}")).collect()
}

fn adv(q: &str, total: usize, docs: Vec<Value>) -> Value {
    json!({"responseHeader": {"status": 0, "params": {"query": q, "qin": q}},
           "response": {"numFound": total, "start": 0, "docs": docs}})
}

/// A source of `records` for collection `coll`: answers the count, the
/// scrape pages (cursor) and the advanced search pages (rows, page).
fn archive_source(coll: &'static str, records: Vec<String>, scrape_page: usize) -> Handler {
    Box::new(move |a, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        if a.url.starts_with("https://archive.org/advancedsearch.php") {
            let rows: usize = p.get("rows").and_then(|r| r.parse().ok()).unwrap_or(0);
            let page: usize = p.get("page").and_then(|r| r.parse().ok()).unwrap_or(1);
            let docs: Vec<Value> = records.iter().skip((page - 1) * rows).take(rows).map(|i| item(i, coll)).collect();
            return ok(adv(&q, records.len(), docs));
        }
        if a.url.starts_with("https://archive.org/services/search/v1/scrape") {
            let start: usize = p.get("cursor").and_then(|c| c.parse().ok()).unwrap_or(0);
            let items: Vec<Value> = records.iter().skip(start).take(scrape_page).map(|i| item(i, coll)).collect();
            let next = start + scrape_page;
            let mut body = json!({"items": items, "count": items.len(), "total": records.len()});
            if next < records.len() {
                body["cursor"] = json!(next.to_string());
            }
            return ok(body);
        }
        panic!("the fake does not know {}", a.url);
    })
}

fn katalog(name: &str) -> (Katalog, String) {
    let p = std::env::temp_dir().join(format!("maer-sync-abruf-{name}-{}", std::process::id()));
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{s}", p.display()));
    }
    let path = p.to_string_lossy().into_owned();
    (Katalog::open(&path).unwrap(), path)
}

fn settings() -> Einstellungen {
    Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: 2026 }
}

fn live(k: &Katalog, quelle: &str) -> Vec<String> {
    k.full(quelle, None, MAX_PAGE).unwrap().zeilen.into_iter().map(|s| s.row.id).collect()
}

// ---------------------------------------------------------------- plan (3 to 7)

const T: i64 = 1_764_000_000;

#[test]
fn a_new_catalog_starts_with_a_full_pass() {
    assert_eq!(faellig(T, &Zustand::default()), Some(Art::Voll));
}

#[test]
fn delta_daily_full_weekly() {
    let z = Zustand { letzter_voll: Some(T), ..Default::default() };
    assert_eq!(faellig(T + DELTA_S - 1, &z), None);
    assert_eq!(faellig(T + DELTA_S, &z), Some(Art::Delta));
    let z2 = Zustand { letzter_voll: Some(T), letzter_delta: Some(T + 6 * DELTA_S), ..Default::default() };
    assert_eq!(faellig(T + 6 * DELTA_S + 100, &z2), None);
    // Both due: the full pass wins.
    assert_eq!(faellig(T + VOLL_S, &z2), Some(Art::Voll));
    assert_eq!(wann(&z), T + DELTA_S);
}

#[test]
fn sources_run_one_after_another() {
    let mut z = BTreeMap::new();
    z.insert("otr".to_string(), Zustand { letzter_voll: Some(T - VOLL_S), ..Default::default() });
    z.insert("librivox".to_string(), Zustand { letzter_voll: Some(T - VOLL_S - 10), ..Default::default() });
    z.insert("netlabel".to_string(), Zustand { letzter_voll: Some(T - VOLL_S), ..Default::default() });
    // The most overdue first, then by name.
    assert_eq!(naechster(T, &z), Some(("librivox".to_string(), Art::Voll)));
    z.get_mut("librivox").unwrap().letzter_voll = Some(T);
    assert_eq!(naechster(T, &z), Some(("netlabel".to_string(), Art::Voll)));
}

#[test]
fn next_wake_is_the_earliest_due_time() {
    let mut z = BTreeMap::new();
    z.insert("otr".to_string(), Zustand { letzter_voll: Some(T), ..Default::default() });
    assert_eq!(schlaf_s(T, &z), 3_600, "at most an hour");
    assert_eq!(schlaf_s(T + DELTA_S - 300, &z), 300);
    assert_eq!(schlaf_s(T + DELTA_S - 5, &z), 60, "at least a minute");
    assert_eq!(schlaf_s(T + DELTA_S + 5, &z), 60);
}

#[test]
fn a_failed_run_is_retried_with_backoff() {
    let mut z = Zustand { fehler: 1, letzter_fehler: Some(T), ..Default::default() };
    assert_eq!(faellig(T + BACKOFF_S[0] - 1, &z), None);
    assert_eq!(faellig(T + BACKOFF_S[0], &z), Some(Art::Voll));
    z.fehler = 2;
    assert_eq!(faellig(T + BACKOFF_S[0], &z), None);
    assert_eq!(faellig(T + BACKOFF_S[1], &z), Some(Art::Voll));
    z.fehler = 9;
    assert_eq!(faellig(T + BACKOFF_S[1], &z), None);
    assert_eq!(faellig(T + BACKOFF_S[2], &z), Some(Art::Voll));
    // A source that asked us to come back later is not asked before.
    let w = Zustand { nicht_vor: Some(T + 7_200), ..Default::default() };
    assert_eq!(faellig(T + 7_199, &w), None);
    assert_eq!(faellig(T + 7_200, &w), Some(Art::Voll));
}

// ---------------------------------------------------------------- full pass (8 to 12)

#[test]
fn full_pass_counts_first_then_pages() {
    let (mut n, log, _) = net(archive_source("librivoxaudio", ids(5, "lv"), 2));
    let (mut k, _) = katalog("count-first");
    let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    assert_eq!(b.ausgang, Ok(()));
    let urls: Vec<String> = log.borrow().iter().map(|(_, a)| a.url.clone()).collect();
    assert!(urls[0].starts_with("https://archive.org/advancedsearch.php"), "{urls:?}");
    let first: BTreeMap<String, String> = decode_params(&urls[0]).into_iter().collect();
    assert_eq!(first.get("rows").map(String::as_str), Some("0"));
    assert_eq!(first.get("q").map(String::as_str), base_query("librivox"));
    assert_eq!(urls.len(), 4, "count, then three scrape pages: {urls:?}");
    for u in &urls[1..] {
        let p: BTreeMap<String, String> = decode_params(u).into_iter().collect();
        assert!(u.starts_with("https://archive.org/services/search/v1/scrape"), "{u}");
        assert_eq!(p.get("count").map(String::as_str), Some("10000"));
    }
}

#[test]
fn full_pass_writes_when_the_count_agrees() {
    let (mut n, _, _) = net(archive_source("netlabels", ids(7, "nl"), 3));
    let (mut k, _) = katalog("agrees");
    let b = lauf(&mut n, &mut k, "netlabel", Art::Voll, &settings());
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!((b.gelesen, b.version_vorher, b.version_nachher), (7, 0, 1));
    assert_eq!(live(&k, "netlabel").len(), 7);
    assert!(k.last_run("netlabel", "voll").unwrap().is_some());
}

#[test]
fn full_pass_refuses_a_short_or_doubled_pass() {
    // The count says 6, the pages carry 5.
    let short: Handler = {
        let mut inner = archive_source("oldtimeradio", ids(5, "ot"), 10);
        Box::new(move |a, p| {
            if a.url.contains("advancedsearch") {
                let q = p.get("q").cloned().unwrap_or_default();
                return ok(adv(&q, 6, vec![]));
            }
            inner(a, p)
        })
    };
    // A record twice on two pages.
    let mut doubled_ids = ids(4, "ot");
    doubled_ids.push("ot-00001".into());
    for (name, handler) in [("short", short), ("doubled", archive_source("oldtimeradio", doubled_ids, 3))] {
        let (mut n, _, _) = net(handler);
        let (mut k, _) = katalog(name);
        let b = lauf(&mut n, &mut k, "otr", Art::Voll, &settings());
        assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{name}: {:?}", b.ausgang);
        assert_eq!(k.version("otr").unwrap(), 0, "{name}: written");
        assert_eq!(k.last_run("otr", "voll").unwrap(), None, "{name}: run time set");
    }
}

#[test]
fn a_foreign_page_stops_the_pass() {
    let mut inner = archive_source("netlabels", ids(4, "nl"), 2);
    let handler: Handler = Box::new(move |a, p| {
        if p.get("cursor").map(String::as_str) == Some("2") {
            return ok(json!({"items": [item("lv-1", "librivoxaudio"), item("lv-2", "librivoxaudio")], "count": 2}));
        }
        inner(a, p)
    });
    let (mut n, _, _) = net(handler);
    let (mut k, _) = katalog("foreign");
    let b = lauf(&mut n, &mut k, "netlabel", Art::Voll, &settings());
    assert!(matches!(b.ausgang, Err(LaufFehler::Lesen(_))), "{:?}", b.ausgang);
    assert_eq!(k.version("netlabel").unwrap(), 0);
}

#[test]
fn a_vanished_record_becomes_a_mark_only_through_a_full_pass() {
    let (mut k, _) = katalog("vanished");
    let (mut n, _, _) = net(archive_source("librivoxaudio", ids(100, "lv"), 40));
    assert_eq!(lauf(&mut n, &mut k, "librivox", Art::Voll, &settings()).ausgang, Ok(()));
    // One record gone: a deletion mark.
    let mut fewer = ids(100, "lv");
    fewer.remove(3);
    let (mut n, _, _) = net(archive_source("librivoxaudio", fewer, 40));
    assert_eq!(lauf(&mut n, &mut k, "librivox", Art::Voll, &settings()).ausgang, Ok(()));
    assert_eq!(live(&k, "librivox").len(), 99);
    // Half gone at once: the mass bound of the catalog refuses it.
    let (mut n, _, _) = net(archive_source("librivoxaudio", ids(50, "lv"), 40));
    let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    assert!(matches!(b.ausgang, Err(LaufFehler::Gesperrt { .. })), "{:?}", b.ausgang);
    assert_eq!(live(&k, "librivox").len(), 99);
}

// ---------------------------------------------------------------- delta (13 to 16)

fn delta_ready(name: &str, quelle: &str, records: Vec<String>) -> (Katalog, i64) {
    let (mut k, _) = katalog(name);
    let coll = maer_sync_abruf::archive::collection_of(quelle).unwrap();
    let (mut n, _, _) = net(archive_source(coll, records, 10));
    assert_eq!(lauf(&mut n, &mut k, quelle, Art::Voll, &settings()).ausgang, Ok(()));
    let at = k.last_run(quelle, "voll").unwrap().unwrap();
    (k, at)
}

#[test]
fn delta_asks_advancedsearch_since_the_last_run() {
    let (mut k, at) = delta_ready("delta-ask", "otr", ids(3, "ot"));
    let (mut n, log, _) = net(archive_source("oldtimeradio", ids(5, "ot"), 10));
    let b = lauf(&mut n, &mut k, "otr", Art::Delta, &Einstellungen { adv_rows: 2, ..settings() });
    assert_eq!(b.ausgang, Ok(()));
    let since = datum(at - DELTA_S);
    let urls: Vec<String> = log.borrow().iter().map(|(_, a)| a.url.clone()).collect();
    assert!(urls.iter().all(|u| u.starts_with("https://archive.org/advancedsearch.php")), "{urls:?}");
    let p: BTreeMap<String, String> = decode_params(&urls[0]).into_iter().collect();
    assert_eq!(p["q"], format!("{} AND addeddate:[{since} TO null]", base_query("otr").unwrap()));
    let pages: Vec<String> = urls.iter().map(|u| decode_params(u).into_iter().collect::<BTreeMap<_, _>>()["page"].clone()).collect();
    assert_eq!(pages, vec!["1", "2", "3"], "paged by rows and page");
}

#[test]
fn delta_checks_the_echo() {
    let (mut k, _) = delta_ready("delta-echo", "otr", ids(3, "ot"));
    let v = k.version("otr").unwrap();
    let handler: Handler = Box::new(|_, _| ok(adv("collection:something-else", 1, vec![item("ot-x", "oldtimeradio")])));
    let (mut n, _, _) = net(handler);
    let b = lauf(&mut n, &mut k, "otr", Art::Delta, &settings());
    assert!(matches!(b.ausgang, Err(LaufFehler::Echo { .. })), "{:?}", b.ausgang);
    assert_eq!(k.version("otr").unwrap(), v);
}

#[test]
fn delta_writes_through_write_delta() {
    let (mut k, _) = delta_ready("delta-write", "otr", ids(3, "ot"));
    // The delta names only two new records; the three known ones stay.
    let new: Vec<String> = vec!["ot-new-1".into(), "ot-new-2".into()];
    let (mut n, _, _) = net(archive_source("oldtimeradio", new, 10));
    let b = lauf(&mut n, &mut k, "otr", Art::Delta, &settings());
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!(live(&k, "otr").len(), 5, "a delta deleted rows");
    assert!(k.last_run("otr", "delta").unwrap().is_some());
}

#[test]
fn an_empty_delta_is_a_quiet_success() {
    let (mut k, _) = delta_ready("delta-empty", "otr", ids(3, "ot"));
    let v = k.version("otr").unwrap();
    let (mut n, _, _) = net(archive_source("oldtimeradio", vec![], 10));
    let b = lauf(&mut n, &mut k, "otr", Art::Delta, &settings());
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!(k.version("otr").unwrap(), v);
    assert!(k.last_run("otr", "delta").unwrap().is_some());
}

// ---------------------------------------------------------------- shellac (17, 18)

fn shelf_source(per_shelf: BTreeMap<String, Vec<String>>) -> Handler {
    Box::new(move |a, p| {
        assert!(a.url.starts_with("https://archive.org/advancedsearch.php"), "shellac asked {}", a.url);
        let q = p.get("q").cloned().unwrap_or_default();
        let word = q.split("subject:\"").nth(1).and_then(|r| r.split('"').next())
            .unwrap_or_else(|| panic!("a shellac query without a shelf: {q}"));
        let records = per_shelf.get(word).cloned().unwrap_or_default();
        let rows: usize = p.get("rows").and_then(|r| r.parse().ok()).unwrap_or(0);
        let page: usize = p.get("page").and_then(|r| r.parse().ok()).unwrap_or(1);
        let docs: Vec<Value> = records.iter().skip((page - 1) * rows).take(rows).map(|i| {
            json!({"identifier": i, "title": i, "collection": ["georgeblood"], "subject": ["78rpm", word], "year": 1920})
        }).collect();
        ok(adv(&q, records.len(), docs))
    })
}

#[test]
fn shellac_reads_every_shelf_and_never_the_collection() {
    let mut shelves = BTreeMap::new();
    shelves.insert("jazz".to_string(), vec!["78_a".to_string(), "78_both".to_string()]);
    shelves.insert("swing".to_string(), vec!["78_both".to_string()]);
    shelves.insert("tango".to_string(), vec!["78_t".to_string()]);
    let (mut n, log, _) = net(shelf_source(shelves));
    let (mut k, _) = katalog("shellac");
    let b = lauf(&mut n, &mut k, "schellack", Art::Voll, &settings());
    assert_eq!(b.ausgang, Ok(()));
    let asked: Vec<String> = log.borrow().iter()
        .map(|(_, a)| decode_params(&a.url).into_iter().collect::<BTreeMap<_, _>>()["q"].clone()).collect();
    let mut queries: Vec<String> = regale_der_app().into_iter()
        .map(|s| format!("collection:georgeblood AND subject:\"{}\" AND mediatype:audio", s.schlagwort)).collect();
    queries.sort();
    let mut seen = asked.clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen, queries, "exactly the nine shelves");
    let rows = k.full("schellack", None, MAX_PAGE).unwrap().zeilen;
    let both = rows.iter().find(|s| s.row.id == "lva:78_both").unwrap();
    assert_eq!(both.row.reihen, vec!["shellac-jazz".to_string(), "shellac-swing".to_string()]);
    assert_eq!(rows.len(), 3);
}

#[test]
fn a_shelf_larger_than_a_page_is_paged() {
    let mut shelves = BTreeMap::new();
    shelves.insert("jazz".to_string(), (0..7).map(|i| format!("78_j{i}")).collect::<Vec<_>>());
    let (mut n, _, _) = net(shelf_source(shelves));
    let (mut k, _) = katalog("shelf-pages");
    let b = lauf(&mut n, &mut k, "schellack", Art::Voll, &Einstellungen { adv_rows: 3, ..settings() });
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!(live(&k, "schellack").len(), 7);
}

// ---------------------------------------------------------------- network (19 to 22, 26, 27)

#[test]
fn every_request_names_the_client() {
    let (mut n, log, _) = net(archive_source("librivoxaudio", ids(3, "lv"), 1));
    let (mut k, _) = katalog("ua");
    lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    assert!(!log.borrow().is_empty());
    for (_, a) in log.borrow().iter() {
        assert_eq!(a.user_agent, USER_AGENT);
        assert_eq!(a.user_agent, "Maer/1.0 (+https://maer.fm)");
        assert_eq!(a.max_bytes, MAX_BYTES);
    }
}

#[test]
fn requests_are_spaced() {
    let (mut n, log, _) = net(archive_source("librivoxaudio", ids(6, "lv"), 1));
    let (mut k, _) = katalog("spaced");
    lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    let times: Vec<i64> = log.borrow().iter().map(|(t, _)| *t).collect();
    assert!(times.len() >= 7);
    for w in times.windows(2) {
        assert!(w[1] - w[0] >= MIN_GAP_MS, "{times:?}");
    }
}

#[test]
fn a_server_error_writes_nothing() {
    for (name, answer) in [
        ("503", Ok(Antwort { status: 503, ..Default::default() })),
        ("timeout", Err(HolFehler::Zeit)),
        ("429", Ok(Antwort { status: 429, retry_after_s: Some(7_200), ..Default::default() })),
    ] {
        let a2 = answer.clone();
        let handler: Handler = Box::new(move |_, _| a2.clone());
        let (mut n, _, _) = net(handler);
        let (mut k, _) = katalog(&format!("err-{name}"));
        let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
        assert!(matches!(b.ausgang, Err(LaufFehler::Holen(_))), "{name}: {:?}", b.ausgang);
        assert_eq!(k.version("librivox").unwrap(), 0, "{name}");
        if name == "429" {
            assert_eq!(b.ausgang, Err(LaufFehler::Holen(HolFehler::SpaeterWieder(7_200))));
        }
    }
}

#[test]
#[should_panic(expected = "the fake does not know")]
fn no_witness_reaches_the_network() {
    let (mut n, _, _) = net(archive_source("librivoxaudio", ids(1, "lv"), 1));
    let _ = n.get("https://example.org/anything");
}

#[test]
fn an_answer_over_the_bound_is_an_error() {
    let big: Handler = Box::new(|_, _| Ok(Antwort { status: 200, koerper: vec![b' '; MAX_BYTES + 1], ..Default::default() }));
    let (mut n, _, _) = net(big);
    assert_eq!(n.get("https://archive.org/x"), Err(HolFehler::ZuGross(MAX_BYTES + 1)));
    let slow: Handler = Box::new(|_, _| Err(HolFehler::Zeit));
    let (mut n, log, _) = net(slow);
    assert_eq!(n.get("https://archive.org/x"), Err(HolFehler::Zeit));
    assert!(log.borrow().iter().all(|(_, a)| a.timeout_s > 0 && a.timeout_s <= 300));
}

#[test]
fn redirects_are_bounded_and_never_leave_https() {
    let to_http: Handler = Box::new(|a, _| {
        if a.url == "https://archive.org/a" {
            return Ok(Antwort { status: 301, location: Some("http://archive.org/b".into()), ..Default::default() });
        }
        panic!("the fake does not know {}", a.url);
    });
    let (mut n, _, _) = net(to_http);
    assert!(matches!(n.get("https://archive.org/a"), Err(HolFehler::UnsichereWeiterleitung(_))));
    let looping: Handler = Box::new(|a, _| {
        let k: usize = a.url.rsplit('/').next().unwrap().parse().unwrap();
        Ok(Antwort { status: 302, location: Some(format!("/{}", k + 1)), ..Default::default() })
    });
    let (mut n, log, _) = net(looping);
    assert_eq!(n.get("https://archive.org/0"), Err(HolFehler::ZuVieleWeiterleitungen));
    assert_eq!(log.borrow().len(), MAX_REDIRECTS + 1);
    let followed: Handler = Box::new(|a, _| match a.url.as_str() {
        "https://archive.org/old" => Ok(Antwort { status: 308, location: Some("https://archive.org/new".into()), ..Default::default() }),
        "https://archive.org/new" => ok(json!({"ok": true})),
        u => panic!("the fake does not know {u}"),
    });
    let (mut n, _, _) = net(followed);
    assert_eq!(n.get("https://archive.org/old").unwrap().status, 200);
    let (mut n, _, _) = net(Box::new(|a, _| panic!("the fake does not know {}", a.url)));
    assert!(matches!(n.get("http://archive.org/plain"), Err(HolFehler::NichtHttps(_))));
}

// ---------------------------------------------------------------- 28: an end in the middle

#[test]
fn an_end_between_pages_writes_nothing() {
    let mut inner = archive_source("librivoxaudio", ids(6, "lv"), 2);
    let mut pages = 0;
    let handler: Handler = Box::new(move |a, p| {
        if a.url.contains("/scrape") {
            pages += 1;
            if pages == 3 {
                return Err(HolFehler::Verbindung("the program ends".into()));
            }
        }
        inner(a, p)
    });
    let (mut n, _, _) = net(handler);
    let (mut k, path) = katalog("end");
    let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    assert!(b.ausgang.is_err());
    assert_eq!(k.version("librivox").unwrap(), 0);
    assert_eq!(k.last_run("librivox", "voll").unwrap(), None);
    // The next start begins the pass again: still a full pass, due now.
    drop(k);
    let k = Katalog::open(&path).unwrap();
    let z = Zustand { letzter_voll: k.last_run("librivox", "voll").unwrap(), ..Default::default() };
    assert_eq!(faellig(T, &z), Some(Art::Voll));
}

fn shellac_row(id: &str, reihen: &[&str]) -> Row {
    Row {
        quelle: "schellack".into(), id: id.into(), art: "musik".into(), titel: None, urheber: None,
        sprache: None, jahr: None, rechte_kennung: "pd".into(), rechte_url: None, laden: true,
        reihen: reihen.iter().map(|s| s.to_string()).collect(), auch: vec![], tief: None, beliebt_stufe: None,
    }
}

#[test]
fn a_shellac_delta_keeps_the_shelves_the_catalog_knows() {
    let mut shelves = BTreeMap::new();
    shelves.insert("jazz".to_string(), vec!["78_both".to_string()]);
    shelves.insert("swing".to_string(), vec!["78_both".to_string()]);
    let (mut n, _, _) = net(shelf_source(shelves));
    let (mut k, _) = katalog("shellac-delta-keeps");
    assert_eq!(lauf(&mut n, &mut k, "schellack", Art::Voll, &settings()).ausgang, Ok(()));
    // The delta finds the record again, on a third shelf only.
    let mut again = BTreeMap::new();
    again.insert("tango".to_string(), vec!["78_both".to_string()]);
    let (mut n, _, _) = net(shelf_source(again));
    assert_eq!(lauf(&mut n, &mut k, "schellack", Art::Delta, &settings()).ausgang, Ok(()));
    let row = k.stored("schellack", "lva:78_both").unwrap().unwrap().row;
    assert_eq!(row.reihen, vec!["shellac-jazz", "shellac-swing", "shellac-tango"]);
}

#[test]
fn a_shellac_delta_refuses_when_a_known_row_cannot_be_read() {
    // A read that fails is not a row that was never there: taken as unknown,
    // the record would lose its shelves in the catalog.
    let mut rows = vec![shellac_row("lva:78_a", &["shellac-tango"]), shellac_row("lva:78_b", &["shellac-tango"])];
    let r = keep_known_shelves(&mut rows, |id| match id {
        "lva:78_a" => Ok(Some(vec!["shellac-jazz".to_string()])),
        _ => Err(KatalogError::Busy),
    });
    assert!(matches!(r, Err(LaufFehler::Katalog(_))), "{r:?}");
}

// ---------------------------------------------------------------- the bound on losses and its way out

fn librivox_with(k: &mut Katalog, records: Vec<String>) -> maer_sync_abruf::lauf::Laufbericht {
    let (mut n, _, _) = net(archive_source("librivoxaudio", records, 40));
    lauf(&mut n, k, "librivox", Art::Voll, &settings())
}

#[test]
fn a_refused_run_names_the_numbers_and_the_way_out() {
    let (mut k, _) = katalog("bound-line");
    assert_eq!(librivox_with(&mut k, ids(100, "lv")).ausgang, Ok(()));
    let b = librivox_with(&mut k, ids(50, "lv"));
    assert!(matches!(b.ausgang, Err(LaufFehler::Gesperrt { lebend: 100, .. })), "{:?}", b.ausgang);
    assert!(!b.bestaetigt);
    let line = b.zeile();
    for part in ["gelesen=50", "lebend=100", "bestaetigen=\"maer-sync-abruf --bestaetigt librivox\""] {
        assert!(line.contains(part), "{line}");
    }
    assert_eq!(live(&k, "librivox").len(), 100);
}

#[test]
fn a_confirmed_run_writes_what_the_bound_refused_once() {
    let (mut k, _) = katalog("bound-confirmed");
    assert_eq!(librivox_with(&mut k, ids(100, "lv")).ausgang, Ok(()));
    let v = k.version("librivox").unwrap();
    let (mut n, _, _) = net(archive_source("librivoxaudio", ids(50, "lv"), 40));
    let b = lauf_bestaetigt(&mut n, &mut k, "librivox", &settings());
    assert_eq!(b.ausgang, Ok(()));
    assert!(b.bestaetigt && b.art == Art::Voll, "{b:?}");
    assert!(b.zeile().contains("bestaetigt=ja"), "{}", b.zeile());
    assert_eq!(live(&k, "librivox").len(), 50);
    assert!(k.version("librivox").unwrap() > v);
    assert!(k.last_run("librivox", "voll").unwrap().is_some());
    // The sign held for that one run: the next large loss is refused again.
    let b = librivox_with(&mut k, ids(10, "lv"));
    assert!(matches!(b.ausgang, Err(LaufFehler::Gesperrt { lebend: 50, .. })), "{:?}", b.ausgang);
    assert!(!b.zeile().contains("bestaetigt=ja"), "{}", b.zeile());
    assert_eq!(live(&k, "librivox").len(), 50);
}

#[test]
fn an_empty_source_is_refused_and_confirmed_the_same_way() {
    let (mut k, _) = katalog("bound-empty");
    assert_eq!(librivox_with(&mut k, ids(100, "lv")).ausgang, Ok(()));
    let b = librivox_with(&mut k, vec![]);
    assert!(matches!(b.ausgang, Err(LaufFehler::Gesperrt { lebend: 100, .. })), "{:?}", b.ausgang);
    assert!(b.zeile().contains("gelesen=0"), "{}", b.zeile());
    let (mut n, _, _) = net(archive_source("librivoxaudio", vec![], 40));
    assert_eq!(lauf_bestaetigt(&mut n, &mut k, "librivox", &settings()).ausgang, Ok(()));
    assert_eq!(live(&k, "librivox").len(), 0);
}

#[test]
fn the_program_takes_one_argument_to_confirm() {
    let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(befehl(&a(&[])), Ok(Befehl::Dauerlauf));
    assert_eq!(befehl(&a(&["--bestaetigt", "schellack"])), Ok(Befehl::Bestaetigt("schellack".into())));
    let bad: [&[&str]; 5] = [
        &["--bestaetigt"], &["--bestaetigt", "hoerspiel-feed"], &["--bestaetigt", "librivox", "otr"],
        &["librivox"], &["--confirm", "librivox"],
    ];
    for b in bad {
        assert!(befehl(&a(b)).is_err(), "{b:?}");
    }
}

#[test]
fn the_program_ends_on_an_argument_it_does_not_know() {
    let dir = std::env::temp_dir().join(format!("maer-sync-abruf-args-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A folder that is missing: a program that opened the catalog first
    // would end with 1 there, never reach the network, and never pass.
    let path = dir.join("fehlt").join("katalog.db");
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_maer-sync-abruf"))
        .args(["--bestaetigt", "hoerspiel-feed"])
        .env("MAER_SYNC_KATALOG", &path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn().unwrap();
    // It must end on its own, before it opens the catalog.
    let mut status = None;
    for _ in 0..100 {
        if let Some(s) = child.try_wait().unwrap() {
            status = Some(s);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let Some(status) = status else {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("the program kept running on an unknown argument");
    };
    let mut err = String::new();
    std::io::Read::read_to_string(child.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert_eq!(status.code(), Some(2), "the arguments are read before the catalog is opened: {err}");
    assert!(err.contains("--bestaetigt"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------- the line of a run

#[test]
fn each_run_leaves_one_line() {
    let (mut n, _, _) = net(archive_source("librivoxaudio", ids(3, "lv"), 10));
    let (mut k, _) = katalog("line");
    let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    let line = b.zeile();
    assert!(!line.contains('\n'));
    for part in ["quelle=librivox", "art=voll", "gelesen=3", "version=0->1", "ausgang=ok"] {
        assert!(line.contains(part), "{line}");
    }
    assert!(!line.contains("Title"), "the line carries answer text: {line}");
    let (mut n, _, _) = net(Box::new(|_, _| Err(HolFehler::Zeit)));
    let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &settings());
    let line = b.zeile();
    assert!(line.contains("ausgang=abgewiesen") && line.contains("Zeit"), "{line}");
}

#[test]
fn dates_are_utc_days() {
    assert_eq!(datum(0), "1970-01-01");
    assert_eq!(datum(1_764_000_000), "2025-11-24");
    assert_eq!(datum(951_782_400), "2000-02-29");
}
