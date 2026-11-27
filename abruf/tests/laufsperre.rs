// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! One run at a time on a catalog file. The long-running program and a
//! run the operator confirms (`--bestaetigt`) are two processes on one
//! file; so is the service, which reads it. The second process in these
//! witnesses is this test binary again, started with one helper test and
//! its orders in the environment; the network is a fake that counts its
//! requests.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::Read;
use std::rc::Rc;
use std::time::{Duration, Instant};

use maer_sync::katalog::{Katalog, KatalogError, MAX_PAGE};
use maer_sync_abruf::lauf::{lauf, Einstellungen, LaufFehler};
use maer_sync_abruf::netz::{decode_params, Anfrage, Antwort, HolFehler, Holer, Netz, Uhr};
use maer_sync_abruf::plan::{Art, BACKOFF_S, MIN_SCHLAF_S};
use maer_sync_abruf::programm::{Programm, QUELLEN};
use serde_json::{json, Value};

// ---------------------------------------------------------------- the fake

#[derive(Clone)]
struct Clock(Rc<RefCell<i64>>);
impl Uhr for Clock {
    fn jetzt_ms(&self) -> i64 { *self.0.borrow() }
    fn schlafe_ms(&mut self, ms: i64) { *self.0.borrow_mut() += ms.max(0); }
}

/// LibriVox with `n` records. `anfragen` counts the requests; while
/// `scheitert` is set every request fails; `vor_seite` runs once, before the
/// first scrape page is answered.
struct Fake {
    clock: Clock,
    n: usize,
    anfragen: Rc<Cell<usize>>,
    scheitert: Rc<Cell<bool>>,
    vor_seite: Option<Box<dyn FnMut()>>,
}

impl Holer for Fake {
    fn hole(&mut self, a: &Anfrage) -> Result<Antwort, HolFehler> {
        self.anfragen.set(self.anfragen.get() + 1);
        *self.clock.0.borrow_mut() += 50;
        if self.scheitert.get() {
            return Err(HolFehler::Zeit);
        }
        let p: BTreeMap<String, String> = decode_params(&a.url).into_iter().collect();
        let q = p.get("q").cloned().unwrap_or_default();
        if a.url.starts_with("https://archive.org/advancedsearch.php") {
            return ok(json!({"responseHeader": {"status": 0, "params": {"query": q, "qin": q}},
                             "response": {"numFound": self.n, "start": 0, "docs": []}}));
        }
        if let Some(mut f) = self.vor_seite.take() {
            f();
        }
        let items: Vec<Value> = (0..self.n).map(|i| json!({
            "identifier": format!("lv-{i:05}"), "title": format!("Title {i}"), "collection": ["librivoxaudio"]})).collect();
        ok(json!({"items": items, "count": self.n, "total": self.n}))
    }
}

fn ok(body: Value) -> Result<Antwort, HolFehler> {
    Ok(Antwort { status: 200, koerper: body.to_string().into_bytes(), ..Default::default() })
}

const START_MS: i64 = 1_764_000_000_000;

struct Netzteil {
    netz: Netz<Fake, Clock>,
    anfragen: Rc<Cell<usize>>,
    scheitert: Rc<Cell<bool>>,
}

fn netz(n: usize, vor_seite: Option<Box<dyn FnMut()>>) -> Netzteil {
    let clock = Clock(Rc::new(RefCell::new(START_MS)));
    let anfragen = Rc::new(Cell::new(0));
    let scheitert = Rc::new(Cell::new(false));
    let fake = Fake { clock: clock.clone(), n, anfragen: anfragen.clone(), scheitert: scheitert.clone(), vor_seite };
    Netzteil { netz: Netz::new(fake, clock), anfragen, scheitert }
}

fn settings() -> Einstellungen { Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: 2026 } }

fn frisch(name: &str) -> String {
    let p = std::env::temp_dir().join(format!("laufsperre-{name}-{}", std::process::id()));
    for s in ["", "-wal", "-shm", "-journal", ".lauf"] {
        let _ = std::fs::remove_file(format!("{}{s}", p.display()));
    }
    p.to_string_lossy().into_owned()
}

