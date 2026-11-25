// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The network side of the fetch program (K3c). Signatures only: the
//! witnesses in `tests/ablauf.rs` are written first.

pub const USER_AGENT: &str = "Maer/1.0 (+https://maer.fm)";
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
pub const TIMEOUT_S: u64 = 120;
pub const MAX_REDIRECTS: usize = 3;
pub const MIN_GAP_MS: i64 = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anfrage {
    pub url: String,
    pub user_agent: String,
    pub max_bytes: usize,
    pub timeout_s: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Antwort {
    pub status: u16,
    pub koerper: Vec<u8>,
    pub location: Option<String>,
    pub retry_after_s: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HolFehler {
    Zeit,
    ZuGross(usize),
    Verbindung(String),
    Status(u16),
    NichtHttps(String),
    ZuVieleWeiterleitungen,
    UnsichereWeiterleitung(String),
    SpaeterWieder(u64),
}

pub trait Holer {
    fn hole(&mut self, anfrage: &Anfrage) -> Result<Antwort, HolFehler>;
}

pub trait Uhr {
    fn jetzt_ms(&self) -> i64;
    fn schlafe_ms(&mut self, ms: i64);
}

pub struct Netz<H: Holer, U: Uhr> {
    pub holer: H,
    pub uhr: U,
}

impl<H: Holer, U: Uhr> Netz<H, U> {
    pub fn new(holer: H, uhr: U) -> Self { Netz { holer, uhr } }
    pub fn get(&mut self, _url: &str) -> Result<Antwort, HolFehler> { todo!("K3c") }
}

pub fn encode(_s: &str) -> String { todo!("K3c") }
pub fn decode_params(_url: &str) -> Vec<(String, String)> { todo!("K3c") }
