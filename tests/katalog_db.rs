// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The catalog file `katalog.db`.
//!
//! The catalog is rebuildable from its sources and lives in a file of its
//! own, next to the user store and never inside it. One writer (the fetch
//! program) applies whole runs; the service only reads. These witnesses fix
//! the shape, the checks of the table, what a run writes (versions and
//! deletion marks), how a reader pages, and the separation from the user
//! store.

use maer_sync::katalog::{Katalog, KatalogError, Row, Since, MASS_DELETE_MIN_ROWS, MAX_PAGE};
use maer_sync::store::Store;
use rusqlite::Connection;

fn scratch(name: &str) -> String {
    let p = std::env::temp_dir().join(format!("maer-sync-katalog-{name}-{}", std::process::id()));
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", p.to_string_lossy()));
    }
    p.to_string_lossy().into_owned()
}

fn row(quelle: &str, id: &str, titel: &str) -> Row {
    Row {
        quelle: quelle.into(),
        id: id.into(),
        art: "hoerbuch".into(),
        titel: Some(titel.into()),
        urheber: None,
        sprache: Some("en".into()),
        jahr: None,
        rechte_kennung: "pd".into(),
        rechte_url: None,
        laden: true,
        reihen: vec![],
        auch: vec![],
        tief: None, beliebt_stufe: None,
    }
}

fn rows(quelle: &str, n: usize) -> Vec<Row> {
    (0..n).map(|i| row(quelle, &format!("lva:item-{i:04}"), &format!("Title {i}"))).collect()
}

/// Every row of the file as text, in a fixed order: "byte-identical" is
/// measured on the content, not on the file (WAL changes the file).
fn dump(path: &str) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut out = vec![];
    for t in ["meta", "stand", "eintrag", "cover"] {
        let mut s = c.prepare(&format!("SELECT * FROM {t} ORDER BY 1, 2")).unwrap();
        let n = s.column_count();
        let mut q = s.query([]).unwrap();
        while let Some(r) = q.next().unwrap() {
            let cells: Vec<String> = (0..n)
                .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                .collect();
            out.push(format!("{t}|{}", cells.join("|")));
        }
    }
    out
}

fn tables(path: &str) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut s = c.prepare("SELECT name FROM sqlite_master WHERE type IN ('table','index') ORDER BY name").unwrap();
    let names: Vec<String> = s.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    names
}

fn ids(page: &[maer_sync::katalog::Stored]) -> Vec<String> {
    page.iter().map(|s| s.row.id.clone()).collect()
}

// ---------------------------------------------------------------- shape

#[test]
fn a_fresh_catalog_carries_its_shape() {
    let p = scratch("shape");
    let _k = Katalog::open(&p).expect("open");
    let t = tables(&p);
    for name in ["meta", "stand", "eintrag", "cover", "eintrag_delta"] {
        assert!(t.contains(&name.to_string()), "{name} missing in {t:?}");
    }
    let c = Connection::open(&p).unwrap();
    let format: String = c.query_row("SELECT value FROM meta WHERE key='katalog_format'", [], |r| r.get(0)).unwrap();
    assert_eq!(format, "1");
    let mode: String = c.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
}

#[test]
fn opening_again_keeps_the_rows() {
    let p = scratch("again");
    {
        let mut k = Katalog::open(&p).unwrap();
        k.write_run("librivox", &rows("librivox", 3), false).unwrap();
    }
    let before = dump(&p);
    let _k = Katalog::open(&p).unwrap();
    assert_eq!(dump(&p), before);
}

#[test]
fn an_unknown_format_is_refused() {
    let p = scratch("format");
    drop(Katalog::open(&p).unwrap());
    Connection::open(&p).unwrap()
        .execute("UPDATE meta SET value='2' WHERE key='katalog_format'", []).unwrap();
    let before = dump(&p);
    match Katalog::open(&p) {
        Err(KatalogError::UnknownFormat(found)) => assert_eq!(found, "2"),
        Err(e) => panic!("wrong error: {e:?}"),
        Ok(_) => panic!("a catalog of format 2 was opened"),
    }
    assert_eq!(dump(&p), before);
}

#[test]
fn the_user_store_is_refused_as_catalog() {
    let p = scratch("userstore");
    drop(Store::open(&p, 10, None).unwrap());
    let before = tables(&p);
    match Katalog::open(&p) {
        Err(KatalogError::UserStore) => {}
        Err(e) => panic!("wrong error: {e:?}"),
        Ok(_) => panic!("the user store was opened as catalog"),
    }
    assert_eq!(tables(&p), before, "the user store was written to");
}

// ---------------------------------------------------------------- checks of the table

