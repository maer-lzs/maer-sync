// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for Radio Browser (K3e): a page of `/json/stations` (asked with
//! `hidebroken=true`, as the app does) to catalog rows.
//!
//! As the app (`RadioMapping.stations`): an entry with an empty name or an
//! empty address is dropped, and a uuid twice in one page counts once (the
//! first wins). The row's id is the app's (`radio:rb:<stationuuid>`), a
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

fn blank(v: &Value) -> bool {
    v.as_str().map(|s| s.trim().is_empty()).unwrap_or(true)
}

pub fn rows_from_page(answer: &[u8]) -> Result<Seite, ReadError> {
    let v: Value = serde_json::from_slice(answer).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    let list = v.as_array().ok_or_else(|| ReadError::Unreadable("a page is not a list".into()))?;
    let mut seen = BTreeSet::new();
    let mut uuids = vec![];
    let mut zeilen = vec![];
    for x in list {
        let uuid = x.get("stationuuid").and_then(Value::as_str).filter(|s| !s.is_empty())
            .ok_or_else(|| ReadError::Unreadable("an entry without stationuuid".into()))?;
        if !seen.insert(uuid.to_string()) {
            continue;
        }
        uuids.push(uuid.to_string());
        let name = x.get("name").cloned().unwrap_or(Value::Null);
        if blank(&name) || blank(x.get("url").unwrap_or(&Value::Null)) {
            continue;
        }
        let codes: Vec<&str> = x.get("languagecodes").and_then(Value::as_str).unwrap_or("")
            .split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        let mut tief = Map::new();
        for k in TIEF {
            let val = x.get(k).cloned().unwrap_or(Value::Null);
            tief.insert(k.into(), if val.as_str().is_some_and(|s| s.trim().is_empty()) { Value::Null } else { val });
        }
        zeilen.push(Row {
            quelle: "radio".into(), id: format!("radio:rb:{uuid}"), art: "sender".into(),
            titel: name.as_str().map(str::to_string), urheber: None, sprache: codes.first().map(|s| s.to_string()),
            jahr: None, rechte_kennung: "unbekannt".into(), rechte_url: None, laden: false,
            reihen: vec![], auch: vec![], tief: Some(Value::Object(tief).to_string()),
        });
    }
    Ok(Seite { zeilen, roh: list.len(), uuids })
}