/// Clears a catalog and its side files when the witness ends, green or not.
struct Weg(String);

impl Drop for Weg {
    fn drop(&mut self) {
        for s in ["", "-wal", "-shm", "-journal", ".lauf"] {
            let _ = std::fs::remove_file(format!("{}{s}", self.0));
        }
    }
}

/// A catalog with `n` LibriVox rows, closed cleanly, in which only LibriVox
/// is due (its full pass eight days old, every other source run just now).
fn bereit(name: &str, n: usize) -> String {
    let path = frisch(name);
    let mut k = Katalog::open(&path).unwrap();
    assert_eq!(lauf(&mut netz(n, None).netz, &mut k, "librivox", Art::Voll, &settings()).ausgang, Ok(()));
    for q in QUELLEN {
        for art in ["voll", "delta"] {
            if *q != "librivox" {
                k.set_last_run(q, art, START_MS / 1000).unwrap();
            }
        }
    }
    k.set_last_run("librivox", "voll", START_MS / 1000 - 8 * 86_400).unwrap();
    k.close_clean().unwrap();
    path
}

fn lebend(path: &str) -> usize {
    let k = Katalog::open_read(path).unwrap();
    k.full("librivox", None, MAX_PAGE).unwrap().zeilen.len()
}

fn seitendateien(path: &str) -> Vec<String> {
    ["-wal", "-shm", "-journal"].iter().filter(|s| std::path::Path::new(&format!("{path}{s}")).exists())
        .map(|s| s.to_string()).collect()
}

