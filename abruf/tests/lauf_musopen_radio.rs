// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Runs of Musopen and Radio Browser against a fake network: counting,
//! paging, the client name, maxlag, the server list, and that a delta of
//! these two sources is a full pass. The fake panics on any address it does
//! not know, a station's own address among them.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use maer_sync::katalog::{Katalog, MAX_PAGE};
use maer_sync_abruf::lauf::{lauf, Einstellungen, LaufFehler};
use maer_sync_abruf::netz::{decode_params, gewichtigster, Anfrage, Antwort, HolFehler, Holer, Netz, Uhr, USER_AGENT};
use maer_sync_abruf::plan::Art;
use maer_sync_abruf::programm::QUELLEN;
use serde_json::{json, Value};

type Handler = Box<dyn FnMut(&Anfrage, &BTreeMap<String, String>) -> Result<Antwort, HolFehler>>;

#[derive(Clone)]
struct Clock(Rc<RefCell<i64>>);
impl Uhr for Clock {
    fn jetzt_ms(&self) -> i64 { *self.0.borrow() }
    fn schlafe_ms(&mut self, ms: i64) { *self.0.borrow_mut() += ms.max(0); }
}

struct Fake { clock: Clock, log: Rc<RefCell<Vec<Anfrage>>>, handler: Handler }
impl Holer for Fake {
    fn hole(&mut self, a: &Anfrage) -> Result<Antwort, HolFehler> {
        self.log.borrow_mut().push(a.clone());
        *self.clock.0.borrow_mut() += 50;
        let p: BTreeMap<String, String> = decode_params(&a.url).into_iter().collect();
        (self.handler)(a, &p)
    }
}

fn net(h: Handler) -> (Netz<Fake, Clock>, Rc<RefCell<Vec<Anfrage>>>) {
    let clock = Clock(Rc::new(RefCell::new(1_764_000_000_000)));
    let log = Rc::new(RefCell::new(vec![]));
    (Netz::new(Fake { clock: clock.clone(), log: log.clone(), handler: h }, clock), log)
}

fn ok(v: Value) -> Result<Antwort, HolFehler> {
    Ok(Antwort { status: 200, koerper: v.to_string().into_bytes(), ..Default::default() })
}

fn katalog(name: &str) -> Katalog {
    let p = std::env::temp_dir().join(format!("maer-sync-mr-{name}-{}", std::process::id()));
    for s in ["", "-wal", "-shm"] { let _ = std::fs::remove_file(format!("{}{s}", p.display())); }
    Katalog::open(p.to_str().unwrap()).unwrap()
}

fn e(rows: usize) -> Einstellungen { Einstellungen { scrape_count: 10_000, adv_rows: rows, kalenderjahr: 2026 } }

fn live(k: &Katalog, q: &str) -> Vec<String> {
    k.full(q, None, MAX_PAGE).unwrap().zeilen.into_iter().map(|s| s.row.id).collect()
}

#[test]
fn seven_sources_are_planned() {
    assert_eq!(QUELLEN, ["archive-buch", "librivox", "musopen", "netlabel", "otr", "radio", "schellack"]);
}

// ---------------------------------------------------------------- Musopen

fn file(id: i64) -> Value {
    json!({"pageid": id, "ns": 6, "title": format!("File:Composer - Work - {}. Part.ogg", id % 5 + 1),
           "videoinfo": [{"url": format!("https://upload.wikimedia.org/x/{id}.ogg"), "mime": "application/ogg",
                          "derivatives": [{"src": format!("https://upload.wikimedia.org/t/{id}.ogg.mp3"), "type": "audio/mpeg", "transcodekey": "mp3"}],
                          "extmetadata": {"License": {"value": "pd"}, "LicenseShortName": {"value": "Public domain"}}}]})
}

/// Two categories; `said` overrides the count of a category.
fn commons(cats: BTreeMap<&'static str, Vec<i64>>, said: BTreeMap<&'static str, usize>) -> Handler {
    Box::new(move |a, p| {
        assert!(a.url.starts_with("https://commons.wikimedia.org/w/api.php"), "the fake does not know {}", a.url);
        if p.get("prop").map(String::as_str) == Some("categoryinfo") {
            let t = p["titles"].clone();
            let n = said.get(t.as_str()).copied().unwrap_or_else(|| cats[t.as_str()].len());
            return ok(json!({"query": {"pages": [{"title": t, "categoryinfo": {"files": n}}]}}));
        }
        let t = p["gcmtitle"].clone();
        let all = &cats[t.as_str()];
        let start: usize = p.get("gcmcontinue").map(|c| c.parse().unwrap()).unwrap_or(0);
        let limit: usize = p["gcmlimit"].parse().unwrap();
        let pages: Vec<Value> = all.iter().skip(start).take(limit).map(|i| file(*i)).collect();
        let mut v = json!({"query": {"pages": pages}});
        if start + limit < all.len() { v["continue"] = json!({"gcmcontinue": (start + limit).to_string()}); }
        ok(v)
    })
}