#[test]
fn only_known_sources() {
    let p = scratch("sources");
    let mut k = Katalog::open(&p).unwrap();
    for q in ["librivox", "archive-buch", "netlabel", "schellack", "otr", "gutenberg", "musopen", "radio", "hoerspiel-feed"] {
        k.write_run(q, &[row(q, "x:1", "t")], false).unwrap_or_else(|e| panic!("{q} refused: {e:?}"));
    }
    for q in ["jamendo", ""] {
        assert!(k.write_run(q, &[row(q, "x:1", "t")], false).is_err(), "{q:?} accepted");
    }
    // The table itself refuses it too, not only the code in front of it.
    let c = Connection::open(&p).unwrap();
    let raw = c.execute(
        "INSERT INTO eintrag (quelle,id,art,rechte_kennung,laden,version) VALUES ('jamendo','j:1','musik','pd',1,1)", []);
    assert!(raw.is_err(), "the table took an unknown source");
}

#[test]
fn only_known_kinds_and_flags() {
    let p = scratch("kinds");
    let mut k = Katalog::open(&p).unwrap();
    let mut bad = row("librivox", "lva:a", "t");
    bad.art = "film".into();
    assert!(k.write_run("librivox", &[bad], false).is_err());
    let c = Connection::open(&p).unwrap();
    for sql in [
        "INSERT INTO eintrag (quelle,id,art,rechte_kennung,laden,version) VALUES ('librivox','a','film','pd',1,1)",
        "INSERT INTO eintrag (quelle,id,art,rechte_kennung,laden,version) VALUES ('librivox','b','hoerbuch','pd',2,1)",
        "INSERT INTO eintrag (quelle,id,art,rechte_kennung,laden,version,geloescht) VALUES ('librivox','c','hoerbuch','pd',1,1,5)",
    ] {
        assert!(c.execute(sql, []).is_err(), "the table took: {sql}");
    }
}

#[test]
fn one_row_per_source_and_id() {
    let p = scratch("unique");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &[row("librivox", "lva:a", "one")], false).unwrap();
    k.write_run("librivox", &[row("librivox", "lva:a", "two")], false).unwrap();
    let c = Connection::open(&p).unwrap();
    let n: i64 = c.query_row("SELECT count(*) FROM eintrag WHERE quelle='librivox' AND id='lva:a'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    let raw = c.execute(
        "INSERT INTO eintrag (quelle,id,art,rechte_kennung,laden,version) VALUES ('librivox','lva:a','hoerbuch','pd',1,9)", []);
    assert!(raw.is_err(), "a second row for the same key");
    // The same id twice within one run is ambiguous and refused.
    assert!(k.write_run("librivox", &[row("librivox", "lva:b", "x"), row("librivox", "lva:b", "y")], false).is_err());
}

// ---------------------------------------------------------------- a run

#[test]
fn a_run_raises_the_version_once() {
    let p = scratch("once");
    let mut k = Katalog::open(&p).unwrap();
    assert_eq!(k.version("librivox").unwrap(), 0);
    assert_eq!(k.write_run("librivox", &rows("librivox", 7), false).unwrap(), 1);
    let mut next = rows("librivox", 7);
    next[0].titel = Some("changed".into());
    next[5].titel = Some("changed too".into());
    assert_eq!(k.write_run("librivox", &next, false).unwrap(), 2);
    assert_eq!(k.version("librivox").unwrap(), 2);
}

#[test]
fn a_run_without_change_keeps_the_version() {
    // Raising the version on a run that changed nothing would turn every
    // device's ETag stale every day for nothing.
    let p = scratch("nochange");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &rows("librivox", 4), false).unwrap();
    let before = dump(&p);
    assert_eq!(k.write_run("librivox", &rows("librivox", 4), false).unwrap(), 1);
    assert_eq!(dump(&p), before);
}

#[test]
fn changed_rows_take_the_new_version_unchanged_keep_theirs() {
    let p = scratch("changed");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &rows("librivox", 5), false).unwrap();
    let mut next = rows("librivox", 5);
    next[1].titel = Some("new title".into());
    next[2].rechte_kennung = "cc0".into();
    next[3].reihen = vec!["row:classics".into()];
    next[4].auch = vec!["lv:123".into()];
    k.write_run("librivox", &next, false).unwrap();
    let all = k.since("librivox", 0, None, MAX_PAGE).unwrap();
    let Since::Page(page) = all else { panic!("since 0 must answer a page") };
    let v: Vec<(String, i64)> = page.zeilen.iter().map(|s| (s.row.id.clone(), s.version)).collect();
    assert_eq!(v, vec![
        ("lva:item-0000".into(), 1),
        ("lva:item-0001".into(), 2),
        ("lva:item-0002".into(), 2),
        ("lva:item-0003".into(), 2),
        ("lva:item-0004".into(), 2),
    ]);
}

