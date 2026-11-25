// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! One run of one source (K3c). Signatures only.

use maer_sync::katalog::Katalog;

use crate::archive::{ReadError, Shelf};
use crate::netz::{HolFehler, Holer, Netz, Uhr};
use crate::plan::Art;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Einstellungen {
    pub scrape_count: usize,
    pub adv_rows: usize,
    pub kalenderjahr: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaufFehler {
    Holen(HolFehler),
    Lesen(ReadError),
    Zaehlung { erwartet: usize, gelesen: usize, eindeutig: usize },
    Echo { gestellt: String, zurueck: Option<String> },
    Katalog(String),
    Quelle(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laufbericht {
    pub quelle: String,
    pub art: Art,
    pub gelesen: usize,
    pub version_vorher: i64,
    pub version_nachher: i64,
    pub dauer_ms: i64,
    pub ausgang: Result<(), LaufFehler>,
}

impl Laufbericht {
    pub fn zeile(&self) -> String { todo!("K3c") }
}

pub fn base_query(_quelle: &str) -> Option<&'static str> { todo!("K3c") }
pub fn regale_der_app() -> Vec<Shelf> { todo!("K3c") }
pub fn datum(_unix_s: i64) -> String { todo!("K3c") }

pub fn lauf<H: Holer, U: Uhr>(_netz: &mut Netz<H, U>, _katalog: &mut Katalog, _quelle: &str, _art: Art,
                              _e: &Einstellungen) -> Laufbericht { todo!("K3c") }
