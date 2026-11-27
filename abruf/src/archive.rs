// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for archive.org answers.
//!
//! It reads the answer of the scrape API (`items`) and of the advanced
//! search (`response.docs`) alike and turns each record into a catalog row
//! with the id the app uses (`lva:<identifier>`).
//!
//! **An answer must prove it belongs to its query.** On 25.11.2026 the
//! scrape API answered a query with the cached answer of an earlier query
//! that had the same `fields` and `count`, without any error. So every
//! record has to name the collection of its source, and on a shellac shelf
//! the subject of that shelf; one record that does not is a foreign answer,
//! and the whole answer is refused rather than filtered.
//!
//! **Malformed is not empty.** An answer without its list of records is an
//! error; only a well-formed empty list reads as no rows, because the
//! catalog treats an empty run as "the source removed everything".
//!
//! **Shellac is never loaded.** Its collection declares no licence; a year
//! up to the calendar year minus 101 gives the mark `pd` as a statement, but
//! loading needs a declared licence.
//!
//! **Shellac only from a shelf**: the collection is
//! never read whole. A shellac answer is read only together with its shelf,
//! and its rows carry that shelf in `reihen`; no other source has rows in
//! `reihen` before the rotation comes.

use std::collections::BTreeMap;

use maer_sync::katalog::Row;
use serde_json::Value;

/// A theme shelf of the shellac collection, as `abruf/regale.json` lists it.
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

/// Recordings older than this many years before the calendar year are in the
/// public domain in the USA (sound recordings: 100 years after publication,
/// counted to the end of the year; in 2026 that is 1925 and earlier).
const SHELLAC_YEARS: i32 = 101;

/// The collection behind a source of the index.
pub fn collection_of(quelle: &str) -> Option<&'static str> {
    match quelle {
        "librivox" => Some("librivoxaudio"),
        "archive-buch" => Some("audio_bookspoetry"),
        "netlabel" => Some("netlabels"),
        "schellack" => Some("georgeblood"),
        "otr" => Some("oldtimeradio"),
        _ => None,
    }
}

fn kind_of(quelle: &str) -> &'static str {
    match quelle {
        "netlabel" | "schellack" => "musik",
        "otr" => "hoerspiel",
        _ => "hoerbuch",
    }
}

/// The mark of a licence URL: `pdm`, `cc0`, `pd`, a Creative Commons
/// licence by its parts in a fixed order (`cc-by-nc-sa`), else `unbekannt`.
/// Scheme, case and a trailing slash do not count.
pub fn licence_mark(url: Option<&str>) -> String {
    let u = url.unwrap_or("").trim().to_lowercase();
    if u.is_empty() {
        return "unbekannt".into();
    }
    let u = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(&u);
    let u = u.strip_prefix("www.").unwrap_or(u).to_string();
    let path = format!("{}/", u.trim_end_matches('/'));
    if path.starts_with("creativecommons.org/publicdomain/mark/") {
        return "pdm".into();
    }
    if path.starts_with("creativecommons.org/publicdomain/zero/") {
        return "cc0".into();
    }
    if path.starts_with("creativecommons.org/licenses/publicdomain/") {
        return "pd".into();
    }
    if let Some(rest) = path.strip_prefix("creativecommons.org/licenses/") {
        let name = rest.split('/').next().unwrap_or("");
        let parts: Vec<&str> = name.split('-').collect();
        let known = parts.iter().all(|p| ["by", "nc", "sa", "nd"].contains(p));
        let mut unique = parts.clone();
        unique.sort();
        unique.dedup();
        if parts.first() == Some(&"by") && known && unique.len() == parts.len() {
            let mut mark = String::from("cc-by");
            for p in ["nc", "sa", "nd"] {
                if parts.contains(&p) {
                    mark.push('-');
                    mark.push_str(p);
                }
            }
            return mark;
        }
    }
    "unbekannt".into()
}

/// A text field that may be a list: the values joined with "; ", none for
/// an empty list or a missing field.
fn joined(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Array(a)) => {
            let parts: Vec<String> = a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
            if parts.is_empty() { None } else { Some(parts.join("; ")) }
        }
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn texts(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => vec![],
    }
}

/// The year of a record: `year` as a number, or as text of exactly four
/// digits; without `year`, the `date` when it starts with four digits
/// followed by '-', 'T' or nothing. Anything uncertain ("1925?") is no year.
fn year_of(d: &Value) -> Option<i64> {
    match d.get("year") {
        Some(Value::Number(n)) => return n.as_i64(),
        Some(Value::String(s)) => {
            return if s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()) { s.parse().ok() } else { None };
        }
        _ => {}
    }
    let date = d.get("date")?.as_str()?;
    let head = date.get(..4)?;
    if !head.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match date.as_bytes().get(4) {
        None | Some(b'-') | Some(b'T') => head.parse().ok(),
        _ => None,
    }
}

/// Whether a subject names the shelf's word as a word of its own, ignoring case.
fn names_word(subject: &str, word: &str) -> bool {
    let s = subject.to_lowercase();
    let w = word.to_lowercase();
    s.split(|c: char| !c.is_alphanumeric()).any(|part| part == w)
}

