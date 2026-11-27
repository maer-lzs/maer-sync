// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Project Gutenberg: the catalog dump `pg_catalog.csv` read as the app
//! reads it (`CatalogStockFetch.gutenbergEntries`, `GutenbergMusic`). Only
//! works of type `Sound` become rows. A work with the class `M` is music;
//! music is art music by the app's two lists of headings or a composer who
//! died before 1900. The kind stays `hoerbuch`; where a row stands the app
//! decides from `tief.musik` and `tief.kunstmusik`.
//!
//! The dump states no rights; until the records are read, a row names its
//! rights `unbekannt` and is never loaded.

use maer_sync::katalog::Row;
use serde_json::{json, Value};

use crate::archive::ReadError;

/// The composer and the performers of a work, from its `Authors` cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    pub composer: Option<String>,
    pub performer: Option<String>,
}

const NEEDED: [&str; 7] = ["Text#", "Type", "Title", "Language", "Authors", "Subjects", "LoCC"];

/// Every work of type `Sound` of the dump as a row. A dump without its
/// header or without one of the columns the rows need is refused whole.
pub fn rows_from_catalog(csv: &[u8]) -> Result<Vec<Row>, ReadError> {
    let mut reader = csv::ReaderBuilder::new().has_headers(true).flexible(true).from_reader(csv);
    let header = reader.headers().map_err(|e| ReadError::Unreadable(e.to_string()))?.clone();
    let col = |name: &str| header.iter().position(|h| h == name);
    let mut at = std::collections::BTreeMap::new();
    for name in NEEDED {
        let i = col(name).ok_or_else(|| ReadError::Unreadable(format!("the dump has no column {name}")))?;
        at.insert(name, i);
    }
    let shelves = col("Bookshelves");
    let mut rows = vec![];
    for record in reader.records() {
        let record = record.map_err(|e| ReadError::Unreadable(e.to_string()))?;
        let cell = |name: &str| record.get(at[name]).unwrap_or("");
        if cell("Type") != "Sound" {
            continue;
        }
        let number = cell("Text#").trim();
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ReadError::Unreadable(format!("a work without a number: {number:?}")));
        }
        let authors = Some(cell("Authors")).filter(|s| !s.trim().is_empty());
        let mut subjects: Vec<String> = split_list(cell("Subjects"));
        if let Some(i) = shelves {
            subjects.extend(split_list(record.get(i).unwrap_or("")));
        }
        let musik = is_music(Some(cell("LoCC")));
        let p = parts(authors);
        let mut tief = json!({"musik": musik});
        if musik {
            tief["kunstmusik"] = Value::Bool(is_art_music(&subjects, authors));
            tief["komponist"] = json!(p.composer);
            tief["interpret"] = json!(p.performer);
        }
        let titel = Some(cell("Title").to_string()).filter(|s| !s.is_empty());
        let sprache = Some(cell("Language").trim().to_string()).filter(|s| !s.is_empty());
        rows.push(Row {
            quelle: "gutenberg".into(),
            id: format!("gut:{number}"),
            art: "hoerbuch".into(),
            titel,
            urheber: p.composer,
            sprache,
            jahr: None,
            rechte_kennung: "unbekannt".into(),
            rechte_url: None,
            laden: false,
            reihen: vec![],
            auch: vec![],
            tief: Some(tief.to_string()),
            beliebt_stufe: None,
        });
    }
    Ok(rows)
}

