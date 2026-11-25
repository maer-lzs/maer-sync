// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The readers of Musopen (Wikimedia Commons) and Radio Browser against the
//! shared vectors, and their rules as tables. Pure: no request is made here.

use std::path::{Path, PathBuf};

use maer_sync::katalog::Row;
use maer_sync_abruf::musopen::{self, MusopenFehler};
use maer_sync_abruf::radio;
use serde_json::{json, Value};

fn root(q: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("vektoren").join("katalog").join(q)
}

fn cases(q: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root(q)).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

fn read(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn opt(v: &Value) -> Option<String> { v.as_str().map(str::to_string) }
fn strings(v: &Value) -> Vec<String> { v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() }

fn expected(v: &Value) -> Row {
    Row {
        quelle: v["quelle"].as_str().unwrap().into(), id: v["id"].as_str().unwrap().into(), art: v["art"].as_str().unwrap().into(),
        titel: opt(&v["titel"]), urheber: opt(&v["urheber"]), sprache: opt(&v["sprache"]), jahr: v["jahr"].as_i64(),
        rechte_kennung: v["rechte"]["kennung"].as_str().unwrap().into(), rechte_url: opt(&v["rechte"]["url"]),
        laden: v["laden"].as_bool().unwrap(), reihen: strings(&v["reihen"]), auch: strings(&v["auch"]),
        tief: if v["tief"].is_null() { None } else { Some(v["tief"].to_string()) },
    }
}

/// Compare rows with `tief` as JSON, not as text.
fn same(got: &[Row], want: &[Row]) -> bool {
    let norm = |r: &Row| { let mut r = r.clone(); r.tief = r.tief.as_deref().map(|t| serde_json::from_str::<Value>(t).unwrap().to_string()); r };
    got.iter().map(norm).collect::<Vec<_>>() == want.iter().map(norm).collect::<Vec<_>>()
}

// ---------------------------------------------------------------- Musopen

#[test]
fn musopen_rows_match_the_vectors() {
    let all = cases("musopen");
    assert_eq!(all.len(), 11);
    for dir in all {
        let a = std::fs::read(dir.join("antwort.json")).unwrap();
        let want: Vec<Row> = read(&dir.join("erwartet.json"))["zeilen"].as_array().unwrap().iter().map(expected).collect();
        let got = musopen::rows_from_answer(&a).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display())).zeilen;
        assert!(same(&got, &want), "{}:\n got  {got:?}\n want {want:?}", dir.display());
    }
}

#[test]
fn musopen_licence_as_the_app() {
    let table: [(Option<&str>, Option<&str>, Option<&str>); 9] = [
        (Some("pd"), Some("Public domain"), Some("pd")),
        (Some("PD"), Some("Public Domain"), Some("pd")),
        (Some("cc0"), Some("CC0"), Some("cc0")),
        (None, Some("PDM-owner"), Some("pdm")),
        (Some(""), Some("PDM-owner"), Some("pdm")),
        (Some("cc-by-3.0"), Some("PDM-owner"), None),
        (Some("cc-by-sa-3.0"), Some("CC BY-SA 3.0"), None),
        (Some("attribution"), Some("Attribution"), None),
        (None, Some("FAL"), None),
    ];
    for (stated, short, mark) in table {
        assert_eq!(musopen::licence(stated, short), mark, "{stated:?} {short:?}");
    }
}

#[test]
fn musopen_movement_number_as_the_app() {
    let table: [(&str, Option<i64>); 12] = [
        ("Bach - Suite - I. Prelude", Some(1)),
        ("X - IV. Allegro", Some(4)),
        ("X - IX. Air", Some(9)),
        ("X - XIV. Fuga", Some(14)),
        ("X - XL. Late", Some(40)),
        ("X - C) Last", Some(100)),
        ("X - 10. Air", Some(10)),
        ("A - b - 2) Andante", Some(2)),
        ("X - ii. largo", None),
        ("Op. 9, I. Allegro", None),
        ("X - 1 Allegro", None),
        ("X - I.Allegro", None),
    ];
    for (t, n) in table {
        assert_eq!(musopen::movement_number(t), n, "{t}");
    }
}

