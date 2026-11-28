// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The three findings of the first runs on a host (28.11.2026).
//!
//! Echo: archive.org states the query back without the parentheses around a
//! single value and with the quotes as `&quot;` (measured on all twelve
//! queries; the vector `schellack/ohne-jahr` carries the same). The fakes of
//! the older witnesses echoed the query as asked, so none of them saw it.
//!
//! Radio Browser: the statistics count one station fewer than the pages list,
//! in two runs one after the other; the list did not move. A list may stand
//! off its count by a thousandth, either way.
//!
//! Gutenberg: the mirror delivers 23 to 31 KB/s, so the catalog dump of about
//! 21 MB needs some fifteen minutes; `www.gutenberg.org` answers 503 to it.
//! The dump gets a limit of its own, `www` is asked first, and a run that saw
//! `www` fail asks the mirror for the rest of that run.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use maer_sync::katalog::{Katalog, MAX_PAGE};
use maer_sync_abruf::lauf::{base_query, gutenberg_rdf_url, lauf, Einstellungen, LaufFehler, GUTENBERG_KATALOG};
use maer_sync_abruf::netz::{decode_params, Anfrage, Antwort, HolFehler, Holer, Netz, Uhr};
use maer_sync_abruf::plan::Art;
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

fn net(h: Handler) -> (Netz<Fake, Clock>, Rc<RefCell<Vec<Anfrage>>>, Clock) {
    let clock = Clock(Rc::new(RefCell::new(1_764_000_000_000)));
    let log = Rc::new(RefCell::new(vec![]));
    (Netz::new(Fake { clock: clock.clone(), log: log.clone(), handler: h }, clock.clone()), log, clock)
}

fn ok(v: Value) -> Result<Antwort, HolFehler> {
    Ok(Antwort { status: 200, koerper: v.to_string().into_bytes(), ..Default::default() })
}

fn katalog(name: &str) -> Katalog {
    let p = std::env::temp_dir().join(format!("maer-sync-befunde-{name}-{}", std::process::id()));
    for s in ["", "-wal", "-shm"] { let _ = std::fs::remove_file(format!("{}{s}", p.display())); }
    Katalog::open(p.to_str().unwrap()).unwrap()
}

fn e(rows: usize) -> Einstellungen { Einstellungen { scrape_count: 10_000, adv_rows: rows, kalenderjahr: 2026 } }

fn live(k: &Katalog, q: &str) -> usize { k.full(q, None, MAX_PAGE).unwrap().zeilen.len() }

// ---------------------------------------------------------------- echo

/// The echo as archive.org states it (measured 28.11.2026): no parentheses
/// around `audio`, the quotes masked.
fn wie_archive(q: &str) -> String {
    q.replace("mediatype:(audio)", "mediatype:audio").replace('"', "&quot;")
}

