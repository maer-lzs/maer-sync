// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! K3d: the catalog paths of contract version 7, against the text of
//! `CONTRACT.md`. The service reads the catalog file only; the fetch
//! program writes it. Every witness lays down its own throwaway catalog and
//! user store.

use maer_sync::katalog::{Katalog, Row};
use maer_sync::store::Store;
use maer_sync::wege::{Anfrage, Antwort, App};
use rusqlite::Connection;
use serde_json::{json, Value};

const OWNER: &str = "anna";
const SECRET: &str = "ein hinreichend langes geheimnis";

struct Stand {
    app: App,
    token: String,
    katalog: String,
    dir: std::path::PathBuf,
}

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("maer-sync-kw-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn row(quelle: &str, id: &str, titel: &str) -> Row {
    Row {
        quelle: quelle.into(), id: id.into(), art: "hoerbuch".into(), titel: Some(titel.into()),
        urheber: Some("A; B".into()), sprache: Some("eng".into()), jahr: Some(1901),
        rechte_kennung: "pdm".into(), rechte_url: Some("http://creativecommons.org/publicdomain/mark/1.0/".into()),
        laden: true, reihen: vec![], auch: vec![], tief: None,
    }
}

fn rows(quelle: &str, n: usize) -> Vec<Row> {
    (0..n).map(|i| row(quelle, &format!("lva:item-{i:04}"), &format!("Title {i}"))).collect()
}

/// A store with one account, an app with the catalog at `katalog` (written
/// by `fill`, then closed cleanly as the fetch program does).
fn stand(name: &str, fill: impl FnOnce(&mut Katalog)) -> Stand {
    let d = dir(name);
    let store_path = d.join("maer-sync.sqlite");
    let katalog = d.join("katalog.db").to_string_lossy().into_owned();
    let mut store = Store::open(store_path.to_str().unwrap(), 10, None).unwrap();
    store.create_account(OWNER, SECRET).unwrap();
    let token = store.issue_token(OWNER, SECRET, "g1").unwrap().unwrap();
    let mut k = Katalog::open(&katalog).unwrap();
    fill(&mut k);
    k.close_clean().unwrap();
    let mut app = App::neu(store).mit_katalog(&katalog);
    app.plain_ways = true;
    Stand { app, token, katalog, dir: d }
}

fn get(st: &mut Stand, path: &str, query: &str) -> Antwort {
    let a = Anfrage::neu("GET", path).mit_frage(query).mit_kopf("Authorization", &format!("Bearer {}", st.token));
    st.app.http(&a)
}

fn body(a: &Antwort) -> Value {
    a.als_json().unwrap_or_else(|| panic!("no JSON: {} {:?}", a.status, String::from_utf8_lossy(&a.body)))
}

fn ids(v: &Value) -> Vec<String> {
    v["zeilen"].as_array().unwrap().iter().map(|z| z["id"].as_str().unwrap().to_string()).collect()
}

fn dump(path: &str) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut out = vec![];
    let mut s = c.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap();
    let tables: Vec<String> = s.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    for t in tables {
        let mut q = c.prepare(&format!("SELECT * FROM {t} ORDER BY 1")).unwrap();
        let n = q.column_count();
        let mut it = q.query([]).unwrap();
        while let Some(r) = it.next().unwrap() {
            out.push(format!("{t}|{}", (0..n).map(|i| format!("{:?}", r.get_ref(i).unwrap())).collect::<Vec<_>>().join("|")));
        }
    }
    out
}

// ---------------------------------------------------------------- 1 to 4: the path

#[test]
fn the_catalog_sits_behind_the_same_gate() {
    let mut st = stand("gate", |k| { k.write_run("librivox", &rows("librivox", 2), false).unwrap(); });
    let bare = st.app.http(&Anfrage::neu("GET", "/v1/katalog/librivox"));
    assert_eq!(bare.status, 401);
    assert_eq!(bare.kopf("www-authenticate"), Some("Bearer"));
    st.app.plain_ways = false;
    let plain = get(&mut st, "/v1/katalog/librivox", "");
    assert_eq!(plain.status, 426);
}

