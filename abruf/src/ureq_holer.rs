// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The real `Holer` over `ureq` (K3c). Signatures only: the witnesses in
//! `tests/holer.rs` are written first.

use crate::netz::{Anfrage, Antwort, HolFehler, Holer};

pub struct UreqHoler;

impl UreqHoler {
    pub fn new() -> Self { todo!("K3c ureq") }
    pub fn plain_http_for_witnesses() -> Self { todo!("K3c ureq") }
    pub fn verifies_certificates(&self) -> bool { todo!("K3c ureq") }
    pub fn https_only(&self) -> bool { todo!("K3c ureq") }
}

impl Default for UreqHoler {
    fn default() -> Self { Self::new() }
}

impl Holer for UreqHoler {
    fn hole(&mut self, _a: &Anfrage) -> Result<Antwort, HolFehler> { todo!("K3c ureq") }
}