/// archive.org for every collection and shelf: the count and the pages of
/// advancedsearch, the scrape pages, each echo through `echo`.
fn archive(echo: fn(&str) -> String) -> Handler {
    Box::new(move |a, p| {
        let q = p.get("q").cloned().unwrap_or_default();
        let ids: Vec<String> = (0..3).map(|i| format!("r-{i}")).collect();
        let coll = q.split("collection:").nth(1).and_then(|r| r.split(char::is_whitespace).next()).unwrap_or("").to_string();
        let word = q.split("subject:\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("jazz").to_string();
        let item = |i: &String| json!({"identifier": i, "title": i, "collection": [coll], "subject": ["78rpm", word], "year": 1920});
        if a.url.starts_with("https://archive.org/advancedsearch.php") {
            let rows: usize = p.get("rows").and_then(|r| r.parse().ok()).unwrap_or(0);
            let page: usize = p.get("page").and_then(|r| r.parse().ok()).unwrap_or(1);
            let docs: Vec<Value> = ids.iter().skip((page - 1) * rows).take(rows).map(item).collect();
            return ok(json!({"responseHeader": {"status": 0, "params": {"query": echo(&q), "qin": q}},
                             "response": {"numFound": ids.len(), "start": 0, "docs": docs}}));
        }
        if a.url.starts_with("https://archive.org/services/search/v1/scrape") {
            let items: Vec<Value> = ids.iter().map(item).collect();
            return ok(json!({"items": items, "count": ids.len(), "total": ids.len()}));
        }
        panic!("the fake does not know {}", a.url);
    })
}

fn asked(log: &Rc<RefCell<Vec<Anfrage>>>) -> Vec<String> {
    log.borrow().iter().filter_map(|a| decode_params(&a.url).into_iter().find(|(k, _)| k == "q").map(|(_, v)| v)).collect()
}

#[test]
fn e1_the_queries_carry_audio_without_parentheses() {
    for q in ["archive-buch", "netlabel", "otr"] {
        let b = base_query(q).unwrap();
        assert!(b.ends_with("mediatype:audio"), "{q}: {b}");
        assert!(!b.contains("(audio)"), "{q}: {b}");
    }
    let (mut n, log, _) = net(archive(wie_archive));
    let mut k = katalog("e1");
    let _ = lauf(&mut n, &mut k, "schellack", Art::Voll, &e(10));
    let qs = asked(&log);
    assert!(!qs.is_empty());
    for q in qs {
        assert!(q.contains("mediatype:audio") && !q.contains("(audio)"), "{q}");
        assert!(q.contains("subject:\""), "the quotes of the app stay in the query: {q}");
    }
}

#[test]
fn e2_the_echo_of_archive_org_passes_for_all_four_sources() {
    for quelle in ["archive-buch", "netlabel", "otr", "schellack"] {
        let (mut n, _, _) = net(archive(wie_archive));
        let mut k = katalog(&format!("e2-{quelle}"));
        let b = lauf(&mut n, &mut k, quelle, Art::Voll, &e(10));
        assert_eq!(b.ausgang, Ok(()), "{quelle}: {}", b.zeile());
        assert_eq!(live(&k, quelle), 3, "{quelle}");
    }
}

#[test]
fn e3_an_echo_is_unmasked_once_only() {
    // `&amp;quot;` is unmasked to `&quot;`, not to a quote: it stays unequal.
    fn doppelt(q: &str) -> String { wie_archive(q).replace("&quot;", "&amp;quot;") }
    let (mut n, _, _) = net(archive(doppelt));
    let mut k = katalog("e3");
    let b = lauf(&mut n, &mut k, "schellack", Art::Voll, &e(10));
    assert!(matches!(b.ausgang, Err(LaufFehler::Echo { .. })), "{}", b.zeile());
    assert_eq!(k.version("schellack").unwrap(), 0);
}

#[test]
fn e4_a_foreign_echo_stays_refused() {
    fn fremd(q: &str) -> String { wie_archive(q).replace("georgeblood", "something-else").replace("oldtimeradio", "something-else") }
    for quelle in ["otr", "schellack"] {
        let (mut n, _, _) = net(archive(fremd));
        let mut k = katalog(&format!("e4-{quelle}"));
        let b = lauf(&mut n, &mut k, quelle, Art::Voll, &e(10));
        assert!(matches!(b.ausgang, Err(LaufFehler::Echo { .. })), "{quelle}: {}", b.zeile());
        assert_eq!(k.version(quelle).unwrap(), 0);
    }
}

#[test]
fn e5_a_missing_echo_stays_refused() {
    let h: Handler = Box::new(|a, _| {
        assert!(a.url.starts_with("https://archive.org/advancedsearch.php"), "{}", a.url);
        ok(json!({"responseHeader": {"status": 0, "params": {}}, "response": {"numFound": 1, "start": 0, "docs": []}}))
    });
    let (mut n, _, _) = net(h);
    let mut k = katalog("e5");
    let b = lauf(&mut n, &mut k, "otr", Art::Voll, &e(10));
    assert!(matches!(b.ausgang, Err(LaufFehler::Echo { zurueck: None, .. })), "{}", b.zeile());
}

// ---------------------------------------------------------------- Radio Browser

fn station(i: usize, name: &str) -> Value {
    json!({"stationuuid": format!("00000000-0000-0000-0000-{i:012}"), "name": name, "url": format!("http://stream.example/{i}"),
           "url_resolved": format!("http://stream.example/{i}"), "favicon": "", "homepage": "", "countrycode": "DE", "tags": "jazz",
           "bitrate": 128, "codec": "MP3", "votes": 3, "languagecodes": "de", "state": "", "clickcount": 1, "lastcheckok": 1})
}

/// `listed` stations on the pages (the first `ohne_name` of them without a
/// name, which the reader drops) and a statistics count of `stats`.
fn radio(listed: usize, ohne_name: usize, stats: usize) -> Handler {
    let stations: Vec<Value> = (0..listed).map(|i| station(i, if i < ohne_name { "" } else { "Station" })).collect();
    Box::new(move |a, p| {
        assert!(a.url.starts_with("https://de1.api.radio-browser.info"), "{}", a.url);
        if a.url.contains("/json/stats") {
            return ok(json!({"stations": stats, "stations_broken": 330, "status": "OK"}));
        }
        let offset: usize = p["offset"].parse().unwrap();
        let limit: usize = p["limit"].parse().unwrap();
        ok(Value::Array(stations.iter().skip(offset).take(limit).cloned().collect()))
    })
}

fn radio_lauf(name: &str, listed: usize, ohne_name: usize, stats: usize) -> (maer_sync_abruf::lauf::Laufbericht, Katalog) {
    let (mut n, _, _) = net(radio(listed, ohne_name, stats));
    let mut k = katalog(name);
    let b = lauf(&mut n, &mut k, "radio", Art::Voll, &e(1000));
    (b, k)
}

#[test]
fn r1_one_station_more_than_the_statistics_passes() {
    // Measured 28.11.2026 twice: stats 53388, pages 53389. Here 2000 and 2001.
    let (b, k) = radio_lauf("r1", 2001, 0, 2000);
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(live(&k, "radio"), 2001);
}

#[test]
fn r2_a_thousandth_either_way_and_not_one_more() {
    // 2000 / 1000 = 2.
    for (listed, gut) in [(2002, true), (1998, true), (2003, false), (1997, false)] {
        let (b, k) = radio_lauf(&format!("r2-{listed}"), listed, 0, 2000);
        if gut {
            assert_eq!(b.ausgang, Ok(()), "{listed}: {}", b.zeile());
        } else {
            assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{listed}: {}", b.zeile());
            assert_eq!(k.version("radio").unwrap(), 0, "{listed}");
        }
    }
}

#[test]
fn r3_dropped_stations_do_not_count_against_the_bound() {
    // Three without a name: 2000 uuids, 1997 rows kept, the bound is 2.
    let (b, k) = radio_lauf("r3", 2000, 3, 2000);
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(live(&k, "radio"), 1997);
}

#[test]
fn r5_the_line_of_a_refusal_names_the_three_numbers_and_the_bound() {
    let (b, _) = radio_lauf("r5", 2003, 0, 2000);
    let z = b.zeile();
    for teil in ["erwartet: 2000", "gelesen: 2003", "eindeutig: 2003", "toleranz: 2"] {
        assert!(z.contains(teil), "{teil} missing in {z}");
    }
}

#[test]
fn r6_a_small_list_still_counts_exactly() {
    // Below a thousand stations the bound is 0.
    let (b, _) = radio_lauf("r6", 8, 0, 7);
    assert!(matches!(b.ausgang, Err(LaufFehler::Zaehlung { .. })), "{}", b.zeile());
}

// ---------------------------------------------------------------- Gutenberg

const WWW_KATALOG: &str = "https://www.gutenberg.org/cache/epub/feeds/pg_catalog.csv";
fn www_rdf(n: &str) -> String { format!("https://www.gutenberg.org/cache/epub/{n}/pg{n}.rdf") }

const PD: &str = "Public domain in the USA.";

fn record(n: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<rdf:RDF xml:base="http://www.gutenberg.org/" xmlns:pgterms="http://www.gutenberg.org/2009/pgterms/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <pgterms:ebook rdf:about="ebooks/{n}">
    <dcterms:rights>{PD}</dcterms:rights>
    <pgterms:downloads rdf:datatype="http://www.w3.org/2001/XMLSchema#integer">5</pgterms:downloads>
  </pgterms:ebook>
</rdf:RDF>
"#)
}

fn dump(n: u32) -> Vec<u8> {
    let mut s = String::from("Text#,Type,Issued,Title,Language,Authors,Subjects,LoCC,Bookshelves\n");
    for i in 1..=n {
        s.push_str(&format!("{i},Sound,2003-01-01,Title {i},en,\"Author, A\",,PR,\n"));
    }
    s.into_bytes()
}

/// How `www` answers: every request, or only the dump, then the records.
#[derive(Clone, Copy)]
enum Www { Gut, Status503, Pause, Zeit }

struct Gutenberg { www: Www, dump_s: i64, werke: u32 }

/// Both hosts. The dump takes `dump_s` seconds on the mirror: a request whose
/// limit is shorter ends with `Zeit` once its limit has passed.
fn gutenberg(g: Gutenberg, clock: Rc<RefCell<Option<Clock>>>) -> Handler {
    Box::new(move |a, _| {
        let www = a.url.starts_with("https://www.gutenberg.org/");
        if www {
            match g.www {
                Www::Gut => {}
                Www::Status503 => return Err(HolFehler::Status(503)),
                Www::Pause => return Err(HolFehler::SpaeterWieder(600)),
                Www::Zeit => return Err(HolFehler::Zeit),
            }
        }
        if a.url == WWW_KATALOG || a.url == GUTENBERG_KATALOG {
            if !www {
                let c = clock.borrow().clone().unwrap();
                if g.dump_s > a.timeout_s as i64 {
                    *c.0.borrow_mut() += a.timeout_s as i64 * 1000;
                    return Err(HolFehler::Zeit);
                }
                *c.0.borrow_mut() += g.dump_s * 1000;
            }
            return Ok(Antwort { status: 200, koerper: dump(g.werke), ..Default::default() });
        }
        let n = a.url.rsplit("/pg").next().unwrap().trim_end_matches(".rdf").to_string();
        assert!(a.url == gutenberg_rdf_url(&n) || a.url == www_rdf(&n), "an address off both hosts: {}", a.url);
        Ok(Antwort { status: 200, koerper: record(&n).into_bytes(), ..Default::default() })
    })
}

fn gutenberg_lauf(name: &str, g: Gutenberg) -> (maer_sync_abruf::lauf::Laufbericht, Rc<RefCell<Vec<Anfrage>>>, Katalog) {
    let slot = Rc::new(RefCell::new(None));
    let (mut n, log, clock) = net(gutenberg(g, slot.clone()));
    *slot.borrow_mut() = Some(clock);
    let mut k = katalog(name);
    let b = lauf(&mut n, &mut k, "gutenberg", Art::Voll, &e(10_000));
    (b, log, k)
}

fn an(log: &Rc<RefCell<Vec<Anfrage>>>, wirt: &str) -> usize {
    log.borrow().iter().filter(|a| a.url.starts_with(wirt)).count()
}

#[test]
fn g1_the_dump_alone_has_a_limit_of_its_own() {
    let (b, log, _) = gutenberg_lauf("g1", Gutenberg { www: Www::Gut, dump_s: 1, werke: 3 });
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    for a in log.borrow().iter() {
        let soll = if a.url.ends_with("pg_catalog.csv") { 1800 } else { 120 };
        assert_eq!(a.timeout_s, soll, "{}", a.url);
    }
}

#[test]
fn g2_a_slow_dump_within_the_limit_passes_and_one_beyond_it_breaks() {
    let (b, _, k) = gutenberg_lauf("g2-15min", Gutenberg { www: Www::Status503, dump_s: 900, werke: 3 });
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(live(&k, "gutenberg"), 3);
    let (b, _, k) = gutenberg_lauf("g2-40min", Gutenberg { www: Www::Status503, dump_s: 2400, werke: 3 });
    assert_eq!(b.ausgang, Err(LaufFehler::Holen(HolFehler::Zeit)), "{}", b.zeile());
    assert_eq!(k.version("gutenberg").unwrap(), 0);
}

#[test]
fn g3_www_first_and_the_mirror_only_when_www_fails() {
    let (b, log, _) = gutenberg_lauf("g3-gut", Gutenberg { www: Www::Gut, dump_s: 1, werke: 3 });
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(an(&log, "https://www.gutenberg.org/"), 4, "the dump and three records at www");
    assert_eq!(an(&log, "https://gutenberg.pglaf.org/"), 0, "the mirror was asked although www answered");
    assert_eq!(log.borrow()[0].url, WWW_KATALOG);
    for (name, www) in [("g3-503", Www::Status503), ("g3-pause", Www::Pause), ("g3-zeit", Www::Zeit)] {
        let (b, log, _) = gutenberg_lauf(name, Gutenberg { www, dump_s: 1, werke: 3 });
        assert_eq!(b.ausgang, Ok(()), "{name}: {}", b.zeile());
        let urls: Vec<String> = log.borrow().iter().map(|a| a.url.clone()).collect();
        assert_eq!(urls[0], WWW_KATALOG, "{name}");
        assert_eq!(urls[1], GUTENBERG_KATALOG, "{name}: the same dump from the mirror");
    }
}

#[test]
fn g4_after_www_failed_the_rest_of_the_run_asks_the_mirror_and_the_line_counts_both() {
    let (b, log, _) = gutenberg_lauf("g4", Gutenberg { www: Www::Status503, dump_s: 1, werke: 5 });
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(an(&log, "https://www.gutenberg.org/"), 1, "www asked again after it failed");
    assert_eq!(an(&log, "https://gutenberg.pglaf.org/"), 6, "the dump and five records at the mirror");
    assert!(b.zeile().contains(" www=1 spiegel=6"), "{}", b.zeile());
    let (b, _, _) = gutenberg_lauf("g4-gut", Gutenberg { www: Www::Gut, dump_s: 1, werke: 5 });
    assert!(b.zeile().contains(" www=6 spiegel=0"), "{}", b.zeile());
}

#[test]
fn g4_the_next_run_asks_www_again() {
    let slot = Rc::new(RefCell::new(None));
    let (mut n, log, clock) = net(gutenberg(Gutenberg { www: Www::Status503, dump_s: 1, werke: 2 }, slot.clone()));
    *slot.borrow_mut() = Some(clock);
    let mut k = katalog("g4-next");
    let b = lauf(&mut n, &mut k, "gutenberg", Art::Voll, &e(10_000));
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    let vorher = log.borrow().len();
    let b = lauf(&mut n, &mut k, "gutenberg", Art::Voll, &e(10_000));
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(log.borrow()[vorher].url, WWW_KATALOG, "the second run went to the mirror first");
}