#[test]
fn musopen_titles_and_ids() {
    for (file, title) in [("File:A - I. B.ogg", "A - I. B"), ("File:x.flac", "x"), ("File:y.webm", "y"), ("File:z.MP3", "z"), ("File:w.oga", "w"), ("File:v.wav", "v"), ("File:u.mid", "u.mid")] {
        assert_eq!(musopen::titel_of(file), title);
    }
}

fn page(pageid: i64, title: &str, url: &str, licence: &str) -> Value {
    json!({"pageid": pageid, "ns": 6, "title": title, "videoinfo": [{"url": url,
        "extmetadata": {"License": {"value": licence}, "LicenseShortName": {"value": licence}}}]})
}

#[test]
fn musopen_a_malformed_answer_is_an_error() {
    for bad in [&b"<html>"[..], br#"{"batchcomplete": true}"#, br#"{"query": {"pages": [{"title": "File:x.ogg"}]}}"#] {
        assert!(matches!(musopen::rows_from_answer(bad), Err(MusopenFehler::Unreadable(_))), "{}", String::from_utf8_lossy(bad));
    }
    // A page without videoinfo is no media file: skipped, but counted as seen.
    let a = json!({"query": {"pages": [{"pageid": 1, "ns": 6, "title": "File:x.pdf"}, page(2, "File:y.ogg", "https://upload.wikimedia.org/y.ogg", "pd")]}});
    let s = musopen::rows_from_answer(a.to_string().as_bytes()).unwrap();
    assert_eq!((s.zeilen.len(), s.gesehen), (1, 2));
}

#[test]
fn musopen_maxlag_postpones() {
    let a = json!({"error": {"code": "maxlag", "info": "Waiting for a database server", "lag": 7}});
    assert_eq!(musopen::rows_from_answer(a.to_string().as_bytes()), Err(MusopenFehler::Maxlag(7)));
}

#[test]
fn musopen_continue_token() {
    let a = json!({"continue": {"gcmcontinue": "file|ABC|123", "continue": "gcmcontinue||"}, "query": {"pages": []}});
    assert_eq!(musopen::rows_from_answer(a.to_string().as_bytes()).unwrap().weiter.as_deref(), Some("file|ABC|123"));
}

// ---------------------------------------------------------------- Radio Browser

#[test]
fn radio_rows_match_the_vectors() {
    let all = cases("radio");
    assert_eq!(all.len(), 8);
    for dir in all {
        let a = std::fs::read(dir.join("antwort.json")).unwrap();
        let want: Vec<Row> = read(&dir.join("erwartet.json"))["zeilen"].as_array().unwrap().iter().map(expected).collect();
        let got = radio::rows_from_page(&a).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display())).zeilen;
        assert!(same(&got, &want), "{}:\n got  {got:?}\n want {want:?}", dir.display());
    }
}

#[test]
fn radio_a_malformed_page_is_an_error() {
    for bad in [&b"<html>"[..], br#"{"stations": []}"#, br#"[{"name": "no uuid", "url": "https://x"}]"#, br#"[{"stationuuid": "", "name": "x", "url": "https://x"}]"#] {
        assert!(radio::rows_from_page(bad).is_err(), "{}", String::from_utf8_lossy(bad));
    }
    let empty = radio::rows_from_page(b"[]").unwrap();
    assert_eq!((empty.zeilen.len(), empty.roh), (0, 0));
}

#[test]
fn radio_no_station_is_ever_loaded() {
    for dir in cases("radio") {
        for r in radio::rows_from_page(&std::fs::read(dir.join("antwort.json")).unwrap()).unwrap().zeilen {
            assert!(!r.laden && r.rechte_kennung == "unbekannt" && r.id.starts_with("radio:rb:") && r.art == "sender");
        }
    }
}

#[test]
fn radio_counts_the_raw_page_and_its_uuids() {
    // Dropped entries (empty name or address) still count for the paging and
    // the count against the statistics; a uuid twice in one page counts once.
    let a = std::fs::read(root("radio").join("doppelte-uuid").join("antwort.json")).unwrap();
    let s = radio::rows_from_page(&a).unwrap();
    assert_eq!((s.roh, s.uuids.len(), s.zeilen.len()), (2, 1, 1));
    let b = std::fs::read(root("radio").join("leerer-name").join("antwort.json")).unwrap();
    let s = radio::rows_from_page(&b).unwrap();
    assert_eq!((s.roh, s.uuids.len(), s.zeilen.len()), (1, 1, 0));
}