#[test]
fn a_vanished_row_becomes_a_deletion_mark() {
    let p = scratch("vanished");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &rows("librivox", 3), false).unwrap();
    k.write_run("librivox", &rows("librivox", 3)[1..], false).unwrap();
    let Since::Page(page) = k.since("librivox", 1, None, MAX_PAGE).unwrap() else { panic!() };
    assert_eq!(page.zeilen.len(), 1);
    assert_eq!(page.zeilen[0].row.id, "lva:item-0000");
    assert!(page.zeilen[0].geloescht);
    assert_eq!(page.zeilen[0].version, 2);
    let c = Connection::open(&p).unwrap();
    let n: i64 = c.query_row("SELECT count(*) FROM eintrag", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 3, "the mark was removed instead of kept");
}

#[test]
fn a_returning_row_is_live_again() {
    let p = scratch("returning");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &rows("librivox", 3), false).unwrap();
    k.write_run("librivox", &rows("librivox", 3)[1..], false).unwrap();
    k.write_run("librivox", &rows("librivox", 3), false).unwrap();
    let Since::Page(page) = k.since("librivox", 2, None, MAX_PAGE).unwrap() else { panic!() };
    assert_eq!(ids(&page.zeilen), vec!["lva:item-0000".to_string()]);
    assert!(!page.zeilen[0].geloescht);
    assert_eq!(page.zeilen[0].version, 3);
}

#[test]
fn sources_count_apart() {
    let p = scratch("apart");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &rows("librivox", 2), false).unwrap();
    let before = k.full("librivox", None, MAX_PAGE).unwrap();
    k.write_run("gutenberg", &rows("gutenberg", 2), false).unwrap();
    k.write_run("gutenberg", &rows("gutenberg", 1), false).unwrap();
    assert_eq!(k.version("librivox").unwrap(), 1);
    assert_eq!(k.version("gutenberg").unwrap(), 2);
    assert_eq!(k.full("librivox", None, MAX_PAGE).unwrap(), before);
}

#[test]
fn a_failed_run_changes_nothing() {
    let p = scratch("failed");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &rows("librivox", 4), false).unwrap();
    let before = dump(&p);
    let mut next = rows("librivox", 4);
    next[0].titel = Some("would change".into());
    next[2].art = "film".into();
    assert!(k.write_run("librivox", &next, false).is_err());
    assert_eq!(dump(&p), before);
}

#[test]
fn an_empty_run_is_refused() {
    let p = scratch("empty");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("hoerspiel-feed", &rows("hoerspiel-feed", 3), false).unwrap();
    let before = dump(&p);
    match k.write_run("hoerspiel-feed", &[], false) {
        Err(KatalogError::EmptyRun) => {}
        other => panic!("an empty run was not refused: {other:?}"),
    }
    assert_eq!(dump(&p), before);
    // Only the caller's explicit sign lets a source become empty.
    assert_eq!(k.write_run("hoerspiel-feed", &[], true).unwrap(), 2);
    assert_eq!(k.full("hoerspiel-feed", None, MAX_PAGE).unwrap().zeilen.len(), 0);
}

#[test]
fn a_mass_deletion_is_refused_at_its_bound() {
    // More than a fifth of the live rows of a source at once looks like a
    // run that broke off after its first page, not like the source.
    let p = scratch("mass");
    let mut k = Katalog::open(&p).unwrap();
    let n = 100;
    k.write_run("netlabel", &rows("netlabel", n), false).unwrap();
    // Exactly a fifth: allowed.
    k.write_run("netlabel", &rows("netlabel", n)[20..], false).unwrap();
    // 80 live now; 17 would go (more than 16): refused, nothing changes.
    let before = dump(&p);
    match k.write_run("netlabel", &rows("netlabel", n)[37..], false) {
        Err(KatalogError::MassDelete { would_delete, live }) => {
            assert_eq!((would_delete, live), (17, 80));
        }
        other => panic!("a mass deletion was not refused: {other:?}"),
    }
    assert_eq!(dump(&p), before);
    // The caller's explicit sign overrules it.
    k.write_run("netlabel", &rows("netlabel", n)[37..], true).unwrap();
    assert_eq!(k.full("netlabel", None, MAX_PAGE).unwrap().zeilen.len(), 63);
}

#[test]
fn a_small_source_has_no_mass_bound() {
    let p = scratch("small");
    let mut k = Katalog::open(&p).unwrap();
    let n = MASS_DELETE_MIN_ROWS - 1;
    k.write_run("hoerspiel-feed", &rows("hoerspiel-feed", n), false).unwrap();
    k.write_run("hoerspiel-feed", &rows("hoerspiel-feed", n)[..1], false).unwrap();
    assert_eq!(k.full("hoerspiel-feed", None, MAX_PAGE).unwrap().zeilen.len(), 1);
}

#[test]
fn a_row_of_another_source_is_refused() {
    let p = scratch("foreign");
    let mut k = Katalog::open(&p).unwrap();
    assert!(k.write_run("librivox", &[row("gutenberg", "gut:1", "t")], false).is_err());
}

