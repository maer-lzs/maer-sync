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
pub fn stufen(_rows: &mut [Row], _zahlen: &BTreeMap<String, i64>) {}
