// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The popularity step: the tenth of a source by its number, 1 the most
//! popular; the same number has the same step. Rows without a number get
//! none and do not count.

use std::collections::BTreeMap;

use maer_sync::katalog::Row;
use maer_sync_abruf::stufe::stufen;

fn row(id: &str) -> Row {
    Row {
        quelle: "librivox".into(), id: id.into(), art: "hoerbuch".into(), titel: None, urheber: None,
        sprache: None, jahr: None, rechte_kennung: "pd".into(), rechte_url: None, laden: true,
        reihen: vec![], auch: vec![], tief: None, beliebt_stufe: Some(7),
    }
}

fn steps(rows: &[Row]) -> BTreeMap<String, Option<i64>> {
    rows.iter().map(|r| (r.id.clone(), r.beliebt_stufe)).collect()
}

#[test]
fn twenty_rows_two_to_a_tenth() {
    let mut rows: Vec<Row> = (0..20).map(|i| row(&format!("lva:{i:02}"))).collect();
    let zahlen: BTreeMap<String, i64> = (0..20).map(|i| (format!("lva:{i:02}"), 1000 - i * 10)).collect();
    stufen(&mut rows, &zahlen);
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r.beliebt_stufe, Some(1 + i as i64 / 2), "{}", r.id);
    }
}

#[test]
fn the_same_number_has_the_same_step() {
    let mut rows = vec![row("lva:b"), row("lva:a")];
    let zahlen = BTreeMap::from([("lva:a".to_string(), 5), ("lva:b".to_string(), 5)]);
    stufen(&mut rows, &zahlen);
    assert_eq!(steps(&rows), BTreeMap::from([("lva:a".into(), Some(1)), ("lva:b".into(), Some(1))]));
}

#[test]
fn a_tie_across_the_border_of_a_tenth_takes_the_upper_step() {
    // Twenty rows, two to a tenth: ranks 0 and 1 are the first tenth, 2 and
    // 3 the second. Ranks 1, 2 and 3 share a number: all three take the
    // step of the first of them, whatever their ids.
    let mut rows: Vec<Row> = (0..20).map(|i| row(&format!("lva:{i:02}"))).collect();
    let mut zahlen: BTreeMap<String, i64> = (0..20).map(|i| (format!("lva:{i:02}"), 1000 - i * 10)).collect();
    for i in [1, 2, 3] {
        zahlen.insert(format!("lva:{i:02}"), 990);
    }
    stufen(&mut rows, &zahlen);
    let s = steps(&rows);
    for i in [0, 1, 2, 3] {
        assert_eq!(s[&format!("lva:{i:02}")], Some(1), "rank {i}");
    }
    assert_eq!(s["lva:04"], Some(3), "the next number keeps its own rank");
}

#[test]
fn rows_without_a_number_get_none_and_do_not_count() {
    let mut rows = vec![row("lva:a"), row("lva:b"), row("lva:c")];
    let zahlen = BTreeMap::from([("lva:a".to_string(), 3), ("lva:b".to_string(), 9)]);
    stufen(&mut rows, &zahlen);
    assert_eq!(steps(&rows), BTreeMap::from([("lva:a".into(), Some(6)), ("lva:b".into(), Some(1)), ("lva:c".into(), None)]));
}

#[test]
fn one_row_is_the_top_tenth() {
    let mut rows = vec![row("lva:a")];
    stufen(&mut rows, &BTreeMap::from([("lva:a".to_string(), 0)]));
    assert_eq!(rows[0].beliebt_stufe, Some(1));
}

#[test]
fn a_number_for_no_row_changes_nothing_and_no_number_clears() {
    let mut rows = vec![row("lva:a")];
    stufen(&mut rows, &BTreeMap::from([("lva:x".to_string(), 4)]));
    assert_eq!(rows[0].beliebt_stufe, None);
}