// ---------------------------------------------------------------- reading

#[test]
fn full_reads_live_rows_in_id_order_without_marks() {
    let p = scratch("full");
    let mut k = Katalog::open(&p).unwrap();
    let mut r = rows("otr", 4);
    r.reverse();
    k.write_run("otr", &r, false).unwrap();
    k.write_run("otr", &r[..3], false).unwrap();
    let page = k.full("otr", None, MAX_PAGE).unwrap();
    assert_eq!(ids(&page.zeilen), vec!["lva:item-0001", "lva:item-0002", "lva:item-0003"]);
    assert!(page.zeilen.iter().all(|s| !s.geloescht));
    assert_eq!(page.version, 2);
    assert_eq!(page.weiter, None);
}

#[test]
fn pages_cover_everything_once() {
    let p = scratch("pages");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("schellack", &rows("schellack", 23), false).unwrap();
    let whole = ids(&k.full("schellack", None, MAX_PAGE).unwrap().zeilen);
    let mut seen = vec![];
    let mut nach: Option<String> = None;
    let mut pages = 0;
    loop {
        let page = k.full("schellack", nach.as_deref(), 5).unwrap();
        pages += 1;
        seen.extend(ids(&page.zeilen));
        match page.weiter {
            Some(w) => {
                assert_eq!(Some(&w), seen.last(), "weiter is the last id of the page");
                nach = Some(w);
            }
            None => break,
        }
        assert!(pages < 10, "paging does not end");
    }
    assert_eq!(seen, whole);
    assert_eq!(pages, 5);
}

#[test]
fn a_full_last_page_ends_paging() {
    // A last page that is exactly full must still say it is the last.
    let p = scratch("exact");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("schellack", &rows("schellack", 10), false).unwrap();
    let first = k.full("schellack", None, 5).unwrap();
    assert!(first.weiter.is_some());
    let second = k.full("schellack", first.weiter.as_deref(), 5).unwrap();
    assert_eq!(second.zeilen.len(), 5);
    assert_eq!(second.weiter, None);
}

#[test]
fn page_size_is_bounded() {
    let p = scratch("size");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 1), false).unwrap();
    for bad in [0, MAX_PAGE + 1] {
        match k.full("otr", None, bad) {
            Err(KatalogError::PageSize(n)) => assert_eq!(n, bad),
            other => panic!("page size {bad} accepted: {other:?}"),
        }
        assert!(matches!(k.since("otr", 0, None, bad), Err(KatalogError::PageSize(_))));
    }
    assert_eq!(MAX_PAGE, 5000);
    assert!(k.full("otr", None, MAX_PAGE).is_ok());
}

#[test]
fn a_missing_after_id_continues_after_it() {
    let p = scratch("missing");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 4), false).unwrap();
    let page = k.full("otr", Some("lva:item-0001x"), MAX_PAGE).unwrap();
    assert_eq!(ids(&page.zeilen), vec!["lva:item-0002", "lva:item-0003"]);
}

#[test]
fn since_returns_newer_rows_with_marks() {
    let p = scratch("since");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 4), false).unwrap();
    let mut next = rows("otr", 4)[1..].to_vec();
    next[2].titel = Some("changed".into());
    k.write_run("otr", &next, false).unwrap();
    let Since::Page(page) = k.since("otr", 1, None, MAX_PAGE).unwrap() else { panic!() };
    let got: Vec<(String, bool, i64)> = page.zeilen.iter().map(|s| (s.row.id.clone(), s.geloescht, s.version)).collect();
    assert_eq!(got, vec![("lva:item-0000".into(), true, 2), ("lva:item-0003".into(), false, 2)]);
    let Since::Page(none) = k.since("otr", 2, None, MAX_PAGE).unwrap() else { panic!() };
    assert!(none.zeilen.is_empty());
    assert_eq!(none.version, 2);
}

#[test]
fn since_below_the_floor_says_too_old() {
    let p = scratch("floor");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 3), false).unwrap();
    k.write_run("otr", &rows("otr", 3)[1..], false).unwrap();
    k.write_run("otr", &rows("otr", 3)[2..], false).unwrap();
    k.prune("otr", 2).unwrap();
    assert!(matches!(k.since("otr", 1, None, MAX_PAGE).unwrap(), Since::TooOld { floor: 2 }));
    assert!(matches!(k.since("otr", 2, None, MAX_PAGE).unwrap(), Since::Page(_)));
    assert!(matches!(k.since("otr", 4, None, MAX_PAGE).unwrap(), Since::Unknown { version: 3 }));
}

