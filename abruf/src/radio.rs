// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for Radio Browser: a page of `/json/stations` (asked with
//! `hidebroken=true`, as the app does) to catalog rows.
//!
//! As the app (`RadioMapping.stations`): name and address lose spaces and
//! tabs at their ends; the address played is `url_resolved`, else `url`; an
//! entry with an empty name or address, or an address Foundation cannot
//! read, is dropped, and of a uuid twice in
//! one page the first usable entry wins. The row's id is the app's (`radio:rb:<stationuuid>`), a
//! station is never loaded, and `tief` carries what the app reads to play
//! and to order (empty strings as null). The station's own address stays in
//! `tief` as it is, plain http included: the index never asks it.

use std::collections::BTreeSet;

use maer_sync::katalog::Row;
use serde_json::{Map, Value};

use crate::archive::ReadError;

/// The servers of the app, in this order; a run falls back to the next.
pub const SERVER: [&str; 3] = [
    "https://de1.api.radio-browser.info",
    "https://nl1.api.radio-browser.info",
    "https://at1.api.radio-browser.info",
];

const TIEF: [&str; 12] = [
    "url", "url_resolved", "favicon", "homepage", "countrycode", "tags", "bitrate", "codec", "votes", "languagecodes", "state", "clickcount",
];

/// One page: its rows, its raw length (for paging), and its uuids once each
/// (for the count), dropped entries included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seite {
    pub zeilen: Vec<Row>,
    pub roh: usize,
    pub uuids: Vec<String>,
}

/// Whether Foundation's `URL(string:)` gives no address: on the systems the
/// app runs on, only for an empty string and for a host that opens a bracket
/// without closing it; everything else it encodes into an address.
fn refused_by_url_string(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let Some(at) = s.find("://") else { return false };
    let rest = &s[at + 3..];
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    host.starts_with('[') && !host.contains(']')
}

/// Swift's `.whitespaces`: spaces and tabs, not line breaks.
fn trimmed(v: Option<&Value>) -> &str {
    v.and_then(Value::as_str).unwrap_or("").trim_matches(|c: char| c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

pub fn rows_from_page(answer: &[u8]) -> Result<Seite, ReadError> {
    let v: Value = serde_json::from_slice(answer).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    let list = v.as_array().ok_or_else(|| ReadError::Unreadable("a page is not a list".into()))?;
    let mut seen = BTreeSet::new();
    let mut taken = BTreeSet::new();
    let mut uuids = vec![];
    let mut zeilen = vec![];
    for x in list {
        let uuid = x.get("stationuuid").and_then(Value::as_str).filter(|s| !s.is_empty())
            .ok_or_else(|| ReadError::Unreadable("an entry without stationuuid".into()))?;
        if seen.insert(uuid.to_string()) {
            uuids.push(uuid.to_string());
        }
        let name = trimmed(x.get("name"));
        let resolved = trimmed(x.get("url_resolved"));
        let stream = if resolved.is_empty() { trimmed(x.get("url")) } else { resolved };
        if name.is_empty() || stream.is_empty() || refused_by_url_string(stream) || !taken.insert(uuid.to_string()) {
            continue;
        }
        let codes: Vec<&str> = x.get("languagecodes").and_then(Value::as_str).unwrap_or("")
            .split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        let mut tief = Map::new();
        for k in TIEF {
            let mut val = x.get(k).cloned().unwrap_or(Value::Null);
            if k == "url" || k == "url_resolved" {
                val = Value::String(trimmed(Some(&val)).to_string());
            }
            tief.insert(k.into(), if val.as_str().is_some_and(|s| s.trim().is_empty()) { Value::Null } else { val });
        }
        zeilen.push(Row {
            quelle: "radio".into(), id: format!("radio:rb:{uuid}"), art: "sender".into(),
            titel: Some(name.to_string()), urheber: None, sprache: codes.first().map(|s| s.to_string()),
            jahr: None, rechte_kennung: "unbekannt".into(), rechte_url: None, laden: false,
            reihen: vec![], auch: vec![], tief: Some(Value::Object(tief).to_string()),
        });
    }
    Ok(Seite { zeilen, roh: list.len(), uuids })
}
