// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The network side of the fetch program (K3c).
//!
//! A `Holer` makes exactly one request and follows nothing; everything a
//! source may do to the process is decided here, in front of it:
//! - every request names the client (`USER_AGENT`);
//! - only https, and a redirect never leads from https to http; at most
//!   `MAX_REDIRECTS` are followed;
//! - an answer larger than `MAX_BYTES` is an error, and the `Holer` is told
//!   the bound so it stops reading there (a foreign server must not be able
//!   to lay the process down through its memory); every request has a
//!   time limit;
//! - two requests to the same host lie at least `MIN_GAP_MS` apart;
//! - 429 and 503 with `Retry-After` become `SpaeterWieder`, so the plan
//!   does not ask again before.
//!
//! The clock is a `Uhr`, so the witnesses run without sleeping.

use std::collections::BTreeMap;

pub const USER_AGENT: &str = "Maer/1.0 (+https://maer.fm)";
/// The largest answer read (a scrape page of 10 000 records measured 6.4 MB).
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
/// The time limit of one request (a page measured up to 17.6 s).
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

/// One request, no redirect followed, reading at most `max_bytes + 1`
/// bytes within `timeout_s`.
pub trait Holer {
    fn hole(&mut self, anfrage: &Anfrage) -> Result<Antwort, HolFehler>;
}

pub trait Uhr {
    fn jetzt_ms(&self) -> i64;
    fn schlafe_ms(&mut self, ms: i64);
}

/// The clock of the program: wall time in milliseconds since 1970, and a
/// real sleep. A negative wait sleeps not at all.
pub struct SystemUhr;

impl Uhr for SystemUhr {
    fn jetzt_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
    fn schlafe_ms(&mut self, ms: i64) {
        if ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(ms as u64));
        }
    }
}

pub struct Netz<H: Holer, U: Uhr> {
    pub holer: H,
    pub uhr: U,
    last: BTreeMap<String, i64>,
}

fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split(['/', '?']).next().unwrap_or(rest)
}

fn join(base: &str, location: &str) -> String {
    if location.contains("://") {
        return location.to_string();
    }
    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let path = base[scheme_end..].find(['/', '?']).map(|i| scheme_end + i).unwrap_or(base.len());
    let origin = &base[..path];
    if location.starts_with('/') {
        return format!("{origin}{location}");
    }
    // Relative to the directory of the current path.
    let current = base[path..].split('?').next().unwrap_or("");
    let dir = current.rfind('/').map(|i| &current[..=i]).unwrap_or("/");
    format!("{origin}{dir}{location}")
}

impl<H: Holer, U: Uhr> Netz<H, U> {
    pub fn new(holer: H, uhr: U) -> Self {
        Netz { holer, uhr, last: BTreeMap::new() }
    }

    /// GET an https address within the bounds above.
    pub fn get(&mut self, url: &str) -> Result<Antwort, HolFehler> {
        let mut url = url.to_string();
        if !url.starts_with("https://") {
            return Err(HolFehler::NichtHttps(url));
        }
        let mut redirects = 0;
        loop {
            let host = host_of(&url).to_string();
            if let Some(&t) = self.last.get(&host) {
                let wait = t + MIN_GAP_MS - self.uhr.jetzt_ms();
                if wait > 0 {
                    self.uhr.schlafe_ms(wait);
                }
            }
            self.last.insert(host, self.uhr.jetzt_ms());
            let anfrage = Anfrage {
                url: url.clone(),
                user_agent: USER_AGENT.into(),
                max_bytes: MAX_BYTES,
                timeout_s: TIMEOUT_S,
            };
            let a = self.holer.hole(&anfrage)?;
            if a.koerper.len() > MAX_BYTES {
                return Err(HolFehler::ZuGross(a.koerper.len()));
            }
            match a.status {
                200..=299 | 304 => return Ok(a),
                301 | 302 | 303 | 307 | 308 => {
                    let Some(loc) = a.location.as_deref() else { return Err(HolFehler::Status(a.status)) };
                    let next = join(&url, loc);
                    if !next.starts_with("https://") {
                        return Err(HolFehler::UnsichereWeiterleitung(next));
                    }
                    redirects += 1;
                    if redirects > MAX_REDIRECTS {
                        return Err(HolFehler::ZuVieleWeiterleitungen);
                    }
                    url = next;
                }
                429 | 503 if a.retry_after_s.is_some() => {
                    return Err(HolFehler::SpaeterWieder(a.retry_after_s.unwrap_or(0)));
                }
                s => return Err(HolFehler::Status(s)),
            }
        }
    }
}

/// Percent-encode a query value: everything but the unreserved characters.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The query parameters of an address, decoded ('+' as space).
pub fn decode_params(url: &str) -> Vec<(String, String)> {
    let Some((_, q)) = url.split_once('?') else { return vec![] };
    let dec = |s: &str| {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'+' => { out.push(b' '); i += 1; }
                b'%' if i + 2 < b.len() => {
                    match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                        Some(v) => { out.push(v); i += 3; }
                        None => { out.push(b'%'); i += 1; }
                    }
                }
                c => { out.push(c); i += 1; }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    };
    q.split('&').filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') { Some((k, v)) => (dec(k), dec(v)), None => (dec(p), String::new()) })
        .collect()
}
