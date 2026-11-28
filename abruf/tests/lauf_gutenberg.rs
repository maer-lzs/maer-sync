// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The records of the Gutenberg works: read one by one from the mirror, in
//! the full pass only, at most a cap of them a run. The rights come from
//! there and the downloads give the step. A failed record leaves a living
//! work as the last run had it and keeps a new one out; more than a tenth
//! failed refuses the run.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use maer_sync::katalog::{Katalog, MAX_PAGE};
use maer_sync_abruf::gutenberg::{rdf_lesen, Rdf, Rechte};
use maer_sync_abruf::lauf::{gutenberg_rdf_url, lauf, Einstellungen, LaufFehler, OHNE_DELTA, GUTENBERG_KATALOG, GUTENBERG_RDF_DECKEL};
use maer_sync_abruf::netz::{Anfrage, Antwort, HolFehler, Holer, Netz, Uhr};
use maer_sync_abruf::plan::Art;
use maer_sync_abruf::programm::QUELLEN;

fn vektor(name: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vektoren/katalog/gutenberg").join(name)).unwrap()
}

#[derive(Clone)]
struct Clock(Rc<RefCell<i64>>);
impl Uhr for Clock {
    fn jetzt_ms(&self) -> i64 { *self.0.borrow() }
    fn schlafe_ms(&mut self, ms: i64) { *self.0.borrow_mut() += ms.max(0); }
}

/// The mirror: the dump of `werke` (number, type) and a record per number,
/// `None` for a 404. Every request is kept with the time it came.
struct Spiegel {
    clock: Clock,
    log: Rc<RefCell<Vec<(String, i64)>>>,
    csv: Rc<RefCell<Vec<u8>>>,
    rdf: Rc<RefCell<BTreeMap<String, Option<(String, i64)>>>>,
}

impl Holer for Spiegel {
    fn hole(&mut self, a: &Anfrage) -> Result<Antwort, HolFehler> {
        // `www` is asked first and answers 503, as measured on 28.11.2026;
        // these witnesses read the mirror (www first: katalog_befunde.rs).
        if a.url.starts_with("https://www.gutenberg.org/") {
            return Err(HolFehler::Status(503));
        }
        let now = *self.clock.0.borrow();
        self.log.borrow_mut().push((a.url.clone(), now));
        *self.clock.0.borrow_mut() += 50;
        if a.url == GUTENBERG_KATALOG {
            return Ok(Antwort { status: 200, koerper: self.csv.borrow().clone(), ..Default::default() });
        }
        let n = a.url.rsplit("/pg").next().unwrap().trim_end_matches(".rdf").to_string();
        assert_eq!(a.url, gutenberg_rdf_url(&n), "an address off the mirror");
        match self.rdf.borrow().get(&n) {
            Some(Some((rights, d))) => Ok(Antwort { status: 200, koerper: record(&n, rights, *d).into_bytes(), ..Default::default() }),
            _ => Err(HolFehler::Status(404)),
        }
    }
}

fn record(n: &str, rights: &str, downloads: i64) -> String {
    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<rdf:RDF xml:base="http://www.gutenberg.org/" xmlns:pgterms="http://www.gutenberg.org/2009/pgterms/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <pgterms:ebook rdf:about="ebooks/{n}">
    <dcterms:rights>{rights}</dcterms:rights>
    <pgterms:downloads rdf:datatype="http://www.w3.org/2001/XMLSchema#integer">{downloads}</pgterms:downloads>
  </pgterms:ebook>
</rdf:RDF>
"#)
}

fn dump(werke: &[(u32, &str)]) -> Vec<u8> {
    let mut s = String::from("Text#,Type,Issued,Title,Language,Authors,Subjects,LoCC,Bookshelves\n");
    for (n, typ) in werke {
        s.push_str(&format!("{n},{typ},2003-01-01,Title {n},en,\"Author, A\",,PR,\n"));
    }
    s.into_bytes()
}

const PD: &str = "Public domain in the USA.";
const CR: &str = "Copyrighted. Read the copyright notice inside this book for details.";

