// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! A measurement, not a witness: the peak memory of a full run of 190 000
//! rows against a throwaway catalog. Run it by hand with `--ignored`; it
//! prints the number for the report.

use maer_sync::katalog::{Katalog, Row};
use maer_sync_abruf::lauf::{lauf, Einstellungen};
use maer_sync_abruf::plan::Art;
use maer_sync_abruf::netz::{Netz, SystemUhr};
use maer_sync_abruf::ureq_holer::UreqHoler;

fn peak_kib() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines().find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok()).unwrap_or(0)
}

#[test]
#[ignore]
fn memory_of_a_full_run() {
    let p = std::env::temp_dir().join(format!("maer-sync-abruf-speicher-{}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    let mut k = Katalog::open(p.to_str().unwrap()).unwrap();
    let rows: Vec<Row> = (0..190_000).map(|i| Row {
        quelle: "schellack".into(), id: format!("lva:78_record-{i:06}_some-performer-and-orchestra_gbia{i:07}b"),
        art: "musik".into(), titel: Some(format!("A title of some length, number {i}")),
        urheber: Some("Performer One; Orchestra Two; Composer Three".into()), sprache: Some("English".into()),
        jahr: Some(1920 + (i % 30) as i64), rechte_kennung: "unbekannt".into(), rechte_url: None, laden: false,
        reihen: vec!["shellac-jazz".into()], auch: vec![], tief: None,
    }).collect();
    let before = peak_kib();
    k.write_run("schellack", &rows, false).unwrap();
    let mut second = rows.clone();
    second[0].titel = Some("changed".into());
    k.write_run("schellack", &second, false).unwrap();
    println!("MESSUNG peak_kib_vor={before} peak_kib_nach={} zeilen=190000", peak_kib());
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{s}", p.display()));
    }
}

/// A real full run of Radio Browser over the network, as the program makes
/// it (pages of 10 000), against a throwaway catalog. By hand with
/// `--ignored`; it prints the peak and the size of the file.
#[test]
#[ignore]
fn memory_of_a_radio_run() {
    let p = std::env::temp_dir().join(format!("maer-sync-abruf-radio-{}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    let mut k = Katalog::open(p.to_str().unwrap()).unwrap();
    let mut n = Netz::new(UreqHoler::new(), SystemUhr);
    let e = Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: 2026 };
    let before = peak_kib();
    let b = lauf(&mut n, &mut k, "radio", Art::Voll, &e);
    let after = peak_kib();
    k.close_clean().unwrap();
    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    println!("MESSUNG {} peak_kib_vor={before} peak_kib_nach={after} datei_bytes={bytes}", b.zeile());
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{s}", p.display()));
    }
    assert_eq!(b.ausgang, Ok(()));
}
