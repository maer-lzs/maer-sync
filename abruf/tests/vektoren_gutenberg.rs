// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The reader of the Gutenberg dump against the fixtures of the app, which come
//! byte for byte from the app's tests: the excerpt of `pg_catalog.csv` and
//! the 132 music works with their art-music verdict.

use std::path::PathBuf;

use maer_sync_abruf::gutenberg::{is_art_music, is_music, parts, rows_from_catalog, Parts};
use serde_json::Value;

fn vektor(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vektoren/katalog/gutenberg").join(name)
}

fn auszug() -> Vec<u8> {
    std::fs::read(vektor("pg_catalog-excerpt.csv")).unwrap()
}

#[test]
fn only_sound_works_become_rows() {
    let rows = rows_from_catalog(&auszug()).unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["gut:3002", "gut:5884"], "the excerpt holds two works of type Sound among fifteen");
    for r in &rows {
        assert_eq!(r.quelle, "gutenberg");
        assert_eq!(r.art, "hoerbuch", "the kind stays a book");
    }
}

#[test]
fn a_title_keeps_its_commas_and_quotes() {
    let rows = rows_from_catalog(&auszug()).unwrap();
    assert_eq!(rows[0].titel.as_deref(), Some("Society's Child (audiofile)"));
    assert_eq!(rows[1].titel.as_deref(), Some("The Flying Saucers are Real"));
}

#[test]
fn a_dump_without_its_columns_is_refused() {
    let ohne_klasse = b"Text#,Type,Issued,Title,Language,Authors,Subjects,Bookshelves\n3002,Sound,2002-01-01,T,en,A,S,\n";
    assert!(rows_from_catalog(ohne_klasse).is_err(), "a dump without LoCC");
    let ohne_kopf = b"3002,Sound,2002-01-01,T,en,A,S,M,\n";
    assert!(rows_from_catalog(ohne_kopf).is_err(), "a dump without its header");
    assert!(rows_from_catalog(b"").is_err(), "an empty dump");
}

#[test]
fn music_is_the_class_m() {
    for (locc, want) in [(Some("M"), true), (Some(" M "), true), (Some("M; PS"), true),
                         (Some("ML"), false), (Some("PR"), false), (Some(""), false), (None, false)] {
        assert_eq!(is_music(locc), want, "{locc:?}");
    }
}

#[test]
fn the_parts_of_a_work() {
    let p = |c: Option<&str>, s: Option<&str>| Parts { composer: c.map(str::to_string), performer: s.map(str::to_string) };
    assert_eq!(parts(Some("Edison Concert Band; Liszt, Franz, 1811-1886 [Composer]")), p(Some("Franz Liszt"), Some("Edison Concert Band")));
    assert_eq!(parts(Some("United States Marine Band [Performer]; Sousa, John Philip, 1854-1932 [Composer]")), p(Some("John Philip Sousa"), Some("United States Marine Band")));
    assert_eq!(parts(Some("Ian, Janis, 1951- [Performer]")), p(Some("Janis Ian"), None));
    assert_eq!(parts(Some("")), p(None, None));
    assert_eq!(parts(None), p(None, None));
}

#[test]
fn a_music_row_names_music_and_art_music_in_tief() {
    let rows = rows_from_catalog(&auszug()).unwrap();
    let tief = |i: usize| -> Value { serde_json::from_str(rows[i].tief.as_deref().expect("tief")).unwrap() };
    // 3002 is music (class M), popular, not art music; its urheber leads.
    assert_eq!(tief(0)["musik"], Value::Bool(true));
    assert_eq!(tief(0)["kunstmusik"], Value::Bool(false));
    assert_eq!(rows[0].urheber.as_deref(), Some("Janis Ian"));
    // 5884 is no music: musik false and no kunstmusik at all.
    assert_eq!(tief(1)["musik"], Value::Bool(false));
    assert!(tief(1).get("kunstmusik").is_none(), "{}", tief(1));
}

#[test]
fn the_132_works_of_the_fixture() {
    let alle: Value = serde_json::from_slice(&std::fs::read(vektor("art-music.json")).unwrap()).unwrap();
    let faelle = alle["faelle"].as_array().unwrap();
    assert_eq!(faelle.len(), 132);
    let mut ja = 0;
    for f in faelle {
        let subjects: Vec<String> = f["subjects"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
        let want = f["kunstmusik"].as_bool().unwrap();
        ja += usize::from(want);
        assert_eq!(is_art_music(&subjects, f["authors"].as_str()), want, "{}", f["id"]);
    }
    assert_eq!(ja, 28);
}

#[test]
fn the_edges_of_art_music() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert!(!is_art_music(&s(&["Operas -- Excerpts", "Popular music -- 1901-1910"]), None), "an opera with a light heading");
    assert!(!is_art_music(&s(&[]), Some("Verdi, Giuseppe, 1813-1901 [Composer]; Caruso, Enrico [Performer]")), "a composer who died 1901");
    assert!(is_art_music(&s(&[]), Some("Liszt, Franz, 1811-1886 [Composer]")), "a composer who died before 1900, no heading");
    assert!(is_art_music(&s(&["Piano music"]), None), "a heading of the art list");
}