struct Aufbau {
    netz: Netz<Spiegel, Clock>,
    log: Rc<RefCell<Vec<(String, i64)>>>,
    csv: Rc<RefCell<Vec<u8>>>,
    rdf: Rc<RefCell<BTreeMap<String, Option<(String, i64)>>>>,
    k: Katalog,
}

fn aufbau(name: &str, werke: &[(u32, &str)], rdf: &[(u32, Option<(&str, i64)>)]) -> Aufbau {
    let clock = Clock(Rc::new(RefCell::new(1_764_000_000_000)));
    let log = Rc::new(RefCell::new(vec![]));
    let csv = Rc::new(RefCell::new(dump(werke)));
    let rdf = Rc::new(RefCell::new(rdf.iter().map(|(n, r)| (n.to_string(), r.map(|(a, d)| (a.to_string(), d)))).collect()));
    let netz = Netz::new(Spiegel { clock: clock.clone(), log: log.clone(), csv: csv.clone(), rdf: rdf.clone() }, clock);
    let p = std::env::temp_dir().join(format!("maer-sync-gut-{name}-{}", std::process::id()));
    for s in ["", "-wal", "-shm"] { let _ = std::fs::remove_file(format!("{}{s}", p.display())); }
    Aufbau { netz, log, csv, rdf, k: Katalog::open(p.to_str().unwrap()).unwrap() }
}

fn e() -> Einstellungen { Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: 2026 } }

fn zeilen(k: &Katalog) -> BTreeMap<String, (String, bool, Option<i64>)> {
    k.full("gutenberg", None, MAX_PAGE).unwrap().zeilen.into_iter()
        .map(|s| (s.row.id, (s.row.rechte_kennung, s.row.laden, s.row.beliebt_stufe))).collect()
}

fn elf(fehlt: &[u32]) -> Vec<(u32, Option<(&'static str, i64)>)> {
    (1..=11).map(|n| (n, (!fehlt.contains(&n)).then_some((PD, 1000 - n as i64)))).collect()
}

fn elf_werke() -> Vec<(u32, &'static str)> { (1..=11).map(|n| (n, "Sound")).collect() }

// ---------------------------------------------------------------- the record

#[test]
fn r1_a_record_in_the_public_domain() {
    assert_eq!(rdf_lesen(&vektor("gemeinfrei-usa/antwort.rdf"), "10157"), Ok(Rdf { rechte: Rechte::Pd, downloads: Some(63) }));
}

#[test]
fn r2_the_records_of_the_app_are_copyrighted() {
    assert_eq!(rdf_lesen(&vektor("pg3002.rdf"), "3002"), Ok(Rdf { rechte: Rechte::Copyrighted, downloads: Some(326) }));
    assert_eq!(rdf_lesen(&vektor("pg5884.rdf"), "5884"), Ok(Rdf { rechte: Rechte::Copyrighted, downloads: Some(246) }));
}

#[test]
fn r3_another_text_of_rights() {
    let r = rdf_lesen(record("7", "Some other statement.", 5).as_bytes(), "7").unwrap();
    assert_eq!(r.rechte, Rechte::Andere);
}

#[test]
fn r4_a_record_of_another_work_or_no_xml_is_refused() {
    assert!(rdf_lesen(&vektor("gemeinfrei-usa/antwort.rdf"), "3002").is_err(), "the record of 10157 asked as 3002");
    assert!(rdf_lesen(b"<rdf:RDF", "7").is_err(), "not XML");
}

// ---------------------------------------------------------------- the run

#[test]
fn r5_the_full_pass_asks_the_mirror_once_per_sound_work() {
    let werke = [(1, "Sound"), (2, "Text"), (3, "Sound"), (4, "Sound")];
    let mut a = aufbau("r5", &werke, &[(1, Some((PD, 30))), (3, Some((PD, 20))), (4, Some((CR, 10)))]);
    let b = lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e());
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    let urls: Vec<String> = a.log.borrow().iter().map(|(u, _)| u.clone()).collect();
    assert_eq!(urls, [GUTENBERG_KATALOG.to_string(), gutenberg_rdf_url("1"), gutenberg_rdf_url("3"), gutenberg_rdf_url("4")]);
    let t: Vec<i64> = a.log.borrow().iter().map(|(_, t)| *t).collect();
    for w in t.windows(2) {
        assert!(w[1] - w[0] >= 1000, "two requests to the mirror less than 1 s apart: {t:?}");
    }
    // 4 is copyrighted and stays out; 2 is a text.
    let z = zeilen(&a.k);
    assert_eq!(z.keys().cloned().collect::<Vec<_>>(), ["gut:1", "gut:3"]);
    assert_eq!(z["gut:1"].0, "pd");
    assert!(z["gut:1"].1, "a work in the public domain loads");
}

#[test]
fn r6_a_new_work_whose_record_fails_stays_out() {
    let mut a = aufbau("r6", &elf_werke(), &elf(&[5]));
    let b = lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e());
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    let z = zeilen(&a.k);
    assert_eq!(z.len(), 10);
    assert!(!z.contains_key("gut:5"), "a new work without its record came in");
}