fn split_list(cell: &str) -> Vec<String> {
    cell.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// A class of the Library of Congress exactly `M`.
pub fn is_music(locc: Option<&str>) -> bool {
    locc.is_some_and(|l| l.split(';').any(|c| c.trim() == "M"))
}

/// The first level of the subjects that make a work art music (the app's
/// `GutenbergMusic.artHeadings`).
const ART: [&str; 15] = [
    "Choruses, Sacred (Mixed voices, 4 parts)", "Masses", "Operas", "Oratorios",
    "Orchestral music, Arranged", "Organ music", "Piano music", "Piano trios", "String quartets",
    "Suites (Orchestra)", "Trumpet with orchestra", "Violin and piano music",
    "Violin and piano music, Arranged", "Wind ensembles, Arranged", "Xylophone with orchestra",
];

/// The first level of the subjects that make a work light music (the app's
/// `GutenbergMusic.lightHeadings`).
const LIGHT: [&str; 35] = [
    "Accordion music", "Blues (Music)", "Bugle calls", "Carols, German", "Christmas music",
    "Dance orchestra music", "Dixieland music", "Folk dance music", "Folk music", "Folk songs, English",
    "Folk songs, Finnish", "Folk songs, French", "Folk songs, Lithuanian", "Folk songs, Spanish",
    "Foxtrots", "Gospel music", "Habaneras", "Humorous songs", "Jazz", "Klezmer music", "Kolos",
    "Minstrel shows", "National songs", "Old-time music", "Polkas", "Popular instrumental music",
    "Popular music", "Radio programs", "Ragtime music", "Reels (Music)", "Sea songs",
    "Songs, Neapolitan", "Spirituals (Songs)", "Tambura (Fretted lute) music", "Yodels",
];

/// Art music: a heading of the art list, or a composer who died before 1900,
/// and no heading of the light list.
pub fn is_art_music(subjects: &[String], authors: Option<&str>) -> bool {
    let headings: Vec<&str> = subjects.iter().map(|s| s.split(" -- ").next().unwrap_or("").trim()).collect();
    if headings.iter().any(|h| LIGHT.contains(h)) {
        return false;
    }
    headings.iter().any(|h| ART.contains(h)) || composer_before_1900(authors)
}

/// A `[Composer]` whose dates end before 1900 (`1811-1886`).
fn composer_before_1900(authors: Option<&str>) -> bool {
    authors.unwrap_or("").split(';').any(|part| {
        part.contains("[Composer]") && died(part).is_some_and(|y| y < 1900)
    })
}

/// The year after the first `dddd-dddd` of a part.
fn died(part: &str) -> Option<i32> {
    let b = part.as_bytes();
    (0..b.len().saturating_sub(8)).find_map(|i| {
        let w = &b[i..i + 9];
        (w[..4].iter().all(u8::is_ascii_digit) && w[4] == b'-' && w[5..].iter().all(u8::is_ascii_digit))
            .then(|| std::str::from_utf8(&w[5..]).ok()?.parse().ok())
            .flatten()
    })
}

/// "Last, First, 1667-1745" becomes "First Last" (the app's
/// `FreeCatalog.gutenbergAuthor`): bracketed parts and dates drop, a title of
/// address leads, a generation follows; a name without a comma stays.
fn name(raw: &str) -> Option<String> {
    let mut without = String::new();
    let mut depth = 0;
    for c in raw.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => without.push(c),
            _ => {}
        }
    }
    let parts: Vec<&str> = without.split(',').map(str::trim)
        .filter(|p| !p.is_empty() && !(p.starts_with(|c: char| c.is_ascii_digit())
            || ["active", "fl.", "ca.", "b.", "d."].iter().any(|w| p.starts_with(w))))
        .collect();
    let (last, rest) = parts.split_first()?;
    let titles: Vec<&str> = rest.iter().copied().filter(|p| ["Sir", "Lord", "Lady", "Dame"].contains(p)).collect();
    let generations: Vec<&str> = rest.iter().copied().filter(|p| ["Jr.", "Sr."].contains(p)).collect();
    let given: Vec<&str> = rest.iter().copied().filter(|p| !titles.contains(p) && !generations.contains(p)).collect();
    let all: Vec<&str> = titles.into_iter().chain(given).chain(std::iter::once(*last)).chain(generations).collect();
    let n = all.join(" ");
    (!n.is_empty()).then_some(n)
}

/// The composer leads; without one the first performer does. Performers are
/// the parts without a role, `[Performer]` or `[Conductor]`.
pub fn parts(authors: Option<&str>) -> Parts {
    let people: Vec<(String, Option<String>)> = authors.unwrap_or("").split(';').filter_map(|raw| {
        let role = raw.find('[').and_then(|a| raw[a + 1..].find(']').map(|b| raw[a + 1..a + 1 + b].to_string()));
        let bare = match raw.find('[') { Some(a) => &raw[..a], None => raw };
        name(bare.trim()).map(|n| (n, role))
    }).collect();
    let performing: Vec<String> = people.iter()
        .filter(|(_, r)| matches!(r.as_deref(), None | Some("Performer") | Some("Conductor")))
        .map(|(n, _)| n.clone()).collect();
    let joined = |v: Vec<String>| (!v.is_empty()).then(|| v.join(", "));
    if let Some((composer, _)) = people.iter().find(|(_, r)| r.as_deref() == Some("Composer")) {
        let rest: Vec<String> = performing.into_iter().filter(|n| n != composer).collect();
        return Parts { composer: Some(composer.clone()), performer: joined(rest) };
    }
    let Some(lead) = performing.first().cloned().or_else(|| people.first().map(|(n, _)| n.clone())) else {
        return Parts { composer: None, performer: None };
    };
    let rest: Vec<String> = performing.into_iter().filter(|n| *n != lead).collect();
    Parts { composer: Some(lead), performer: joined(rest) }
}