#[test]
fn prune_removes_only_marks_below_the_bound() {
    let p = scratch("prune");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 4), false).unwrap(); // v1: 0..3 live
    k.write_run("otr", &rows("otr", 4)[1..], false).unwrap(); // v2: 0 marked
    k.write_run("otr", &rows("otr", 4)[2..], false).unwrap(); // v3: 1 marked
    assert_eq!(k.prune("otr", 2).unwrap(), 1, "only the mark of version 2 goes");
    let c = Connection::open(&p).unwrap();
    let left: Vec<(String, i64, i64)> = c
        .prepare("SELECT id, version, geloescht FROM eintrag WHERE quelle='otr' ORDER BY id").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
        .map(|x| x.unwrap()).collect();
    assert_eq!(left, vec![
        ("lva:item-0001".into(), 3, 1),
        ("lva:item-0002".into(), 1, 0),
        ("lva:item-0003".into(), 1, 0),
    ], "a live row of an old version was removed, or the wrong mark");
    let floor: i64 = c.query_row("SELECT voll_ab FROM stand WHERE quelle='otr'", [], |r| r.get(0)).unwrap();
    assert_eq!(floor, 2);
}

#[test]
fn lists_round_trip() {
    let p = scratch("lists");
    let mut k = Katalog::open(&p).unwrap();
    let mut r = row("musopen", "mso:1", "Satz");
    r.reihen = vec!["row:a".into(), "row:b".into()];
    r.auch = vec!["lv:42".into()];
    k.write_run("musopen", &[r.clone()], false).unwrap();
    let got = k.full("musopen", None, MAX_PAGE).unwrap();
    assert_eq!(got.zeilen[0].row, r);
    assert_eq!(got.zeilen[0].row.tief, None);
}

// ---------------------------------------------------------------- separation and readers

#[test]
fn backup_of_the_user_store_carries_no_catalog() {
    let dir = scratch("sep");
    std::fs::create_dir_all(&dir).unwrap();
    let store_path = format!("{dir}/maer-sync.sqlite");
    let katalog_path = format!("{dir}/katalog.db");
    let copy = format!("{dir}/copy.sqlite");
    let store = Store::open(&store_path, 10, None).unwrap();
    let mut k = Katalog::open(&katalog_path).unwrap();
    k.write_run("librivox", &rows("librivox", 3), false).unwrap();
    let before = dump(&katalog_path);
    store.backup(&copy).unwrap();
    let t = tables(&copy);
    for name in ["eintrag", "stand", "cover"] {
        assert!(!t.contains(&name.to_string()), "{name} is in the user backup");
    }
    assert_eq!(dump(&katalog_path), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_reader_cannot_write() {
    let p = scratch("reader");
    drop(Katalog::open(&p).unwrap());
    let mut r = Katalog::open_read(&p).unwrap();
    assert!(r.write_run("librivox", &rows("librivox", 1), false).is_err());
    assert!(r.prune("librivox", 1).is_err());
    assert_eq!(Katalog::open(&p).unwrap().version("librivox").unwrap(), 0);
}

#[test]
fn a_reader_sees_whole_runs() {
    let p = scratch("whole");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 3), false).unwrap();
    let reader = Katalog::open_read(&p).unwrap();
    // A second connection holds a run half written: the version raised and
    // one row changed, not yet committed.
    let mut w = Connection::open(&p).unwrap();
    let tx = w.transaction().unwrap();
    tx.execute("UPDATE stand SET version = 2 WHERE quelle='otr'", []).unwrap();
    tx.execute("UPDATE eintrag SET titel='half', version=2 WHERE id='lva:item-0000'", []).unwrap();
    let during = reader.full("otr", None, MAX_PAGE).unwrap();
    assert_eq!(during.version, 1);
    assert!(during.zeilen.iter().all(|s| s.version == 1));
    tx.commit().unwrap();
    let after = reader.full("otr", None, MAX_PAGE).unwrap();
    assert_eq!(after.version, 2);
    assert_eq!(after.zeilen[0].row.titel.as_deref(), Some("half"));
}

#[test]
fn a_second_writer_waits_or_fails_by_name() {
    let p = scratch("writers");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 3), false).unwrap();
    let mut other = Katalog::open(&p).unwrap();
    other.set_wait_ms(200).expect("the wait is set");
    // The first writer holds its run open.
    let mut w = Connection::open(&p).unwrap();
    let tx = w.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
    let before = dump(&p);
    let mut next = rows("otr", 3);
    next[0].titel = Some("second writer".into());
    match other.write_run("otr", &next, false) {
        Err(KatalogError::Busy) => {}
        other => panic!("a second writer was not refused by name: {other:?}"),
    }
    drop(tx);
    assert_eq!(dump(&p), before, "a half run was left");
    assert_eq!(other.write_run("otr", &next, false).unwrap(), 2);
}