#[test]
fn r7_more_than_a_tenth_failed_refuses_the_run() {
    let mut a = aufbau("r7", &elf_werke(), &elf(&[5, 6]));
    let b = lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e());
    assert!(matches!(b.ausgang, Err(LaufFehler::Ausfall { ausgefallen: 2, gesamt: 11 })), "{}", b.zeile());
    assert_eq!(a.k.version("gutenberg").unwrap(), 0);
}

#[test]
fn r8_more_works_than_the_cap_refuses_before_the_first_record() {
    let werke: Vec<(u32, &str)> = (1..=(GUTENBERG_RDF_DECKEL as u32 + 1)).map(|n| (n, "Sound")).collect();
    let mut a = aufbau("r8", &werke, &[]);
    let b = lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e());
    assert!(matches!(b.ausgang, Err(LaufFehler::Deckel(_))), "{}", b.zeile());
    assert_eq!(a.log.borrow().len(), 1, "only the dump was asked");
    assert_eq!(a.k.version("gutenberg").unwrap(), 0);
}

#[test]
fn r9_a_delta_of_gutenberg_asks_nothing() {
    let mut a = aufbau("r9", &elf_werke(), &elf(&[]));
    assert_eq!(lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e()).ausgang, Ok(()));
    let vorher = (a.log.borrow().len(), a.k.version("gutenberg").unwrap());
    let b = lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Delta, &e());
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!((a.log.borrow().len(), a.k.version("gutenberg").unwrap()), vorher, "a delta asked the mirror or wrote");
    assert!(b.zeile().contains("art=delta gelesen=0"), "{}", b.zeile());
}

#[test]
fn r10_gutenberg_is_planned_weekly() {
    assert!(QUELLEN.contains(&"gutenberg"), "{QUELLEN:?}");
    assert!(!OHNE_DELTA.contains(&"gutenberg"), "in OHNE_DELTA the full pass would run daily");
}

#[test]
fn r11_the_downloads_give_the_step() {
    let werke: Vec<(u32, &str)> = (1..=10).map(|n| (n, "Sound")).collect();
    let rdf: Vec<(u32, Option<(&str, i64)>)> = (1..=10).map(|n| (n, Some((PD, 100 - n as i64)))).collect();
    let mut a = aufbau("r11", &werke, &rdf);
    assert_eq!(lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e()).ausgang, Ok(()));
    let z = zeilen(&a.k);
    for n in 1..=10 {
        assert_eq!(z[&format!("gut:{n}")].2, Some(n as i64), "gut:{n}");
    }
}

#[test]
fn r12_a_living_work_whose_record_fails_keeps_the_last_run() {
    let mut a = aufbau("r12", &elf_werke(), &elf(&[]));
    assert_eq!(lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e()).ausgang, Ok(()));
    let vorher = zeilen(&a.k)["gut:3"].clone();
    assert_eq!((vorher.0.as_str(), vorher.1), ("pd", true));
    assert!(vorher.2.is_some());
    a.rdf.borrow_mut().insert("3".into(), None);
    let b = lauf(&mut a.netz, &mut a.k, "gutenberg", Art::Voll, &e());
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(zeilen(&a.k)["gut:3"], vorher, "a living work lost its rights or step");
    let _ = &a.csv;
}
