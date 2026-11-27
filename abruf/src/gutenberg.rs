// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Project Gutenberg: the catalog dump `pg_catalog.csv` read as the app
//! reads it (`CatalogStockFetch.gutenbergEntries`, `GutenbergMusic`). Only
//! works of type `Sound` become rows. A work with the class `M` is music;
//! music is art music by the app's two lists of headings or a composer who
//! died before 1900. The kind stays `hoerbuch`; where a row stands the app
//! decides from `tief.musik` and `tief.kunstmusik`.

use maer_sync::katalog::Row;

use crate::archive::ReadError;

/// The composer and the performers of a work, from its `Authors` cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    pub composer: Option<String>,
    pub performer: Option<String>,
}

/// Every work of type `Sound` of the dump as a row.
pub fn rows_from_catalog(_csv: &[u8]) -> Result<Vec<Row>, ReadError> {
    Ok(vec![])
}

/// A class of the Library of Congress exactly `M`.
pub fn is_music(_locc: Option<&str>) -> bool {
    false
}

/// Art music: a heading of the art list, or a composer who died before 1900,
/// and no heading of the light list.
pub fn is_art_music(_subjects: &[String], _authors: Option<&str>) -> bool {
    false
}

/// The composer leads; without one the first performer does.
pub fn parts(_authors: Option<&str>) -> Parts {
    Parts { composer: None, performer: None }
}