fn two() -> BTreeMap<&'static str, Vec<i64>> {
    let mut m = BTreeMap::new();
    m.insert("Category:Musopen", vec![1, 2, 3, 4, 5, 6, 7]);
    m.insert("Category:Set Chopin Free", vec![7, 8]);
    m
}

#[test]
fn musopen_reads_both_categories_once_each_file() {
    let (mut n, _) = net(commons(two(), BTreeMap::new()));
    let mut k = katalog("mus-both");
    let b = lauf(&mut n, &mut k, "musopen", Art::Voll, &e(3));
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!(live(&k, "musopen").len(), 8, "file 7 stands in both categories and is one row");
}

#[test]
fn musopen_a_short_category_writes_nothing() {
    let mut said = BTreeMap::new();
    said.insert("Category:Musopen", 9);
    let (mut n, _) = net(commons(two(), said));
    let mut k = katalog("mus-short");
    let b = lauf(&mut n, &mut k, "musopen", Art::Voll, &e(3));
    assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{:?}", b.ausgang);
    assert_eq!(k.version("musopen").unwrap(), 0);
}

#[test]
fn musopen_every_request_names_the_client_and_asks_maxlag() {
    let (mut n, log) = net(commons(two(), BTreeMap::new()));
    let mut k = katalog("mus-ua");
    lauf(&mut n, &mut k, "musopen", Art::Voll, &e(3));
    assert!(log.borrow().len() >= 6);
    for a in log.borrow().iter() {
        assert_eq!(a.user_agent, USER_AGENT);
        let p: BTreeMap<String, String> = decode_params(&a.url).into_iter().collect();
        assert_eq!(p.get("maxlag").map(String::as_str), Some("5"), "{}", a.url);
        assert_eq!(p.get("format").map(String::as_str), Some("json"));
        if p.get("generator").map(String::as_str) == Some("categorymembers") {
            // The same fields as the app asks (iOS `MusopenCatalog.fileFields`).
            assert_eq!(p.get("prop").map(String::as_str), Some("videoinfo"));
            assert_eq!(p.get("viprop").map(String::as_str), Some("url|size|mime|derivatives|extmetadata"), "{}", a.url);
        }
    }
}

#[test]
fn musopen_maxlag_postpones_the_run() {
    let h: Handler = Box::new(|_, _| ok(json!({"error": {"code": "maxlag", "info": "lagged", "lag": 12}})));
    let (mut n, _) = net(h);
    let mut k = katalog("mus-lag");
    let b = lauf(&mut n, &mut k, "musopen", Art::Voll, &e(3));
    assert_eq!(b.ausgang, Err(LaufFehler::Holen(HolFehler::SpaeterWieder(12))));
}

#[test]
fn a_delta_of_musopen_is_a_full_pass() {
    let mut k = katalog("mus-delta");
    let (mut n, _) = net(commons(two(), BTreeMap::new()));
    assert_eq!(lauf(&mut n, &mut k, "musopen", Art::Voll, &e(3)).ausgang, Ok(()));
    let mut fewer = two();
    fewer.get_mut("Category:Musopen").unwrap().retain(|i| *i != 2);
    let (mut n, log) = net(commons(fewer, BTreeMap::new()));
    assert_eq!(lauf(&mut n, &mut k, "musopen", Art::Delta, &e(3)).ausgang, Ok(()));
    assert!(log.borrow().iter().any(|a| a.url.contains("categoryinfo")), "the delta did not count like a full pass");
    assert_eq!(live(&k, "musopen").len(), 7, "a file gone from Commons stays live after a delta");
}

// ---------------------------------------------------------------- Radio Browser

fn station(i: usize) -> Value {
    json!({"stationuuid": format!("00000000-0000-0000-0000-{i:012}"), "name": format!("Station {i}"), "url": format!("http://stream.example/{i}"),
           "url_resolved": format!("http://stream.example/{i}"), "favicon": "", "homepage": "", "countrycode": "DE", "tags": "jazz",
           "bitrate": 128, "codec": "MP3", "votes": 3, "languagecodes": "de", "state": "", "clickcount": 1, "lastcheckok": 1})
}

