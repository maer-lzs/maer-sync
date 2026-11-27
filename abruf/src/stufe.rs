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
    // The most popular first; a tie goes by id, so the order is the same
    // on every run.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    let n = ranked.len() as i64;
    let stufe: BTreeMap<String, i64> = ranked.iter().enumerate()
        .map(|(rang, (_, id))| (id.to_string(), 1 + 10 * rang as i64 / n))
        .collect();
    for r in rows.iter_mut() {
        r.beliebt_stufe = stufe.get(&r.id).copied();
    }
}