#[test]
fn without_a_catalog_the_path_is_unknown() {
    let d = dir("none");
    let mut store = Store::open(d.join("s.sqlite").to_str().unwrap(), 10, None).unwrap();
    store.create_account(OWNER, SECRET).unwrap();
    let token = store.issue_token(OWNER, SECRET, "g1").unwrap().unwrap();
    let mut app = App::neu(store);
    app.plain_ways = true;
    let a = app.http(&Anfrage::neu("GET", "/v1/katalog/librivox").mit_kopf("Authorization", &format!("Bearer {token}")));
    assert_eq!((a.status, a.grund().as_deref()), (404, Some("no such path")));
}

#[test]
fn only_known_sources() {
    let mut st = stand("sources", |_| {});
    for q in ["librivox", "archive-buch", "netlabel", "schellack", "otr", "gutenberg", "musopen", "radio", "hoerspiel-feed"] {
        assert_eq!(get(&mut st, &format!("/v1/katalog/{q}"), "").status, 200, "{q}");
    }
    for p in ["/v1/katalog/jamendo", "/v1/katalog/LibriVox", "/v1/katalog/librivox/x", "/v1/katalog/", "/v1/katalog"] {
        assert_eq!(get(&mut st, p, "").status, 404, "{p}");
    }
}

#[test]
fn only_get() {
    let mut st = stand("get", |_| {});
    for m in ["PUT", "POST", "DELETE"] {
        let a = Anfrage::neu(m, "/v1/katalog/librivox").mit_kopf("Authorization", &format!("Bearer {}", st.token));
        assert_eq!(st.app.http(&a).status, 405, "{m}");
    }
}

// ---------------------------------------------------------------- 5, 6: answer and ETag

