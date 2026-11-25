// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The fetch program as a program. Signatures only until its witnesses are red.

use crate::lauf::Laufbericht;
use crate::netz::{Holer, Netz, Uhr};

/// The sources that have a reader today.
pub const QUELLEN: [&str; 5] = ["archive-buch", "librivox", "netlabel", "otr", "schellack"];

pub struct Programm<H: Holer, U: Uhr> {
    pub netz: Netz<H, U>,
}

impl<H: Holer, U: Uhr> Programm<H, U> {
    pub fn new(_netz: Netz<H, U>, _path: &str) -> Result<Self, String> { todo!("programm") }
    pub fn schritt(&mut self) -> (Option<Laufbericht>, i64) { todo!("programm") }
    pub fn kalenderjahr(&self) -> i32 { todo!("programm") }
}