/// Start the helper `name` in a second process and wait until it marks
/// that it holds what it was told to hold.
fn helfer(name: &str, env: &[(&str, String)]) -> std::process::Child {
    // One mark per call: two witnesses start the same helper side by side,
    // and a shared mark let one of them see the other's.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mark = frisch(&format!("mark-{name}-{call}"));
    let mut c = std::process::Command::new(std::env::current_exe().unwrap());
    c.args(["--ignored", "--exact", name, "--nocapture"]).env("LS_MARKE", &mark)
        .stdout(std::process::Stdio::piped());
    for (k, v) in env {
        c.env(k, v);
    }
    let mut child = c.spawn().unwrap();
    let t = Instant::now();
    while !std::path::Path::new(&mark).exists() {
        if let Some(s) = child.try_wait().unwrap() {
            panic!("the helper {name} ended before it marked: {s}");
        }
        assert!(t.elapsed() < Duration::from_secs(30), "the helper {name} never marked");
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = std::fs::remove_file(&mark);
    child
}

fn ausgabe(mut child: std::process::Child) -> String {
    child.wait().unwrap();
    let mut s = String::new();
    child.stdout.take().unwrap().read_to_string(&mut s).unwrap();
    s
}

fn markiere() {
    std::fs::write(std::env::var("LS_MARKE").unwrap(), b"da").unwrap();
}

fn ms() -> u64 { std::env::var("LS_MS").unwrap().parse().unwrap() }

// ---------------------------------------------------------------- helpers (the second process)

#[test]
#[ignore]
fn helfer_haelt_die_laufsperre() {
    let Ok(path) = std::env::var("LS_DATEI") else { return };
    let _sperre = Katalog::laufsperre(&path).expect("the helper got no lock");
    // As a run of the service's neighbour would: a write transaction too.
    let _schreiber = std::env::var("LS_AUCH_SCHREIBEN").is_ok().then(|| {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("BEGIN IMMEDIATE").unwrap();
        db
    });
    markiere();
    std::thread::sleep(Duration::from_millis(ms()));
}

#[test]
#[ignore]
fn helfer_bestaetigt() {
    let Ok(path) = std::env::var("LS_DATEI") else { return };
    let n: usize = std::env::var("LS_ZEILEN").unwrap().parse().unwrap();
    let mut p = Programm::new(netz(n, None).netz, &path).unwrap();
    let b = p.bestaetigt("librivox");
    println!("HELFER {}", b.zeile());
    markiere();
}

#[test]
#[ignore]
fn helfer_liest() {
    let Ok(path) = std::env::var("LS_DATEI") else { return };
    // The service's way: read only, one connection.
    let k = Katalog::open_read(&path).unwrap();
    let vorher = k.full("librivox", None, MAX_PAGE).unwrap();
    println!("HELFER vorher {} {}", vorher.zeilen.len(), vorher.version);
    markiere();
    std::thread::sleep(Duration::from_millis(ms()));
    let danach = k.full("librivox", None, MAX_PAGE).unwrap();
    println!("HELFER danach {} {}", danach.zeilen.len(), danach.version);
}

// ---------------------------------------------------------------- the witnesses

#[test]
fn a_run_finds_the_catalog_taken_and_fetches_nothing() {
    let path = bereit("taken", 100);
    let _weg = Weg(path.clone());
    let v = Katalog::open_read(&path).unwrap().version("librivox").unwrap();
    let h = helfer("helfer_haelt_die_laufsperre", &[("LS_DATEI", path.clone()), ("LS_MS", "20000".into())]);
    let n = netz(99, None);
    let anfragen = n.anfragen.clone();
    let mut p = Programm::new(n.netz, &path).unwrap();
    let (b, _) = p.schritt();
    let b = b.expect("LibriVox was due");
    let mut h = h;
    h.kill().unwrap();
    h.wait().unwrap();
    assert_eq!(b.ausgang, Err(LaufFehler::Belegt), "{}", b.zeile());
    assert!(b.zeile().contains("grund=Belegt"), "{}", b.zeile());
    assert_eq!(anfragen.get(), 0, "a run without the lock asked the network");
    assert_eq!(Katalog::open_read(&path).unwrap().version("librivox").unwrap(), v);
    assert_eq!(lebend(&path), 100);
}

#[test]
fn the_program_takes_a_taken_catalog_as_later() {
    let path = bereit("later", 100);
    let _weg = Weg(path.clone());
    let mut h = helfer("helfer_haelt_die_laufsperre", &[("LS_DATEI", path.clone()), ("LS_MS", "20000".into())]);
    let n = netz(99, None);
    let scheitert = n.scheitert.clone();
    let mut p = Programm::new(n.netz, &path).unwrap();
    let (b, schlaf) = p.schritt();
    assert_eq!(b.unwrap().ausgang, Err(LaufFehler::Belegt));
    assert_eq!(schlaf, MIN_SCHLAF_S, "a taken catalog is tried again soon");
    h.kill().unwrap();
    h.wait().unwrap();
    // The next step runs LibriVox and fails at the source: the pause is the
    // first stage, so the taken catalog did not count as a failure.
    p.netz.uhr.schlafe_ms(schlaf * 1000);
    scheitert.set(true);
    let (b, schlaf) = p.schritt();
    let b = b.expect("LibriVox was due again");
    assert!(matches!(b.ausgang, Err(LaufFehler::Holen(_))), "{}", b.zeile());
    assert_eq!(schlaf, BACKOFF_S[0]);
}

#[test]
fn a_confirmed_run_ends_when_the_catalog_is_taken() {
    let path = bereit("confirmed-taken", 100);
    let _weg = Weg(path.clone());
    let v = Katalog::open_read(&path).unwrap().version("librivox").unwrap();
    let mut h = helfer("helfer_haelt_die_laufsperre", &[("LS_DATEI", path.clone()), ("LS_MS", "20000".into())]);
    let n = netz(10, None);
    let anfragen = n.anfragen.clone();
    let mut p = Programm::new(n.netz, &path).unwrap();
    let b = p.bestaetigt("librivox");
    h.kill().unwrap();
    h.wait().unwrap();
    // No waiting: the operator learns at once and runs it again later.
    assert_eq!(b.ausgang, Err(LaufFehler::Belegt), "{}", b.zeile());
    assert!(b.bestaetigt);
    assert!(b.zeile().contains("another run holds the catalog"), "{}", b.zeile());
    assert_eq!(anfragen.get(), 0);
    assert_eq!(Katalog::open_read(&path).unwrap().version("librivox").unwrap(), v);
    assert_eq!(lebend(&path), 100);
}

#[test]
fn no_two_runs_interleave() {
    // Measured 26.11.2026 without the lock: the confirmed run wrote 50, the
    // program wrote its 150 over it, both lines said ok.
    let path = bereit("interleave", 150);
    let _weg = Weg(path.clone());
    let v = Katalog::open_read(&path).unwrap().version("librivox").unwrap();
    let p2 = path.clone();
    let helfer_zeile = Rc::new(RefCell::new(String::new()));
    let hz = helfer_zeile.clone();
    let vor_seite: Box<dyn FnMut()> = Box::new(move || {
        let h = helfer("helfer_bestaetigt", &[("LS_DATEI", p2.clone()), ("LS_ZEILEN", "50".into())]);
        *hz.borrow_mut() = ausgabe(h);
    });
    let mut p = Programm::new(netz(150, Some(vor_seite)).netz, &path).unwrap();
    let (b, _) = p.schritt();
    let b = b.unwrap();
    let hz = helfer_zeile.borrow().clone();
    assert!(hz.contains("grund=Belegt"), "the confirmed run did not find the catalog taken: {hz}");
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(lebend(&path), 150);
    // The program's run stands alone: no second version in between.
    assert_eq!(Katalog::open_read(&path).unwrap().version("librivox").unwrap(), v, "{hz}");
}

#[test]
fn the_lock_goes_with_its_process() {
    let path = bereit("dies", 10);
    let _weg = Weg(path.clone());
    let mut h = helfer("helfer_haelt_die_laufsperre", &[("LS_DATEI", path.clone()), ("LS_MS", "20000".into())]);
    // While it lives, the lock is taken.
    assert!(matches!(Katalog::laufsperre(&path), Err(KatalogError::Busy)), "the lock is not held by its process");
    // Killed, it releases nothing itself; the system does.
    h.kill().unwrap();
    h.wait().unwrap();
    assert!(Katalog::laufsperre(&path).is_ok(), "the lock stayed with a dead process");
}

#[test]
fn the_program_starts_beside_a_run() {
    let path = bereit("start", 10);
    let _weg = Weg(path.clone());
    let mut h = helfer("helfer_haelt_die_laufsperre", &[
        ("LS_DATEI", path.clone()), ("LS_MS", "20000".into()), ("LS_AUCH_SCHREIBEN", "1".into())]);
    let neu = Programm::new(netz(10, None).netz, &path);
    h.kill().unwrap();
    h.wait().unwrap();
    // Measured 26.11.2026 before: "another writer holds the catalog" at once,
    // and the program ended at its start.
    assert!(neu.is_ok(), "{:?}", neu.err());
}

#[test]
fn a_whole_run_closes_clean_beside_a_reader() {
    // The service reads beside every run. A reader that opened the file
    // while it was in WAL holds a share of the WAL index; leaving WAL needs
    // the file alone (measured 26.11.2026: opened before the switch, the
    // close succeeded; opened after it, it failed at once and the side
    // files stayed). The run waits for such a reader instead.
    let path = bereit("reader", 100);
    let _weg = Weg(path.clone());
    let p2 = path.clone();
    let leser: Rc<RefCell<Option<std::process::Child>>> = Rc::new(RefCell::new(None));
    let l2 = leser.clone();
    let vor_seite: Box<dyn FnMut()> = Box::new(move || {
        // The run has opened the catalog (WAL) by now; the reader opens after it.
        *l2.borrow_mut() = Some(helfer("helfer_liest", &[("LS_DATEI", p2.clone()), ("LS_MS", "1500".into())]));
    });
    let mut p = Programm::new(netz(99, Some(vor_seite)).netz, &path).unwrap();
    let (b, _) = p.schritt();
    let b = b.unwrap();
    assert_eq!(b.ausgang, Ok(()), "{}", b.zeile());
    assert_eq!(seitendateien(&path), Vec::<String>::new(), "the run did not close cleanly beside the reader");
    let gelesen = ausgabe(leser.borrow_mut().take().unwrap());
    assert!(gelesen.contains("HELFER vorher 100 1"), "{gelesen}");
    assert!(gelesen.contains("HELFER danach 99 2"), "the reader did not see the run: {gelesen}");
    assert_eq!(lebend(&path), 99);
}