#[test]
fn full_answer_has_the_contract_shape() {
    let mut st = stand("shape", |k| {
        let mut r = rows("musopen", 2);
        r[1].tief = Some(r#"{"satz": 2, "komponist": "X"}"#.into());
        r[1].reihen = vec!["row:a".into()];
        r[1].auch = vec!["lv:1".into()];
        r[1].laden = false;
        r[1].rechte_kennung = "unbekannt".into();
        r[1].rechte_url = None;
        k.write_run("musopen", &r, false).unwrap();
        k.write_run("musopen", &r[1..], false).unwrap();
    });
    let a = get(&mut st, "/v1/katalog/musopen", "");
    assert_eq!(a.status, 200);
    let v = body(&a);
    assert_eq!(v["katalog_format"], json!(1));
    assert_eq!(v["quelle"], json!("musopen"));
    assert_eq!(v["version"], json!(2));
    assert_eq!(v["weiter"], Value::Null, "weiter is null on the last page and never missing");
    assert_eq!(ids(&v), vec!["lva:item-0001"], "a deletion mark is not in the full state");
    let z = &v["zeilen"][0];
    assert_eq!(z, &json!({
        "id": "lva:item-0001", "art": "hoerbuch", "titel": "Title 1", "urheber": "A; B", "sprache": "eng",
        "jahr": 1901, "rechte": {"kennung": "unbekannt", "url": null}, "laden": false,
        "reihen": ["row:a"], "auch": ["lv:1"], "tief": {"komponist": "X", "satz": 2},
    }));
}

#[test]
fn a_source_before_its_first_run_has_version_zero() {
    let mut st = stand("zero", |_| {});
    let v = body(&get(&mut st, "/v1/katalog/otr", ""));
    assert_eq!((v["version"].clone(), v["zeilen"].clone(), v["weiter"].clone()), (json!(0), json!([]), Value::Null));
    let d = body(&get(&mut st, "/v1/katalog/otr", "seit=0"));
    assert_eq!((d["version"].clone(), d["zeilen"].clone()), (json!(0), json!([])));
}

#[test]
fn etag_per_version_and_304_on_every_page() {
    let mut st = stand("etag", |k| { k.write_run("otr", &rows("otr", 3), false).unwrap(); });
    let a = get(&mut st, "/v1/katalog/otr", "anzahl=2");
    assert_eq!(a.kopf("etag"), Some("\"1-otr-1\""));
    let next = body(&a)["weiter"].as_str().unwrap().to_string();
    for q in ["anzahl=2".to_string(), format!("anzahl=2&nach={next}"), "seit=0".to_string()] {
        let b = st.app.http(&Anfrage::neu("GET", "/v1/katalog/otr").mit_frage(&q)
            .mit_kopf("Authorization", &format!("Bearer {}", st.token)).mit_kopf("If-None-Match", "\"1-otr-1\""));
        assert_eq!(b.status, 304, "{q}");
        assert!(b.body.is_empty(), "{q}");
        assert_eq!(b.kopf("etag"), Some("\"1-otr-1\""), "{q}");
    }
    // A new run: a new ETag, and the old one no longer answers 304.
    let mut k = Katalog::open(&st.katalog).unwrap();
    k.write_run("otr", &rows("otr", 4), false).unwrap();
    k.close_clean().unwrap();
    let c = st.app.http(&Anfrage::neu("GET", "/v1/katalog/otr")
        .mit_kopf("Authorization", &format!("Bearer {}", st.token)).mit_kopf("If-None-Match", "\"1-otr-1\""));
    assert_eq!(c.status, 200);
    assert_eq!(c.kopf("etag"), Some("\"1-otr-2\""));
}

// ---------------------------------------------------------------- 7: pages

#[test]
fn pages_cover_everything_once() {
    let mut st = stand("pages", |k| { k.write_run("schellack", &rows("schellack", 11), false).unwrap(); });
    let whole = ids(&body(&get(&mut st, "/v1/katalog/schellack", "")));
    assert_eq!(whole.len(), 11);
    let mut seen = vec![];
    let mut q = "anzahl=4".to_string();
    for _ in 0..10 {
        let v = body(&get(&mut st, "/v1/katalog/schellack", &q));
        seen.extend(ids(&v));
        match v["weiter"].as_str() {
            Some(w) => q = format!("anzahl=4&nach={w}"),
            None => break,
        }
    }
    assert_eq!(seen, whole);
    for bad in ["anzahl=0", "anzahl=5001", "anzahl=x", "anzahl=-1"] {
        assert_eq!(get(&mut st, "/v1/katalog/schellack", bad).status, 400, "{bad}");
    }
    assert_eq!(get(&mut st, "/v1/katalog/schellack", "anzahl=5000").status, 200);
}

#[test]
fn an_unknown_after_id_continues_after_it() {
    let mut st = stand("after", |k| { k.write_run("otr", &rows("otr", 4), false).unwrap(); });
    let v = body(&get(&mut st, "/v1/katalog/otr", "nach=lva:item-0001x"));
    assert_eq!(ids(&v), vec!["lva:item-0002", "lva:item-0003"]);
}

// ---------------------------------------------------------------- 8: delta

#[test]
fn since_returns_rows_and_bare_marks() {
    let mut st = stand("since", |k| {
        k.write_run("otr", &rows("otr", 3), false).unwrap();
        let mut next = rows("otr", 3)[1..].to_vec();
        next[1].titel = Some("changed".into());
        k.write_run("otr", &next, false).unwrap();
    });
    let v = body(&get(&mut st, "/v1/katalog/otr", "seit=1"));
    assert_eq!(v["version"], json!(2));
    let z = v["zeilen"].as_array().unwrap();
    assert_eq!(z.len(), 2);
    assert_eq!(z[0], json!({"id": "lva:item-0000", "geloescht": true}), "a mark carries exactly id and geloescht");
    assert_eq!(z[1]["id"], json!("lva:item-0002"));
    assert_eq!(z[1]["geloescht"], json!(false));
    assert_eq!(z[1]["titel"], json!("changed"));
    for bad in ["seit=3", "seit=x", "seit=-1"] {
        assert_eq!(get(&mut st, "/v1/katalog/otr", bad).status, 400, "{bad}");
    }
}

#[test]
fn since_below_the_floor_is_410_with_the_way_back() {
    let mut st = stand("floor", |k| {
        k.write_run("otr", &rows("otr", 3), false).unwrap();
        k.write_run("otr", &rows("otr", 3)[1..], false).unwrap();
        k.prune("otr", 2).unwrap();
    });
    let a = get(&mut st, "/v1/katalog/otr", "seit=1");
    assert_eq!(a.status, 410);
    let v = body(&a);
    assert_eq!(v["voll"], json!("/v1/katalog/otr"));
    assert!(v["reason"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(get(&mut st, "/v1/katalog/otr", "seit=2").status, 200);
}

// ---------------------------------------------------------------- 9, 10: unavailable, read only

#[test]
fn an_unavailable_catalog_answers_503() {
    let mut st = stand("gone", |_| {});
    std::fs::remove_file(&st.katalog).unwrap();
    let a = get(&mut st, "/v1/katalog/librivox", "");
    assert_eq!(a.status, 503);
    assert_eq!(a.kopf("retry-after"), Some("300"));
    assert!(st.app.protokoll.iter().any(|z| format!("{z:?}").contains("katalog")), "no line for the failure");
    assert_eq!(st.app.http(&Anfrage::neu("GET", "/v1/health")).status, 200);
    assert!(!std::path::Path::new(&st.katalog).exists(), "the service laid a catalog down");
}

#[test]
fn the_service_reads_from_a_folder_it_cannot_write() {
    use std::os::unix::fs::PermissionsExt;
    let mut st = stand("readonly", |k| { k.write_run("otr", &rows("otr", 5), false).unwrap(); });
    let before = dump(&st.katalog);
    let folder = std::path::Path::new(&st.katalog).parent().unwrap().join("ro");
    std::fs::create_dir_all(&folder).unwrap();
    let moved = folder.join("katalog.db");
    std::fs::rename(&st.katalog, &moved).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    st.app = std::mem::replace(&mut st.app, App::neu(Store::open(st.dir.join("x.sqlite").to_str().unwrap(), 10, None).unwrap()))
        .mit_katalog(moved.to_str().unwrap());
    for q in ["", "anzahl=2", "seit=0"] {
        assert_eq!(get(&mut st, "/v1/katalog/otr", q).status, 200, "{q}");
    }
    let names: Vec<String> = std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["katalog.db".to_string()], "the service wrote beside the catalog");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(dump(moved.to_str().unwrap()), before);
}

// ---------------------------------------------------------------- 13, 14

#[test]
fn a_run_between_two_pages_shows_in_the_version() {
    let mut st = stand("between", |k| { k.write_run("otr", &rows("otr", 4), false).unwrap(); });
    let first = body(&get(&mut st, "/v1/katalog/otr", "anzahl=2"));
    assert_eq!(first["version"], json!(1));
    let mut k = Katalog::open(&st.katalog).unwrap();
    let mut r = rows("otr", 4);
    r[3].titel = Some("moved on".into());
    k.write_run("otr", &r, false).unwrap();
    k.close_clean().unwrap();
    let w = first["weiter"].as_str().unwrap();
    let second = body(&get(&mut st, "/v1/katalog/otr", &format!("anzahl=2&nach={w}")));
    assert_eq!(second["version"], json!(2), "the service hid the new version");
}

#[test]
fn the_catalog_paths_touch_no_user_data() {
    let mut st = stand("nouser", |k| { k.write_run("otr", &rows("otr", 2), false).unwrap(); });
    let store_path = st.dir.join("maer-sync.sqlite");
    let before = dump(store_path.to_str().unwrap());
    for q in ["", "seit=0", "anzahl=1"] {
        let a = get(&mut st, "/v1/katalog/otr", q);
        let text = String::from_utf8_lossy(&a.body).to_string();
        assert!(!text.contains(OWNER) && !text.contains(&st.token), "the answer carries user data");
    }
    assert_eq!(dump(store_path.to_str().unwrap()), before, "the catalog paths wrote to the user store");
    let lines: Vec<String> = st.app.protokoll.iter().map(|z| format!("{z:?}")).collect();
    assert!(lines.iter().any(|l| l.contains("katalog")), "no line for a catalog request: {lines:?}");
    for l in lines.iter().filter(|l| l.contains("katalog")) {
        assert!(!l.contains(OWNER) && !l.contains(&st.token), "a catalog line names the account or the token: {l}");
    }
}

#[test]
#[ignore]
fn page_size_measured() {
    let mut st = stand("size", |k| {
        let r: Vec<Row> = (0..5000).map(|i| {
            let mut x = row("schellack", &format!("lva:78_some-record-title-{i:05}_performer-and-orchestra_gbia{i:07}b"), &format!("A title of a 78 rpm record number {i}"));
            x.reihen = vec!["shellac-jazz".into()];
            x
        }).collect();
        k.write_run("schellack", &r, false).unwrap();
    });
    let a = get(&mut st, "/v1/katalog/schellack", "");
    println!("MESSUNG volle Seite 5000 Zeilen: {} Bytes", a.body.len());
}
