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
use maer_sync_abruf::lauf::{lauf, Einstellungen};
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

#[test]
#[ignore]
fn probe_run_librivox_against_archive_org() {
    if std::env::var("MAER_PROBELAUF").as_deref() != Ok("1") {
        println!("PROBELAUF nicht gefahren: MAER_PROBELAUF=1 fehlt");
        return;
    }
    let p = std::env::temp_dir().join(format!("maer-sync-probelauf-{}", std::process::id()));
    let mut k = Katalog::open(p.to_str().unwrap()).unwrap();
    let mut n = Netz::new(UreqHoler::new(), SystemUhr);
    let jahr = 2026;
    let b = lauf(&mut n, &mut k, "librivox", Art::Voll, &Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: jahr });
    println!("PROBELAUF {}", b.zeile());
    if b.ausgang.is_ok() {
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
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{s}", p.display()));
    }
}
