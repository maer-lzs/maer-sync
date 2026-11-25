// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for archive.org answers (K3b-1). Signatures only: the witnesses in
//! `tests/vektoren_archive.rs` are written first and must fail on behaviour.

use maer_sync::katalog::Row;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelf {
    pub schluessel: String,
    pub schlagwort: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Not an answer of the scrape or the advanced search.
    Unreadable(String),
    /// The answer holds records of another query.
    ForeignAnswer(String),
    /// Shellac is read only from a shelf.
    NoShelf,
    UnknownSource(String),
}

/// The collection behind a source of the index.
pub fn collection_of(_quelle: &str) -> Option<&'static str> { todo!("K3b-1") }

/// The mark of a licence URL.
pub fn licence_mark(_url: Option<&str>) -> String { todo!("K3b-1") }

/// The rows of one answer. `calendar_year` decides the shellac year bound;
/// `shelf` is required for shellac and forbidden for every other source.
pub fn rows_from_answer(_quelle: &str, _answer: &[u8], _calendar_year: i32, _shelf: Option<&Shelf>)
    -> Result<Vec<Row>, ReadError> { todo!("K3b-1") }

/// One row per record over several shelf answers, with every shelf it stands on.
pub fn merge_shelves(_parts: Vec<Vec<Row>>) -> Vec<Row> { todo!("K3b-1") }
