// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The archive.org reader against the shared vectors under
//! `vektoren/katalog/`. The same folders are read by Swift and Kotlin; what a
//! case expects stands in its `erwartet.json`, not here.

use std::path::{Path, PathBuf};

use maer_sync::katalog::{Katalog, Row, Since, MAX_PAGE};
use maer_sync_abruf::archive::{licence_mark, merge_shelves, rows_from_answer, ReadError, Shelf};
use serde_json::{json, Value};

const ARCHIVE_SOURCES: [(&str, usize); 5] =
    [("librivox", 4), ("archive-buch", 5), ("netlabel", 6), ("schellack", 9), ("otr", 6)];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("vektoren").join("katalog")
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn cases(quelle: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join(quelle)).unwrap()
        .map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

fn shelves() -> Vec<Shelf> {
    read_json(&root().join("schellack").join("regale.json"))["regale"].as_array().unwrap().iter()
        .map(|r| Shelf {
            schluessel: r["schluessel"].as_str().unwrap().into(),
            schlagwort: r["schlagwort"].as_str().unwrap().into(),
        })
        .collect()
}

fn shelf(key: &str) -> Shelf {
    shelves().into_iter().find(|s| s.schluessel == key).unwrap_or_else(|| panic!("no shelf {key}"))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

fn opt(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.to_string())
}

fn expected_row(v: &Value) -> Row {
    Row {
        quelle: v["quelle"].as_str().unwrap().into(),
        id: v["id"].as_str().unwrap().into(),
        art: v["art"].as_str().unwrap().into(),
        titel: opt(&v["titel"]),
        urheber: opt(&v["urheber"]),
        sprache: opt(&v["sprache"]),
        jahr: v["jahr"].as_i64(),
        rechte_kennung: v["rechte"]["kennung"].as_str().unwrap().into(),
        rechte_url: opt(&v["rechte"]["url"]),
        laden: v["laden"].as_bool().unwrap(),
        reihen: strings(&v["reihen"]),
        auch: strings(&v["auch"]),
        tief: opt(&v["tief"]),
    }
}

fn error_name(e: &ReadError) -> &'static str {
    match e {
        ReadError::Unreadable(_) => "unlesbar",
        ReadError::ForeignAnswer(_) => "fremde_antwort",
        ReadError::NoShelf => "kein_regal",
        ReadError::UnknownSource(_) => "unbekannte_quelle",
    }
}

/// The rows a case reads: one answer, or several shelf answers merged.
fn read_case(dir: &Path) -> Result<Vec<Row>, ReadError> {
    let fall = read_json(&dir.join("fall.json"));
    let quelle = fall["quelle"].as_str().unwrap();
    let year = fall["kalenderjahr"].as_i64().unwrap() as i32;
    if let Some(keys) = fall["regale"].as_array() {
        let mut parts = vec![];
        for k in keys {
            let k = k.as_str().unwrap();
            let answer = std::fs::read(dir.join(format!("antwort-{k}.json"))).unwrap();
            parts.push(rows_from_answer(quelle, &answer, year, Some(&shelf(k)))?);
        }
        return Ok(merge_shelves(parts));
    }
    let s = fall["regal"].as_str().map(shelf);
    rows_from_answer(quelle, &std::fs::read(dir.join("antwort.json")).unwrap(), year, s.as_ref())
}

// ---------------------------------------------------------------- 1, 2: the vectors

#[test]
fn every_archive_case_has_answer_and_expectation() {
    for (quelle, n) in ARCHIVE_SOURCES {
        let found = cases(quelle);
        assert_eq!(found.len(), n, "{quelle}: {found:?}");
        for dir in found {
            assert!(dir.join("fall.json").is_file(), "{}", dir.display());
            assert!(dir.join("erwartet.json").is_file(), "{}", dir.display());
            let fall = read_json(&dir.join("fall.json"));
            let answers = if dir.ends_with("geloescht") {
                dir.join("vorher.json").is_file() && dir.join("nachher.json").is_file()
            } else if let Some(keys) = fall["regale"].as_array() {
                keys.iter().all(|k| dir.join(format!("antwort-{}.json", k.as_str().unwrap())).is_file())
            } else {
                dir.join("antwort.json").is_file()
            };
            assert!(answers, "answer missing in {}", dir.display());
        }
    }
}