#[test]
fn a_reordered_list_keeps_the_version() {
    // The order of a list and the order of keys in the deep part carry no
    // meaning; a run that only reorders them must not turn every ETag stale.
    let p = scratch("reorder");
    let mut k = Katalog::open(&p).unwrap();
    let mut r = row("schellack", "lva:78_a", "t");
    r.reihen = vec!["shellac-swing".into(), "shellac-jazz".into()];
    r.auch = vec!["lv:2".into(), "lv:1".into()];
    r.tief = Some(r#"{"satz": 2, "komponist": "X"}"#.into());
    assert_eq!(k.write_run("schellack", &[r.clone()], false).unwrap(), 1);
    let mut again = r.clone();
    again.reihen = vec!["shellac-jazz".into(), "shellac-swing".into()];
    again.auch = vec!["lv:1".into(), "lv:2".into(), "lv:1".into()];
    again.tief = Some(r#"{"komponist":"X","satz":2}"#.into());
    assert_eq!(k.write_run("schellack", &[again], false).unwrap(), 1);
    let stored = &k.full("schellack", None, MAX_PAGE).unwrap().zeilen[0].row;
    assert_eq!(stored.reihen, vec!["shellac-jazz".to_string(), "shellac-swing".to_string()]);
    assert_eq!(stored.auch, vec!["lv:1".to_string(), "lv:2".to_string()]);
    assert_eq!(stored.tief.as_deref(), Some(r#"{"komponist":"X","satz":2}"#));
    // Deep text that is not JSON is refused, not stored.
    let mut bad = r.clone();
    bad.tief = Some("not json".into());
    assert!(k.write_run("schellack", &[bad], false).is_err());
}

// ---------------------------------------------------------------- delta and run times

#[test]
fn write_delta_keeps_absent_rows() {
    let p = scratch("delta");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 3), false).unwrap();
    let mut d = vec![row("otr", "lva:new", "new")];
    let mut changed = rows("otr", 3)[1].clone();
    changed.titel = Some("changed".into());
    d.push(changed);
    assert_eq!(k.write_delta("otr", &d).unwrap(), 2);
    let page = k.full("otr", None, MAX_PAGE).unwrap();
    assert_eq!(page.zeilen.len(), 4, "a delta deleted rows");
    assert!(page.zeilen.iter().all(|s| !s.geloescht));
    // Nothing changed: the version stays.
    assert_eq!(k.write_delta("otr", &d).unwrap(), 2);
    // An empty delta is no change, not an empty run.
    assert_eq!(k.write_delta("otr", &[]).unwrap(), 2);
}

#[test]
fn a_mark_that_returns_in_a_delta_lives_again() {
    let p = scratch("delta-mark");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("otr", &rows("otr", 3), false).unwrap();
    k.write_run("otr", &rows("otr", 3)[1..], false).unwrap();
    assert_eq!(k.write_delta("otr", &rows("otr", 3)[..1]).unwrap(), 3);
    let Since::Page(page) = k.since("otr", 2, None, MAX_PAGE).unwrap() else { panic!() };
    assert_eq!(ids(&page.zeilen), vec!["lva:item-0000".to_string()]);
    assert!(!page.zeilen[0].geloescht);
    // A row of another source is refused, as in write_run.
    assert!(k.write_delta("otr", &[row("gutenberg", "gut:1", "t")]).is_err());
    let mut bad = row("otr", "lva:x", "t");
    bad.art = "film".into();
    assert!(k.write_delta("otr", &[bad]).is_err());
}

#[test]
fn run_times_survive_reopening() {
    let p = scratch("times");
    {
        let mut k = Katalog::open(&p).unwrap();
        assert_eq!(k.last_run("otr", "voll").unwrap(), None);
        k.set_last_run("otr", "voll", 1_764_000_000).unwrap();
        k.set_last_run("otr", "delta", 1_764_086_400).unwrap();
        assert!(k.set_last_run("otr", "halb", 1).is_err(), "only voll and delta");
    }
    let k = Katalog::open(&p).unwrap();
    assert_eq!(k.last_run("otr", "voll").unwrap(), Some(1_764_000_000));
    assert_eq!(k.last_run("otr", "delta").unwrap(), Some(1_764_086_400));
    assert_eq!(k.last_run("librivox", "voll").unwrap(), None);
}

// ---------------------------------------------------------------- the service reads without any write right

#[test]
fn a_clean_close_leaves_no_side_files() {
    use std::os::unix::fs::PermissionsExt;
    let d = std::env::temp_dir().join(format!("maer-sync-katalog-clean-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("katalog.db");
    let mut k = Katalog::open(p.to_str().unwrap()).unwrap();
    k.write_run("otr", &rows("otr", 3), false).unwrap();
    k.close_clean().unwrap();
    let names: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["katalog.db".to_string()], "side files after a clean close");
    let mode: String = Connection::open(&p).unwrap().query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode.to_lowercase(), "delete");
    // A reader in a folder it cannot write opens it.
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o555)).unwrap();
    let r = Katalog::open_read(p.to_str().unwrap()).map(|r| r.full("otr", None, MAX_PAGE).map(|pg| pg.zeilen.len()));
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(r.unwrap().unwrap(), 3);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_writer_runs_in_wal() {
    let p = scratch("writer-wal");
    { let mut k = Katalog::open(&p).unwrap(); k.write_run("otr", &rows("otr", 1), false).unwrap(); k.close_clean().unwrap(); }
    let mode = |p: &str| -> String { Connection::open(p).unwrap().query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap() };
    assert_eq!(mode(&p).to_lowercase(), "delete");
    let _k = Katalog::open(&p).unwrap();
    assert_eq!(mode(&p).to_lowercase(), "wal", "the writer did not switch back to WAL");
}
#[test]
fn reading_an_empty_file_says_there_is_no_catalog() {
    let p = scratch("leer-lesen");
    std::fs::write(&p, b"").unwrap();
    match Katalog::open_read(&p) {
        Err(KatalogError::NoCatalog) => {}
        Err(e) => panic!("wrong error: {e:?}"),
        Ok(_) => panic!("an empty file was read as a catalog"),
    }
    assert_eq!(std::fs::metadata(&p).unwrap().len(), 0, "reading wrote to the file");
}

#[test]
fn reading_a_file_without_tables_says_there_is_no_catalog() {
    let p = scratch("ohne-tabellen");
    rusqlite::Connection::open(&p).unwrap().execute_batch("PRAGMA user_version = 1;").unwrap();
    match Katalog::open_read(&p) {
        Err(KatalogError::NoCatalog) => {}
        Err(e) => panic!("wrong error: {e:?}"),
        Ok(_) => panic!("a file without tables was read as a catalog"),
    }
}

// ---------------------------------------------------------------- the popularity step

fn mit_stufe(mut r: Row, s: Option<i64>) -> Row {
    r.beliebt_stufe = s;
    r
}

#[test]
fn a_row_keeps_its_popularity_step() {
    let p = scratch("stufe");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &[mit_stufe(row("librivox", "lva:a", "A"), Some(3)), row("librivox", "lva:b", "B")], false).unwrap();
    assert_eq!(k.stored("librivox", "lva:a").unwrap().unwrap().row.beliebt_stufe, Some(3));
    assert_eq!(k.stored("librivox", "lva:b").unwrap().unwrap().row.beliebt_stufe, None);
    let page = k.full("librivox", None, MAX_PAGE).unwrap();
    assert_eq!(page.zeilen.iter().map(|s| s.row.beliebt_stufe).collect::<Vec<_>>(), vec![Some(3), None]);
}

