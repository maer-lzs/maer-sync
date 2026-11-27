// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The popularity step of a row: the tenth of its source by the number the
//! source states (archive.org `downloads`, Radio Browser `votes`), 1 the
//! most popular. Only a full pass sees every row, so only a full pass sets
//! it; the raw number never goes into the index, so a row changes only when
//! its tenth does.

use std::collections::BTreeMap;

use maer_sync::katalog::Row;

/// Set the step of every row that has a number in `zahlen` (by row id);
/// the others get none.
pub fn stufen(rows: &mut [Row], zahlen: &BTreeMap<String, i64>) {
    let mut ranked: Vec<(i64, &str)> = rows.iter()
        .filter_map(|r| zahlen.get(&r.id).map(|z| (*z, r.id.as_str())))
        .collect();
    // The most popular first. The rank of a row is the number of rows with
    // a greater number, so the same number has the same rank and the same
    // step, whatever the ids.
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    let n = ranked.len() as i64;
    let mut stufe: BTreeMap<String, i64> = BTreeMap::new();
    let mut rang = 0;
    for (i, (zahl, id)) in ranked.iter().enumerate() {
        if i == 0 || *zahl != ranked[i - 1].0 {
            rang = i as i64;
        }
        stufe.insert(id.to_string(), 1 + 10 * rang / n);
    }
    for r in rows.iter_mut() {
        r.beliebt_stufe = stufe.get(&r.id).copied();
    }
}