#[test]
fn archive_rows_match_the_vectors() {
    let mut checked = 0;
    for (quelle, _) in ARCHIVE_SOURCES {
        for dir in cases(quelle) {
            if dir.ends_with("geloescht") {
                continue;
            }
            let want = read_json(&dir.join("erwartet.json"));
            let got = read_case(&dir);
            match (want.get("fehler").and_then(|f| f.as_str()), got) {
                (Some(name), Err(e)) => assert_eq!(error_name(&e), name, "{}", dir.display()),
                (Some(name), Ok(rows)) => panic!("{}: expected {name}, read {rows:?}", dir.display()),
                (None, Err(e)) => panic!("{}: {e:?}", dir.display()),
                (None, Ok(rows)) => {
                    let want: Vec<Row> = want["zeilen"].as_array().unwrap().iter().map(expected_row).collect();
                    assert_eq!(rows, want, "{}", dir.display());
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 25);
}

// ---------------------------------------------------------------- 3: deletion through the catalog

#[test]
fn a_deletion_runs_through_the_catalog() {
    for (quelle, _) in ARCHIVE_SOURCES {
        let dir = root().join(quelle).join("geloescht");
        let fall = read_json(&dir.join("fall.json"));
        let s = fall["regal"].as_str().map(shelf);
        let read = |name: &str| rows_from_answer(quelle, &std::fs::read(dir.join(name)).unwrap(), 2026, s.as_ref()).unwrap();
        let p = std::env::temp_dir().join(format!("maer-sync-abruf-del-{quelle}-{}", std::process::id()));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", p.display()));
        }
        let mut k = Katalog::open(p.to_str().unwrap()).unwrap();
        k.write_run(quelle, &read("vorher.json"), false).unwrap();
        k.write_run(quelle, &read("nachher.json"), false).unwrap();
        let Since::Page(page) = k.since(quelle, 0, None, MAX_PAGE).unwrap() else { panic!() };
        let want = read_json(&dir.join("erwartet.json"));
        let want: Vec<(Row, i64, bool)> = want["nach_nachher"].as_array().unwrap().iter()
            .map(|v| (expected_row(v), v["version"].as_i64().unwrap(), v["geloescht"].as_bool().unwrap()))
            .collect();
        let got: Vec<(Row, i64, bool)> = page.zeilen.into_iter().map(|s| (s.row, s.version, s.geloescht)).collect();
        assert_eq!(got, want, "{quelle}");
    }
}

// ---------------------------------------------------------------- 4, 5: licences

#[test]
fn licence_urls_map_to_one_mark() {
    let table: [(Option<&str>, &str); 15] = [
        (Some("http://creativecommons.org/publicdomain/mark/1.0/"), "pdm"),
        (Some("https://creativecommons.org/publicdomain/mark/1.0"), "pdm"),
        (Some("HTTP://CreativeCommons.org/publicdomain/zero/1.0/"), "cc0"),
        (Some("http://creativecommons.org/licenses/publicdomain/"), "pd"),
        (Some("http://creativecommons.org/licenses/by/3.0/"), "cc-by"),
        (Some("http://creativecommons.org/licenses/by-sa/4.0"), "cc-by-sa"),
        (Some("http://creativecommons.org/licenses/by-nc-sa/3.0/"), "cc-by-nc-sa"),
        (Some("http://creativecommons.org/licenses/by-sa-nc/2.0/"), "cc-by-nc-sa"),
        (Some("http://creativecommons.org/licenses/by-nd-nc/1.0/"), "cc-by-nc-nd"),
        (Some("http://creativecommons.org/licenses/by-nc-nd/3.0/us/"), "cc-by-nc-nd"),
        (Some("http://creativecommons.org/licenses/nc/1.0/"), "unbekannt"),
        (Some("http://creativecommons.org/licenses/by-by/3.0/"), "unbekannt"),
        (Some("http://example.org/my-licence"), "unbekannt"),
        (Some(""), "unbekannt"),
        (None, "unbekannt"),
    ];
    for (url, mark) in table {
        assert_eq!(licence_mark(url), mark, "{url:?}");
    }
}

#[test]
fn laden_follows_the_mark() {
    let answer = json!({"items": [
        {"identifier": "a", "title": "A", "collection": ["netlabels"], "licenseurl": "http://creativecommons.org/licenses/by-nc-nd/3.0/"},
        {"identifier": "b", "title": "B", "collection": ["netlabels"], "licenseurl": "http://example.org/x"},
        {"identifier": "c", "title": "C", "collection": ["netlabels"]},
    ]});
    let rows = rows_from_answer("netlabel", answer.to_string().as_bytes(), 2026, None).unwrap();
    let got: Vec<(String, Option<String>, bool)> =
        rows.into_iter().map(|r| (r.rechte_kennung, r.rechte_url, r.laden)).collect();
    assert_eq!(got, vec![
        ("cc-by-nc-nd".into(), Some("http://creativecommons.org/licenses/by-nc-nd/3.0/".into()), true),
        ("unbekannt".into(), Some("http://example.org/x".into()), false),
        ("unbekannt".into(), None, false),
    ]);
}

// ---------------------------------------------------------------- 6: shellac years

#[test]
fn shellac_years_by_calendar() {
    let jazz = shelf("shellac-jazz");
    let doc = |extra: Value| {
        let mut d = json!({"identifier": "78_x", "title": "X", "collection": ["georgeblood"], "subject": ["78rpm", "Jazz"]});
        for (k, v) in extra.as_object().unwrap() {
            d[k] = v.clone();
        }
        json!({"response": {"numFound": 1, "start": 0, "docs": [d]}}).to_string()
    };
    let read = |extra: Value, year: i32| {
        let r = &rows_from_answer("schellack", doc(extra).as_bytes(), year, Some(&jazz)).unwrap()[0];
        (r.jahr, r.rechte_kennung.clone(), r.laden, r.rechte_url.clone())
    };
    // Shellac states no licence, so it is never loaded; the year is only a statement.
    assert_eq!(read(json!({"year": 1925}), 2026), (Some(1925), "pd".into(), false, None));
    assert_eq!(read(json!({"year": 1926}), 2026), (Some(1926), "unbekannt".into(), false, None));
    assert_eq!(read(json!({"year": 1926}), 2027), (Some(1926), "pd".into(), false, None));
    assert_eq!(read(json!({}), 2026), (None, "unbekannt".into(), false, None));
    assert_eq!(read(json!({"year": "1925"}), 2026).0, Some(1925));
    assert_eq!(read(json!({"year": "1925?"}), 2026).0, None);
    assert_eq!(read(json!({"year": "192-"}), 2026).0, None);
    assert_eq!(read(json!({"date": "1925-03-01T00:00:00Z"}), 2026).0, Some(1925));
    assert_eq!(read(json!({"date": "1925"}), 2026).0, Some(1925));
    assert_eq!(read(json!({"date": "19250"}), 2026).0, None);
    assert_eq!(read(json!({"year": 1930, "date": "1925-01-01"}), 2026).0, Some(1930), "year comes before date");
    // A licence URL never makes a shellac record free; only the year does.
    assert_eq!(read(json!({"year": 1940, "licenseurl": "http://creativecommons.org/publicdomain/zero/1.0/"}), 2026),
               (Some(1940), "unbekannt".into(), false, None));
}

// ---------------------------------------------------------------- 7, 8: fields and ids

#[test]
fn shellac_is_never_loaded() {
    // Loading needs a declared licence; the shellac
    // collection declares none, whatever its year says.
    for (quelle, _) in ARCHIVE_SOURCES {
        if quelle != "schellack" {
            continue;
        }
        for dir in cases(quelle) {
            if dir.ends_with("geloescht") {
                continue;
            }
            if let Ok(rows) = read_case(&dir) {
                assert!(rows.iter().all(|r| !r.laden), "{}", dir.display());
            }
        }
    }
}

#[test]
fn several_creators_and_languages_join() {
    let answer = json!({"items": [
        {"identifier": "a", "title": "A", "collection": ["librivoxaudio"], "creator": ["One", "Two"], "language": ["eng", "deu"]},
        {"identifier": "b", "title": "B", "collection": ["librivoxaudio"], "creator": "Single", "language": "eng"},
        {"identifier": "c", "title": "C", "collection": ["librivoxaudio"], "creator": []},
    ]});
    let rows = rows_from_answer("librivox", answer.to_string().as_bytes(), 2026, None).unwrap();
    let got: Vec<(Option<String>, Option<String>)> = rows.into_iter().map(|r| (r.urheber, r.sprache)).collect();
    assert_eq!(got, vec![
        (Some("One; Two".into()), Some("eng; deu".into())),
        (Some("Single".into()), Some("eng".into())),
        (None, None),
    ]);
}

#[test]
fn ids_carry_the_app_prefix() {
    let answer = json!({"items": [
        {"identifier": "008ArnaudB.-Cookaloneeatinfrontoftv", "collection": ["netlabels"]},
    ]});
    let rows = rows_from_answer("netlabel", answer.to_string().as_bytes(), 2026, None).unwrap();
    assert_eq!(rows[0].id, "lva:008ArnaudB.-Cookaloneeatinfrontoftv");
    assert_eq!(rows[0].quelle, "netlabel");
    assert_eq!(rows[0].art, "musik");
}

// ---------------------------------------------------------------- 9: malformed is not empty

#[test]
fn a_malformed_answer_is_an_error_not_an_empty_run() {
    let bad: [&[u8]; 6] = [
        b"<html>maintenance</html>",
        br#"{"count": 0}"#,
        br#"{"items": {"identifier": "a"}}"#,
        br#"{"items": [{"title": "no identifier", "collection": ["oldtimeradio"]}]}"#,
        br#"{"items": [{"identifier": "", "collection": ["oldtimeradio"]}]}"#,
        br#"{"response": {"numFound": 1}}"#,
    ];
    for b in bad {
        match rows_from_answer("otr", b, 2026, None) {
            Err(ReadError::Unreadable(_)) => {}
            other => panic!("{}: {other:?}", String::from_utf8_lossy(b)),
        }
    }
    for empty in [br#"{"items": [], "count": 0}"#.as_slice(), br#"{"response": {"numFound": 0, "docs": []}}"#.as_slice()] {
        assert_eq!(rows_from_answer("otr", empty, 2026, None).unwrap(), vec![]);
    }
    assert!(matches!(rows_from_answer("jamendo", br#"{"items": []}"#, 2026, None), Err(ReadError::UnknownSource(_))));
}

// ---------------------------------------------------------------- 19: shellac only from shelves

#[test]
fn shellac_is_read_only_from_a_shelf() {
    let answer = json!({"response": {"docs": [
        {"identifier": "78_y", "collection": ["georgeblood"], "subject": ["78rpm", "Waltz"], "year": 1920},
    ]}}).to_string();
    assert_eq!(rows_from_answer("schellack", answer.as_bytes(), 2026, None), Err(ReadError::NoShelf));
    // A record without the shelf's subject is not on the shelf: the answer
    // belongs to another query.
    assert!(matches!(rows_from_answer("schellack", answer.as_bytes(), 2026, Some(&shelf("shellac-jazz"))),
                     Err(ReadError::ForeignAnswer(_))));
    // The subject is a word, not a part of one, and its case does not count.
    let fits = |subject: &str| {
        let a = json!({"response": {"docs": [
            {"identifier": "78_z", "collection": ["georgeblood"], "subject": ["78rpm", subject]},
        ]}}).to_string();
        rows_from_answer("schellack", a.as_bytes(), 2026, Some(&shelf("shellac-jazz"))).is_ok()
    };
    assert!(fits("Jazz"));
    assert!(fits("Jazz vocal"));
    assert!(fits("Dixieland jazz"));
    assert!(!fits("Jazzy"));
    // A shelf belongs to shellac only.
    let lv = json!({"items": [{"identifier": "a", "collection": ["librivoxaudio"]}]}).to_string();
    assert!(rows_from_answer("librivox", lv.as_bytes(), 2026, Some(&shelf("shellac-jazz"))).is_err());
}

#[test]
fn every_shelf_of_the_app_is_listed() {
    let keys: Vec<String> = shelves().into_iter().map(|s| s.schluessel).collect();
    assert_eq!(keys, vec![
        "shellac-jazz", "shellac-opera", "free-classical", "shellac-folk", "shellac-blues",
        "shellac-country", "shellac-tango", "shellac-swing", "shellac-gospel",
    ]);
}

#[test]
fn a_record_of_another_collection_is_a_foreign_answer() {
    let a = json!({"items": [{"identifier": "a", "collection": ["librivoxaudio"]}]}).to_string();
    assert!(matches!(rows_from_answer("netlabel", a.as_bytes(), 2026, None), Err(ReadError::ForeignAnswer(_))));
    let b = json!({"items": [{"identifier": "a"}]}).to_string();
    assert!(matches!(rows_from_answer("netlabel", b.as_bytes(), 2026, None), Err(ReadError::ForeignAnswer(_))),
            "a record that names no collection cannot prove it belongs");
}

// ---------------------------------------------------------------- rows carry no rotation shelves yet

#[test]
fn rows_carry_no_rows_before_the_rotation() {
    for (quelle, _) in ARCHIVE_SOURCES {
        for dir in cases(quelle) {
            if dir.ends_with("geloescht") {
                continue;
            }
            let Ok(rows) = read_case(&dir) else { continue };
            for r in rows {
                if quelle == "schellack" {
                    assert!(!r.reihen.is_empty() && r.reihen.iter().all(|k| shelves().iter().any(|s| &s.schluessel == k)),
                            "{}: {:?}", dir.display(), r.reihen);
                } else {
                    assert!(r.reihen.is_empty(), "{}: {:?}", dir.display(), r.reihen);
                }
                assert!(r.auch.is_empty() && r.tief.is_none(), "{}", dir.display());
            }
        }
    }
}

#[test]
fn the_program_shelves_are_the_vector_shelves() {
    // The program carries its own list of the theme shelves; the vectors keep
    // a copy for the device sides. Both name the app as their source
    // (`_quelle`): when they differ, the one behind the app is brought to it.
    let program: Value = serde_json::from_str(maer_sync_abruf::lauf::REGALE_JSON).unwrap();
    let vector = read_json(&root().join("schellack").join("regale.json"));
    assert!(program["regale"].as_array().is_some_and(|r| !r.is_empty()), "the program lists no shelf");
    assert_eq!(program, vector,
        "the shelves of the program and of vektoren/katalog/schellack/regale.json differ; the app is the source, bring the file behind it up to date");
}