#[test]
fn a_step_outside_one_to_ten_refuses_the_run() {
    let p = scratch("stufe-grenze");
    let mut k = Katalog::open(&p).unwrap();
    for s in [0, 11, -1] {
        match k.write_run("librivox", &[mit_stufe(row("librivox", "lva:a", "A"), Some(s))], false) {
            Err(KatalogError::Invalid(_)) => {}
            other => panic!("step {s}: {other:?}"),
        }
    }
    assert_eq!(k.version("librivox").unwrap(), 0);
    for s in [1, 10] {
        k.write_run("librivox", &[mit_stufe(row("librivox", "lva:a", "A"), Some(s))], false).unwrap();
    }
}

#[test]
fn only_a_new_step_raises_the_version() {
    let p = scratch("stufe-version");
    let mut k = Katalog::open(&p).unwrap();
    let lauf = |a: i64, b: i64| vec![mit_stufe(row("librivox", "lva:a", "A"), Some(a)), mit_stufe(row("librivox", "lva:b", "B"), Some(b))];
    let v1 = k.write_run("librivox", &lauf(3, 5), false).unwrap();
    assert_eq!(k.write_run("librivox", &lauf(3, 5), false).unwrap(), v1, "the same steps raised the version");
    let v2 = k.write_run("librivox", &lauf(4, 5), false).unwrap();
    assert!(v2 > v1);
    assert_eq!(k.stored("librivox", "lva:a").unwrap().unwrap().version, v2);
    assert_eq!(k.stored("librivox", "lva:b").unwrap().unwrap().version, v1, "a row whose step stayed took a new version");
}

#[test]
fn a_delta_never_sets_a_step() {
    // A delta sees only some rows, so it cannot know a tenth: the stored
    // step stays, and a row that first comes through a delta has none.
    let p = scratch("stufe-delta");
    let mut k = Katalog::open(&p).unwrap();
    k.write_run("librivox", &[mit_stufe(row("librivox", "lva:a", "A"), Some(2))], false).unwrap();
    let mut a = mit_stufe(row("librivox", "lva:a", "A changed"), None);
    a.jahr = Some(1900);
    k.write_delta("librivox", &[a, mit_stufe(row("librivox", "lva:c", "C"), Some(1))]).unwrap();
    let a = k.stored("librivox", "lva:a").unwrap().unwrap().row;
    assert_eq!((a.titel.as_deref(), a.beliebt_stufe), (Some("A changed"), Some(2)));
    assert_eq!(k.stored("librivox", "lva:c").unwrap().unwrap().row.beliebt_stufe, None);
    let v = k.version("librivox").unwrap();
    // The same row once more, only with another step: no change.
    let mut again = a.clone();
    again.beliebt_stufe = Some(9);
    assert_eq!(k.write_delta("librivox", &[again]).unwrap(), v, "a step in a delta changed the row");
    assert_eq!(k.stored("librivox", "lva:a").unwrap().unwrap().row.beliebt_stufe, Some(2));
}