/// The servers that resolve (measured 26.11.2026: `de1` and `all`, one
/// machine; `at1` and `nl1` no longer resolve).
const RESOLVING: [&str; 2] = ["de1.", "all."];

/// A Radio Browser server; `down` servers fail, a name outside `RESOLVING`
/// does not resolve. Its statistics as the real
/// API states them: `stations` counts the stations that are not broken,
/// exactly what the pages with `hidebroken=true` list, and `stations_broken`
/// the others (de1, 25.11.2026: 53 393 listed, 53 393 stations, 330 broken).
fn radio_api(stations: Rc<RefCell<Vec<Value>>>, stats: Rc<RefCell<(usize, usize)>>, down: Vec<&'static str>) -> Handler {
    Box::new(move |a, p| {
        let host = a.url.split('/').nth(2).unwrap_or("").to_string();
        assert!(host.ends_with(".api.radio-browser.info"), "the fake does not know {} (a station address?)", a.url);
        if !RESOLVING.iter().any(|d| host.starts_with(d)) { return Err(HolFehler::WirtUnbekannt); }
        if down.iter().any(|d| host.starts_with(d)) { return Err(HolFehler::Verbindung("down".into())); }
        if a.url.contains("/json/stats") {
            let (s, b) = *stats.borrow();
            return ok(json!({"stations": s, "stations_broken": b, "status": "OK"}));
        }
        assert!(a.url.contains("/json/stations?"), "{}", a.url);
        assert_eq!(p.get("hidebroken").map(String::as_str), Some("true"));
        let offset: usize = p["offset"].parse().unwrap();
        let limit: usize = p["limit"].parse().unwrap();
        ok(Value::Array(stations.borrow().iter().skip(offset).take(limit).cloned().collect()))
    })
}

#[test]
fn radio_pages_to_the_end_and_counts() {
    let st = Rc::new(RefCell::new((0..7).map(station).collect::<Vec<_>>()));
    let (mut n, log) = net(radio_api(st, Rc::new(RefCell::new((7, 330))), vec![]));
    let mut k = katalog("radio-pages");
    let b = lauf(&mut n, &mut k, "radio", Art::Voll, &e(3));
    assert_eq!(b.ausgang, Ok(()));
    assert_eq!(live(&k, "radio").len(), 7);
    let pages = log.borrow().iter().filter(|a| a.url.contains("/json/stations?")).count();
    assert_eq!(pages, 3, "pages of 3: 3, 3, 1");
    for a in log.borrow().iter() {
        assert_eq!(a.user_agent, USER_AGENT);
        assert!(a.url.starts_with("https://"));
    }
}

#[test]
fn radio_a_count_that_does_not_match_writes_nothing() {
    let st = Rc::new(RefCell::new((0..7).map(station).collect::<Vec<_>>()));
    let (mut n, _) = net(radio_api(st, Rc::new(RefCell::new((9, 1))), vec![]));
    let mut k = katalog("radio-count");
    let b = lauf(&mut n, &mut k, "radio", Art::Voll, &e(3));
    assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{:?}", b.ausgang);
    assert_eq!(k.version("radio").unwrap(), 0);
}

#[test]
fn radio_a_uuid_on_two_pages_repeats_then_refuses() {
    // A station that moves between pages while paging: same uuid on page 1 and 2.
    let mut v: Vec<Value> = (0..6).map(station).collect();
    v[3] = station(2);
    let st = Rc::new(RefCell::new(v));
    let (mut n, log) = net(radio_api(st, Rc::new(RefCell::new((6, 0))), vec![]));
    let mut k = katalog("radio-twice");
    let b = lauf(&mut n, &mut k, "radio", Art::Voll, &e(3));
    assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{:?}", b.ausgang);
    let passes = log.borrow().iter().filter(|a| a.url.contains("offset=0")).count();
    assert_eq!(passes, 2, "the pass was not repeated once");
    assert_eq!(k.version("radio").unwrap(), 0);
}