fn records(answer: &Value) -> Result<&Vec<Value>, ReadError> {
    if let Some(items) = answer.get("items") {
        return items.as_array().ok_or_else(|| ReadError::Unreadable("items is not a list".into()));
    }
    if let Some(docs) = answer.get("response").and_then(|r| r.get("docs")) {
        return docs.as_array().ok_or_else(|| ReadError::Unreadable("response.docs is not a list".into()));
    }
    Err(ReadError::Unreadable("neither items nor response.docs".into()))
}

/// The rows of one answer. `calendar_year` decides the shellac year bound;
/// `shelf` is required for shellac and refused for every other source.
pub fn rows_from_answer(quelle: &str, answer: &[u8], calendar_year: i32, shelf: Option<&Shelf>)
    -> Result<Vec<Row>, ReadError> {
    let collection = collection_of(quelle).ok_or_else(|| ReadError::UnknownSource(quelle.into()))?;
    match (quelle, shelf) {
        ("schellack", None) => return Err(ReadError::NoShelf),
        (q, Some(s)) if q != "schellack" => {
            return Err(ReadError::Unreadable(format!("shelf {} given for {q}", s.schluessel)));
        }
        _ => {}
    }
    let parsed: Value = serde_json::from_slice(answer).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    let list = records(&parsed)?;

    // First every record must be readable, then every record must belong.
    let mut ids = Vec::with_capacity(list.len());
    for d in list {
        let id = d.get("identifier").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            return Err(ReadError::Unreadable("a record without identifier".into()));
        }
        ids.push(id.to_string());
    }
    for (d, id) in list.iter().zip(&ids) {
        if !texts(d.get("collection")).iter().any(|c| c == collection) {
            return Err(ReadError::ForeignAnswer(format!("{id} is not in {collection}")));
        }
        if let Some(s) = shelf {
            if !texts(d.get("subject")).iter().any(|x| names_word(x, &s.schlagwort)) {
                return Err(ReadError::ForeignAnswer(format!("{id} is not on the shelf {}", s.schluessel)));
            }
        }
    }

    let mut rows = Vec::with_capacity(list.len());
    for (d, id) in list.iter().zip(ids) {
        let jahr = year_of(d);
        let url = d.get("licenseurl").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string);
        let (kennung, rechte_url) = if quelle == "schellack" {
            // The collection states no licence; the year is the only right.
            let free = jahr.is_some_and(|y| y <= (calendar_year - SHELLAC_YEARS) as i64);
            (if free { "pd".to_string() } else { "unbekannt".to_string() }, None)
        } else {
            (licence_mark(url.as_deref()), url)
        };
        rows.push(Row {
            quelle: quelle.into(),
            id: format!("lva:{id}"),
            art: kind_of(quelle).into(),
            titel: joined(d.get("title")),
            urheber: joined(d.get("creator")),
            sprache: joined(d.get("language")),
            jahr,
            // Loading needs a declared licence; shellac declares none, so its
            // year is a statement only and never opens the download.
            laden: quelle != "schellack" && kennung != "unbekannt",
            rechte_kennung: kennung,
            rechte_url,
            reihen: shelf.map(|s| vec![s.schluessel.clone()]).unwrap_or_default(),
            auch: vec![],
            tief: None, beliebt_stufe: None,
        });
    }
    Ok(rows)
}

/// The number of downloads per row id of an answer; a record without a
/// whole number from 0 is left out. An unreadable answer has none.
pub fn downloads_from_answer(answer: &[u8]) -> BTreeMap<String, i64> {
    let Ok(parsed) = serde_json::from_slice::<Value>(answer) else { return BTreeMap::new() };
    let Ok(list) = records(&parsed) else { return BTreeMap::new() };
    list.iter().filter_map(|d| {
        let id = d.get("identifier").and_then(Value::as_str).filter(|s| !s.is_empty())?;
        let n = d.get("downloads").filter(|v| v.is_i64() || v.is_u64()).and_then(Value::as_i64).filter(|n| *n >= 0)?;
        Some((format!("lva:{id}"), n))
    }).collect()
}

/// One row per record over several shelf answers, with every shelf it
/// stands on (sorted); the rest of the row is taken from its first answer.
/// Rows keep the order in which they first appear.
pub fn merge_shelves(parts: Vec<Vec<Row>>) -> Vec<Row> {
    let mut order: Vec<String> = vec![];
    let mut by_id: BTreeMap<String, Row> = BTreeMap::new();
    for row in parts.into_iter().flatten() {
        match by_id.get_mut(&row.id) {
            Some(seen) => {
                for k in row.reihen {
                    if !seen.reihen.contains(&k) {
                        seen.reihen.push(k);
                    }
                }
                seen.reihen.sort();
            }
            None => {
                order.push(row.id.clone());
                by_id.insert(row.id.clone(), row);
            }
        }
    }
    order.into_iter().map(|id| by_id.remove(&id).expect("every id was inserted")).collect()
}
