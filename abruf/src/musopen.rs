// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for Musopen on Wikimedia Commons (K3e). Signatures only.

use maer_sync::katalog::Row;

pub const KATEGORIEN: [&str; 2] = ["Category:Musopen", "Category:Set Chopin Free"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusopenFehler { Unreadable(String), Maxlag(u64) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seite { pub zeilen: Vec<Row>, pub gesehen: usize, pub weiter: Option<String> }

pub fn licence(_stated: Option<&str>, _short: Option<&str>) -> Option<&'static str> { todo!("K3e") }
pub fn movement_number(_title: &str) -> Option<i64> { todo!("K3e") }
pub fn titel_of(_file: &str) -> String { todo!("K3e") }
pub fn rows_from_answer(_answer: &[u8]) -> Result<Seite, MusopenFehler> { todo!("K3e") }
