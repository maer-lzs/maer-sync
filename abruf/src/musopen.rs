// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for Musopen on Wikimedia Commons.
//!
//! Rights as the app (iOS `MusopenCatalog.Licence`): the
//! `License` field of the file page decides; `pd` and `cc0` give a row, an
//! empty field with the short name exactly "PDM-owner" gives `pdm`, every
//! other licence gives none. The short name alone decides nothing. A file
//! whose address is not on `upload.wikimedia.org` gives no row either: that
//! address is what the device plays later.
//!
//! `satz` follows `MusopenOrder.movementNumber` (iOS `8319e1c0`). Composer,
//! work and performer are a rule of the app with its own lists; they come
//! into `tief` once the Swift side has delivered its values for the shared
//! vectors, not before.

use maer_sync::katalog::Row;
use serde_json::Value;

pub const KATEGORIEN: [&str; 2] = ["Category:Musopen", "Category:Set Chopin Free"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusopenFehler {
    Unreadable(String),
    /// Commons asks to come back later (`maxlag`); the lag in seconds.
    Maxlag(u64),
}

/// One answer: its rows, how many pages it carried (media or not), and the
/// token of the next page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seite {
    pub zeilen: Vec<Row>,
    pub gesehen: usize,
    pub weiter: Option<String>,
}

pub fn licence(stated: Option<&str>, short: Option<&str>) -> Option<&'static str> {
    match stated.unwrap_or("").trim().to_lowercase().as_str() {
        "pd" => Some("pd"),
        "cc0" => Some("cc0"),
        "" if short.map(str::trim) == Some("PDM-owner") => Some("pdm"),
        _ => None,
    }
}

/// The number after the last " - ": Roman in capitals or Arabic, followed
/// by ". " or ") "; else none.
pub fn movement_number(title: &str) -> Option<i64> {
    let rest = &title[title.rfind(" - ")? + 3..];
    let roman: String = rest.chars().take_while(|c| "IVXLC".contains(*c)).collect();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let (number, len) = if !roman.is_empty() { (None, roman.len()) } else if !digits.is_empty() { (digits.parse().ok(), digits.len()) } else { return None };
    let after = &rest[len..];
    if !(after.starts_with(". ") || after.starts_with(") ")) {
        return None;
    }
    if number.is_some() {
        return number;
    }
    let v: Vec<i64> = roman.chars().map(|c| match c { 'I' => 1, 'V' => 5, 'X' => 10, 'L' => 50, _ => 100 }).collect();
    Some((0..v.len()).map(|i| if i + 1 < v.len() && v[i] < v[i + 1] { -v[i] } else { v[i] }).sum())
}

/// A file title without "File:" and without its audio ending.
pub fn titel_of(file: &str) -> String {
    let t = file.strip_prefix("File:").unwrap_or(file);
    for ext in [".ogg", ".flac", ".webm", ".mp3", ".oga", ".wav"] {
        if t.len() > ext.len() && t[t.len() - ext.len()..].eq_ignore_ascii_case(ext) {
            return t[..t.len() - ext.len()].to_string();
        }
    }
    t.to_string()
}

fn text<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    v.pointer(path).and_then(Value::as_str)
}

pub fn rows_from_answer(answer: &[u8]) -> Result<Seite, MusopenFehler> {
    let v: Value = serde_json::from_slice(answer).map_err(|e| MusopenFehler::Unreadable(e.to_string()))?;
    if text(&v, "/error/code") == Some("maxlag") {
        return Err(MusopenFehler::Maxlag(v.pointer("/error/lag").and_then(Value::as_u64).unwrap_or(5)));
    }
    let pages = v.pointer("/query/pages").and_then(Value::as_array)
        .ok_or_else(|| MusopenFehler::Unreadable("no query.pages".into()))?;
    let mut zeilen = vec![];
    for p in pages {
        let id = p.get("pageid").and_then(Value::as_i64).ok_or_else(|| MusopenFehler::Unreadable("a page without pageid".into()))?;
        let Some(vi) = p.pointer("/videoinfo/0") else { continue };
        let url = vi.get("url").and_then(Value::as_str).unwrap_or("");
        let Some(mark) = licence(text(vi, "/extmetadata/License/value"), text(vi, "/extmetadata/LicenseShortName/value")) else { continue };
        if !url.starts_with("https://upload.wikimedia.org/") {
            continue;
        }
        let titel = titel_of(p.get("title").and_then(Value::as_str).unwrap_or(""));
        let tief = movement_number(&titel).map(|s| serde_json::json!({ "satz": s }).to_string());
        zeilen.push(Row {
            quelle: "musopen".into(), id: format!("mso:{id}"), art: "musik".into(), titel: Some(titel),
            urheber: None, sprache: None, jahr: None, rechte_kennung: mark.into(),
            rechte_url: text(vi, "/extmetadata/LicenseUrl/value").map(str::to_string),
            laden: true, reihen: vec![], auch: vec![], tief,
        });
    }
    Ok(Seite { zeilen, gesehen: pages.len(), weiter: text(&v, "/continue/gcmcontinue").map(str::to_string) })
}

/// Why a file of the category gives no row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    NotAudio,
    Licence,
    NoPlayableForm,
    ForeignHost,
}

pub fn playable(_videoinfo: &Value) -> Result<String, Dropped> {
    unimplemented!("written with the witnesses green")
}

pub fn work_stem(_title: &str) -> Option<String> {
    unimplemented!("written with the witnesses green")
}

pub fn composer(_author: Option<&str>, _title: &str) -> Option<String> {
    unimplemented!("written with the witnesses green")
}

pub fn artist_text(_html: Option<&str>) -> Option<String> {
    unimplemented!("written with the witnesses green")
}

pub fn shown_title(_object_name: Option<&str>, _file_title: &str) -> String {
    unimplemented!("written with the witnesses green")
}
