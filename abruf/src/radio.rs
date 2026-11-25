// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for Radio Browser (K3e). Signatures only.

use maer_sync::katalog::Row;
use crate::archive::ReadError;

pub const SERVER: [&str; 3] = ["https://de1.api.radio-browser.info", "https://nl1.api.radio-browser.info", "https://at1.api.radio-browser.info"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seite { pub zeilen: Vec<Row>, pub roh: usize, pub uuids: Vec<String> }

pub fn rows_from_page(_answer: &[u8]) -> Result<Seite, ReadError> { todo!("K3e") }