#[test]
fn radio_falls_back_to_the_next_server_of_the_list() {
    let st = Rc::new(RefCell::new((0..2).map(station).collect::<Vec<_>>()));
    let (mut n, log) = net(radio_api(st, Rc::new(RefCell::new((2, 0))), vec!["de1."]));
    let mut k = katalog("radio-next");
    assert_eq!(lauf(&mut n, &mut k, "radio", Art::Voll, &e(3)).ausgang, Ok(()));
    let hosts: Vec<String> = log.borrow().iter().map(|a| a.url.split('/').nth(2).unwrap().to_string()).collect();
    assert_eq!(hosts[0], "de1.api.radio-browser.info");
    assert!(hosts[1..].iter().all(|h| h == "all.api.radio-browser.info"), "{hosts:?}");
}

/// When every server of the list fails, the run is refused and the stations
/// of the last good run stay as they were.
#[test]
fn radio_servers_as_the_app() {
    // iOS dev 4887e253, `RadioBrowserClient.mirrors`.
    assert_eq!(&maer_sync_abruf::radio::SERVER[..], &["https://de1.api.radio-browser.info", "https://all.api.radio-browser.info"][..]);
}

#[test]
fn the_weightiest_error_counts() {
    // As the app (`RadioBrowserClient.weightiest`, SourceOutageTests): a
    // server that answered says most, one that timed out or refused more
    // than a host not found; on a tie the first of the list.
    use HolFehler::*;
    let fall: [(Vec<HolFehler>, HolFehler); 5] = [
        (vec![Status(503), WirtUnbekannt], Status(503)),
        (vec![WirtUnbekannt, Zeit], Zeit),
        (vec![Zeit, Status(503)], Status(503)),
        (vec![Status(500), Status(502)], Status(500)),
        (vec![Verbindung("refused".into()), Zeit], Verbindung("refused".into())),
    ];
    for (fehler, erwartet) in fall {
        assert_eq!(gewichtigster(fehler.clone()), erwartet, "{fehler:?}");
    }
}

#[test]
fn radio_reports_the_weightiest_error() {
    // de1 runs out of time, the fallback does not resolve: the line names
    // the time, not the missing name of the fallback.
    let handler: Handler = Box::new(|a, _| {
        let host = a.url.split('/').nth(2).unwrap_or("").to_string();
        if host.starts_with("de1.") { return Err(HolFehler::Zeit); }
        Err(HolFehler::WirtUnbekannt)
    });
    let (mut n, _) = net(handler);
    let mut k = katalog("radio-weightiest");
    let b = lauf(&mut n, &mut k, "radio", Art::Voll, &e(3));
    assert_eq!(b.ausgang, Err(LaufFehler::Holen(HolFehler::Zeit)), "{}", b.zeile());
}

#[test]
fn radio_all_servers_fail_and_the_catalog_stays() {
    let st = Rc::new(RefCell::new((0..3).map(station).collect::<Vec<_>>()));
    let stats = Rc::new(RefCell::new((3, 0)));
    let (mut n, _) = net(radio_api(st.clone(), stats.clone(), vec![]));
    let mut k = katalog("radio-all-down");
    assert_eq!(lauf(&mut n, &mut k, "radio", Art::Voll, &e(3)).ausgang, Ok(()));
    let (mut down, log) = net(radio_api(st, stats, vec!["de1.", "all."]));
    let b = lauf(&mut down, &mut k, "radio", Art::Voll, &e(3));
    assert!(matches!(b.ausgang, Err(LaufFehler::Holen(_))), "{:?}", b.ausgang);
    let hosts: Vec<String> = log.borrow().iter().map(|a| a.url.split('/').nth(2).unwrap().to_string()).collect();
    assert_eq!(hosts, ["de1.api.radio-browser.info", "all.api.radio-browser.info"]);
    assert_eq!(k.version("radio").unwrap(), 1, "a refused run wrote");
    assert_eq!(live(&k, "radio").len(), 3, "the stations of the last run are gone");
}

#[test]
fn a_delta_of_radio_is_a_full_pass() {
    let st = Rc::new(RefCell::new((0..4).map(station).collect::<Vec<_>>()));
    let stats = Rc::new(RefCell::new((4, 0)));
    let (mut n, _) = net(radio_api(st.clone(), stats.clone(), vec![]));
    let mut k = katalog("radio-delta");
    assert_eq!(lauf(&mut n, &mut k, "radio", Art::Voll, &e(3)).ausgang, Ok(()));
    st.borrow_mut().remove(1);
    *stats.borrow_mut() = (3, 0);
    assert_eq!(lauf(&mut n, &mut k, "radio", Art::Delta, &e(3)).ausgang, Ok(()));
    assert_eq!(live(&k, "radio").len(), 3, "a station gone stays live after a delta");
}
