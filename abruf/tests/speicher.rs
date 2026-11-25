// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! A measurement, not a witness: the peak memory of a full run of 190 000
//! rows against a throwaway catalog. Run it by hand with `--ignored`; it
//! prints the number for the report.

use maer_sync::katalog::{Katalog, Row};

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
