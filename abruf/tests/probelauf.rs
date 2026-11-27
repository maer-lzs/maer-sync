// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The system clock, and a probe run against archive.org that runs only by
//! hand (`--ignored`, and only with `MAER_PROBELAUF=1` set): it fetches one
//! full pass of LibriVox into a throwaway catalog and prints the line of the
//! run. No witness here reaches the network unless both are given.

use std::time::Duration;

use maer_sync::katalog::{Katalog, MAX_PAGE};
use maer_sync_abruf::programm::Programm;
use maer_sync_abruf::netz::{Netz, SystemUhr, Uhr};
use maer_sync_abruf::plan::Art;
use maer_sync_abruf::ureq_holer::UreqHoler;

#[test]
fn the_system_clock_moves_and_sleeps() {
    let mut u = SystemUhr;
    let a = u.jetzt_ms();
    assert!(a > 1_700_000_000_000, "milliseconds since 1970: {a}");
    u.schlafe_ms(30);
    let b = u.jetzt_ms();
    assert!(b - a >= 30, "{a} {b}");
    u.schlafe_ms(-5);
    assert!(u.jetzt_ms() - b < 1_000);
    let _ = Duration::ZERO;
}

/// Whether the probe run may reach archive.org; without the variable it
/// says what is missing.
fn probe_wanted(v: Option<&str>) -> Result<(), String> {
    if v == Some("1") { Ok(()) } else { Err("the probe run needs MAER_PROBELAUF=1; nothing was fetched".into()) }
}

#[test]
fn the_probe_run_without_its_variable_says_what_is_missing() {
    // Run with `--ignored` but without the variable, the probe run ended
    // green without a request: a green without a measurement.
    let e = probe_wanted(None).unwrap_err();
    assert!(e.contains("MAER_PROBELAUF=1"), "{e}");
    assert!(probe_wanted(Some("0")).is_err());
    assert!(probe_wanted(Some("1")).is_ok());
}

#[test]
#[ignore]
fn probe_run_librivox_against_archive_org() {
    if let Err(e) = probe_wanted(std::env::var("MAER_PROBELAUF").ok().as_deref()) {
        panic!("{e}");
    }
    let p = std::env::temp_dir().join(format!("maer-sync-probelauf-{}", std::process::id()));
    // As the program runs: under the run lock, closed cleanly.
    let mut prog = Programm::new(Netz::new(UreqHoler::new(), SystemUhr), p.to_str().unwrap()).unwrap();
    let b = prog.ein_lauf("librivox", Art::Voll, false);
    println!("PROBELAUF {}", b.zeile());
    // A run that failed is no measurement; the line above says why.
    assert_eq!(b.ausgang, Ok(()), "the probe run failed");
    let k = Katalog::open_read(p.to_str().unwrap()).unwrap();
    {
        let mut rows = 0;
        let mut kinds = std::collections::BTreeMap::new();
        let mut nach: Option<String> = None;
        loop {
            let page = k.full("librivox", nach.as_deref(), MAX_PAGE).unwrap();
            for s in &page.zeilen {
                rows += 1;
                *kinds.entry(s.row.rechte_kennung.clone()).or_insert(0) += 1;
            }
            match page.weiter { Some(w) => nach = Some(w), None => break }
        }
        println!("PROBELAUF zeilen={rows} rechte={kinds:?}");
    }
    drop(k);
    for s in ["", "-wal", "-shm", ".lauf"] {
        let _ = std::fs::remove_file(format!("{}{s}", p.display()));
    }
}