/// The table of the catalog as it stood before the popularity step: a probe
/// of the old form, kept here on purpose and never changed.
const SCHEMA_VOR_DER_STUFE: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE stand (quelle TEXT PRIMARY KEY, version INTEGER NOT NULL, voll_ab INTEGER NOT NULL);
CREATE TABLE eintrag (
  quelle TEXT NOT NULL, id TEXT NOT NULL, art TEXT NOT NULL, titel TEXT, urheber TEXT, sprache TEXT,
  jahr INTEGER, rechte_kennung TEXT NOT NULL, rechte_url TEXT, laden INTEGER NOT NULL,
  reihen TEXT NOT NULL DEFAULT '[]', auch TEXT NOT NULL DEFAULT '[]', tief TEXT,
  version INTEGER NOT NULL, geloescht INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (quelle, id)
) WITHOUT ROWID;
CREATE INDEX eintrag_delta ON eintrag (quelle, version);
CREATE TABLE cover (sha256 TEXT PRIMARY KEY, breite INTEGER, hoehe INTEGER, bytes INTEGER, typ TEXT);
INSERT INTO meta VALUES ('katalog_format', '1');
INSERT INTO stand VALUES ('librivox', 4, 0);
INSERT INTO eintrag (quelle, id, art, titel, rechte_kennung, laden, version)
  VALUES ('librivox', 'lva:alt', 'hoerbuch', 'Old', 'pd', 1, 4);
";

#[test]
fn a_catalog_of_the_form_before_the_step_is_read_and_kept() {
    let p = scratch("vor-der-stufe");
    Connection::open(&p).unwrap().execute_batch(SCHEMA_VOR_DER_STUFE).unwrap();
    // The reader (the service) cannot change the file and reads it as it is.
    {
        let r = Katalog::open_read(&p).expect("the reader opens a catalog of the old form");
        let page = r.full("librivox", None, MAX_PAGE).unwrap();
        assert_eq!(page.version, 4);
        assert_eq!(page.zeilen.len(), 1);
        assert_eq!(page.zeilen[0].row.beliebt_stufe, None);
    }
    // The writer (the fetch program) takes it over: rows and versions stay,
    // and the next run writes steps.
    let mut k = Katalog::open(&p).expect("the writer opens a catalog of the old form");
    let alt = k.stored("librivox", "lva:alt").unwrap().unwrap();
    assert_eq!((alt.version, alt.row.titel.as_deref()), (4, Some("Old")));
    assert_eq!(k.version("librivox").unwrap(), 4);
    k.write_run("librivox", &[mit_stufe(row("librivox", "lva:alt", "Old"), Some(1))], false).unwrap();
    assert_eq!(k.stored("librivox", "lva:alt").unwrap().unwrap().row.beliebt_stufe, Some(1));
}

// ---------------------------------------------------------------- a foreign file

/// A SQLite file with tables of its own, none of them a catalog's.
fn fremde_datei(name: &str) -> String {
    let p = scratch(name);
    let c = Connection::open(&p).unwrap();
    c.execute_batch("CREATE TABLE fremd (x TEXT); INSERT INTO fremd VALUES ('eigen');").unwrap();
    p
}

#[test]
fn the_reader_refuses_a_file_with_foreign_tables_and_leaves_it() {
    let p = fremde_datei("fremd-lesen");
    let before = tables(&p);
    match Katalog::open_read(&p) {
        Err(KatalogError::NoCatalog) => panic!("a foreign file was taken as one without a catalog"),
        Err(_) => {}
        Ok(_) => panic!("a foreign file was read as a catalog"),
    }
    assert_eq!(tables(&p), before, "the reader changed the foreign file");
}

#[test]
fn the_writer_refuses_a_file_with_foreign_tables_and_leaves_it() {
    let p = fremde_datei("fremd-schreiben");
    let before = tables(&p);
    match Katalog::open(&p) {
        Err(KatalogError::NoCatalog) => panic!("a foreign file was taken as one without a catalog"),
        Err(_) => {}
        Ok(_) => panic!("the writer laid its schema into a foreign file"),
    }
    assert_eq!(tables(&p), before, "the writer changed the foreign file");
}
