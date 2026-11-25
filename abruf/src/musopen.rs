// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Reader for Musopen on Wikimedia Commons.
//!
//! The index takes exactly the files the app takes (iOS
//! `MusopenCatalog.recording`), in its order: a sound at all, then the
//! licence, then a form the player takes. Rights: the `License` field of the
//! file page decides; `pd` and `cc0` give a row, an empty field with the
//! short name exactly "PDM-owner" gives `pdm`, every other licence gives
//! none. The form: the MP3 Commons derives, else an MP3 original without its
//! query. One rule more than the app: that address must be on
//! `upload.wikimedia.org`, since the device plays it later.
//!
//! Title, composer, work, performer and movement follow the rules of the app
//! (`MusopenCatalog.shownTitle`, `artistText`, `MusopenOrder.parts`,
//! `movementNumber`); `tief` carries them with the one track to play and the
//! page of the recording on Commons (`descriptionurl`), as the app keeps it.

use maer_sync::katalog::Row;
use serde_json::{json, Value};

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

/// Why a file of the category gives no row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    NotAudio,
    Licence,
    NoPlayableForm,
    ForeignHost,
}

pub fn licence(stated: Option<&str>, short: Option<&str>) -> Option<&'static str> {
    match trim_spaces(stated.unwrap_or("")).to_lowercase().as_str() {
        "pd" => Some("pd"),
        "cc0" => Some("cc0"),
        "" if short.map(trim_spaces) == Some("PDM-owner") => Some("pdm"),
        _ => None,
    }
}

fn text<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    v.pointer(path).and_then(Value::as_str)
}

fn derivatives(vi: &Value) -> &[Value] {
    vi.get("derivatives").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// A sound file: an audio type, or Ogg that Commons derives no video from.
fn is_sound(vi: &Value) -> bool {
    let Some(mime) = text(vi, "/mime") else { return false };
    if mime.starts_with("audio/") {
        return true;
    }
    mime == "application/ogg" && !derivatives(vi).iter().any(|d| text(d, "/type").unwrap_or("").starts_with("video/"))
}

/// The MP3 Commons derives, else an MP3 original without its query.
fn playable_url(vi: &Value) -> Option<String> {
    let derived = derivatives(vi).iter().find(|d| {
        text(d, "/transcodekey") == Some("mp3") || text(d, "/type").unwrap_or("").starts_with("audio/mpeg")
    });
    if let Some(src) = derived.and_then(|d| text(d, "/src")).filter(|s| !s.is_empty()) {
        return Some(src.to_string());
    }
    if text(vi, "/mime") != Some("audio/mpeg") {
        return None;
    }
    let url = text(vi, "/url").filter(|u| !u.is_empty())?;
    Some(match url.find('?') {
        None => url.to_string(),
        Some(q) => match url[q..].find('#') {
            Some(h) => format!("{}{}", &url[..q], &url[q + h..]),
            None => url[..q].to_string(),
        },
    })
}

/// The address the device plays for one `videoinfo`, or why there is none.
pub fn playable(vi: &Value) -> Result<String, Dropped> {
    if !is_sound(vi) {
        return Err(Dropped::NotAudio);
    }
    if licence(text(vi, "/extmetadata/License/value"), text(vi, "/extmetadata/LicenseShortName/value")).is_none() {
        return Err(Dropped::Licence);
    }
    let url = playable_url(vi).ok_or(Dropped::NoPlayableForm)?;
    if !url.starts_with("https://upload.wikimedia.org/") {
        return Err(Dropped::ForeignHost);
    }
    Ok(url)
}

// ---------------------------------------------------------------- text helpers

/// Swift's `.whitespaces`: spaces and tabs, not line breaks.
fn trim_spaces(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() && !is_newline(c))
}

fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}

/// Case and diacritics folded, as Swift's `folding([.caseInsensitive,
/// .diacriticInsensitive])` for the Latin letters these titles use.
fn folded(s: &str) -> String {
    const FROM: &str = "ÀÁÂÃÄÅàáâãäåĀāĂăĄąÇçĆćĈĉĊċČčĎďÈÉÊËèéêëĒēĔĕĖėĘęĚěĜĝĞğĠġĢģĤĥÌÍÎÏìíîïĨĩĪīĬĭĮįİĴĵĶķĹĺĻļĽľÑñŃńŅņŇňÒÓÔÕÖòóôõöŌōŎŏŐőŔŕŖŗŘřŚśŜŝŞşŠšŢţŤťÙÚÛÜùúûüŨũŪūŬŭŮůŰűŲųŴŵÝýÿŶŷŸŹźŻżŽž";
    const TO: &str = "AAAAAAaaaaaaAaAaAaCcCcCcCcCcDdEEEEeeeeEeEeEeEeEeGgGgGgGgHhIIIIiiiiIiIiIiIiIJjKkLlLlLlNnNnNnNnOOOOOoooooOoOoOoRrRrRrSsSsSsSsTtTtUUUUuuuuUuUuUuUuUuUuWwYyyYyYZzZzZz";
    s.chars()
        .map(|c| FROM.chars().position(|f| f == c).and_then(|i| TO.chars().nth(i)).unwrap_or(c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Replace every match of `pat`, ASCII case ignored.
fn replace_ci(s: &str, pat: &str, with: &str) -> String {
    let low = s.to_ascii_lowercase();
    let pat = pat.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0;
    while let Some(at) = low[i..].find(&pat) {
        out.push_str(&s[i..i + at]);
        out.push_str(with);
        i += at + pat.len();
    }
    out.push_str(&s[i..]);
    out
}

/// The app's `htmlPlainText`: paragraph and line breaks become breaks, every
/// other tag goes, the common entities are decoded, `&amp;` last.
fn html_plain_text(html: &str) -> String {
    let mut t = replace_ci(html, "</p>", "\n\n");
    // <br\s*/?>
    let mut out = String::new();
    let low = t.to_ascii_lowercase();
    let mut i = 0;
    while let Some(at) = low[i..].find("<br") {
        let start = i + at;
        let mut j = start + 3;
        while j < low.len() && low.as_bytes()[j].is_ascii_whitespace() { j += 1; }
        if low[j..].starts_with("/>") { j += 2; } else if low[j..].starts_with('>') { j += 1; } else {
            out.push_str(&t[i..start + 3]);
            i = start + 3;
            continue;
        }
        out.push_str(&t[i..start]);
        out.push('\n');
        i = j;
    }
    out.push_str(&t[i..]);
    t = out;
    // <[^>]+>
    let mut out = String::new();
    let mut rest = t.as_str();
    while let Some(lt) = rest.find('<') {
        match rest[lt + 1..].find('>') {
            Some(gt) if gt > 0 => {
                out.push_str(&rest[..lt]);
                rest = &rest[lt + 1 + gt + 1..];
            }
            _ => {
                out.push_str(&rest[..lt + 1]);
                rest = &rest[lt + 1..];
            }
        }
    }
    out.push_str(rest);
    for (e, r) in [("&nbsp;", " "), ("&quot;", "\""), ("&#39;", "'"), ("&apos;", "'"), ("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&")] {
        out = out.replace(e, r);
    }
    out.trim().to_string()
}

/// Remove `<tag ...>...</tag>` (case ignored, the inner part on one line)
/// where the opening tag satisfies `keep_out`.
fn remove_elements(s: &str, opening_matches: impl Fn(&str, &str) -> bool, same_tag: bool, close: &str) -> String {
    let low = s.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0;
    let mut k = 0;
    while let Some(at) = low[k..].find('<') {
        let lt = k + at;
        let name_end = lt + 1 + low[lt + 1..].find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(low.len() - lt - 1);
        let name = &low[lt + 1..name_end];
        let Some(gt) = low[name_end..].find('>').map(|g| name_end + g) else { break };
        if name.is_empty() || !opening_matches(name, &low[lt..gt]) {
            k = lt + 1;
            continue;
        }
        let closing = if same_tag { format!("</{name}>") } else { close.to_string() };
        let inner = &low[gt + 1..];
        let line_end = inner.find(is_newline).unwrap_or(inner.len());
        match inner[..line_end].find(&closing) {
            Some(c) => {
                out.push_str(&s[i..lt]);
                i = gt + 1 + c + closing.len();
                k = i;
            }
            None => k = lt + 1,
        }
    }
    out.push_str(&s[i..]);
    out
}

fn display_none(tag: &str) -> bool {
    tag.match_indices("display:").any(|(p, m)| tag[p + m.len()..].trim_start().starts_with("none"))
}

/// The artist field of a file page as text (iOS `MusopenCatalog.artistText`):
/// a hidden element goes, a link to a Commons user goes, " / " and line
/// breaks part the names, "Unknown author" is no name; `None` when nothing
/// is left.
pub fn artist_text(html: Option<&str>) -> Option<String> {
    let html = html?;
    let t = remove_elements(html, |_, tag| display_none(tag), true, "");
    let t = remove_elements(&t, |name, tag| name == "a" && tag.starts_with("<a ") && tag.contains("/wiki/user:"), false, "</a>");
    let t = html_plain_text(&t.replace(" / ", ";"));
    let mut seen = std::collections::HashSet::new();
    let parts: Vec<&str> = t.split(|c: char| c == ';' || is_newline(c)).map(str::trim)
        .filter(|p| !p.is_empty() && p.to_lowercase() != "unknown author" && seen.insert(p.to_lowercase()))
        .collect();
    if parts.is_empty() { None } else { Some(parts.join("; ")) }
}

/// The object name the page states, else the file name without its
/// namespace and its extension; an object name with markup is the rendering
/// of a template, not a name.
pub fn shown_title(object_name: Option<&str>, file_title: &str) -> String {
    if let Some(n) = object_name.map(str::trim).filter(|n| !n.is_empty() && !n.contains('<')) {
        return n.to_string();
    }
    let name = file_title.strip_prefix("File:").unwrap_or(file_title);
    match name.rfind('.') {
        Some(dot) => name[..dot].to_string(),
        None => name.to_string(),
    }
}

// ---------------------------------------------------------------- movement

/// The work stem and the movement number of a title (iOS
/// `MusopenOrder.movement`): after the last " - ", else the last ", ", a
/// Roman number in capitals or in small letters, or digits, then "." or ")"
/// and a space; only 1 to 40, and only with something before it.
fn movement(title: &str) -> Option<(String, i64)> {
    for sep in [" - ", ", "] {
        let Some(at) = title.rfind(sep) else { continue };
        let rest = &title[at + sep.len()..];
        let run = |set: &str| rest.chars().take_while(|c| set.contains(*c)).collect::<String>();
        let token = [run("IVXLC"), run("ivxlc"), run("0123456789")].into_iter().find(|t| !t.is_empty());
        let Some(token) = token else { continue };
        let after = &rest[token.len()..];
        if !(after.starts_with(". ") || after.starts_with(") ")) {
            continue;
        }
        let number = if token.chars().all(|c| c.is_ascii_digit()) {
            token.parse::<i64>().unwrap_or(0)
        } else {
            let v: Vec<i64> = token.to_uppercase().chars().map(|c| match c { 'I' => 1, 'V' => 5, 'X' => 10, 'L' => 50, _ => 100 }).collect();
            (0..v.len()).map(|i| if i + 1 < v.len() && v[i] < v[i + 1] { -v[i] } else { v[i] }).sum()
        };
        let stem = trim_spaces(&folded(&title[..at])).to_string();
        if !(1..=40).contains(&number) || stem.is_empty() {
            continue;
        }
        return Some((stem, number));
    }
    None
}

pub fn movement_number(title: &str) -> Option<i64> {
    movement(title).map(|m| m.1)
}

pub fn work_stem(title: &str) -> Option<String> {
    movement(title).map(|m| m.0)
}

// ---------------------------------------------------------------- composer

const NOT_NAME_WORDS: [&str; 30] = [
    "orchestra", "archive", "archives", "musopen", "unknown", "quartet", "ensemble", "symphony",
    "choir", "trio", "band", "philharmonic", "by", "author", "no", "op", "minor", "major", "sonata",
    "concerto", "suite", "prelude", "nocturne", "waltz", "etude", "overture", "mazurka", "in", "of", "the",
];

const KNOWN_WORKS: [(&str, &str); 1] = [("la traviata", "Giuseppe Verdi")];

const STOCK_COMPOSERS: [&str; 55] = [
    "Ludwig van Beethoven", "Johann Sebastian Bach", "Wolfgang Amadeus Mozart", "Frédéric Chopin",
    "Johannes Brahms", "Franz Schubert", "Antonín Dvořák", "Pyotr Ilyich Tchaikovsky", "Giuseppe Verdi",
    "Felix Mendelssohn", "Robert Schumann", "Franz Liszt", "Claude Debussy", "Edvard Grieg", "Joseph Haydn",
    "George Frideric Handel", "Antonio Vivaldi", "Erik Satie", "Sergei Rachmaninoff", "Modest Mussorgsky",
    "Richard Wagner", "Gioachino Rossini", "Giacomo Puccini", "Georges Bizet", "Camille Saint-Saëns",
    "Maurice Ravel", "Edward Elgar", "Gustav Holst", "Johann Strauss", "Johann Pachelbel", "Isaac Albeniz",
    "Gabriel Fauré", "Bedřich Smetana", "Jean Sibelius", "Gustav Mahler", "Anton Bruckner", "Hector Berlioz",
    "Carl Maria von Weber", "Niccolò Paganini", "Domenico Scarlatti", "Arcangelo Corelli", "Henry Purcell",
    "Georg Philipp Telemann", "Christoph Willibald Gluck", "Luigi Boccherini", "Muzio Clementi",
    "Nikolai Rimsky-Korsakov", "Alexander Borodin", "Enrique Granados", "Manuel de Falla", "Scott Joplin",
    "Gaetano Donizetti", "Vincenzo Bellini", "Jacques Offenbach", "Charles Gounod",
];

/// The family name: before a comma, else the last word.
fn family_name(composer: Option<&str>) -> String {
    let name = composer.unwrap_or("").trim();
    let name = name.split(',').next().unwrap_or("");
    name.split_whitespace().last().unwrap_or("").to_string()
}

fn family_key(name: &str) -> String {
    folded(&family_name(Some(name)))
}

/// The first part of a field that reads as a name, or `None` when the first
/// one that is not skipped is no name.
fn readable_name(raw: &str) -> Option<String> {
    for part in raw.split(|c: char| c == ';' || is_newline(c)) {
        let mut name = part;
        if let Some(colon) = name.find(':') {
            let label = trim_spaces(&name[..colon]).to_lowercase();
            if label != "composer" && label != "author" {
                continue;
            }
            name = &name[colon + 1..];
        }
        if name.to_lowercase().contains("(performer") {
            continue;
        }
        if let Some(b) = name.find('(') {
            name = &name[..b];
        }
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let lower = name.to_lowercase();
        let words: Vec<&str> = lower.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty())
            .map(|w| w.trim_matches(|c| c == '.' || c == ';')).collect();
        if words.len() > 4 || name.chars().any(char::is_numeric) || words.iter().any(|w| NOT_NAME_WORDS.contains(w))
            || lower.contains("unknown") {
            return None;
        }
        return Some(name.to_string());
    }
    None
}

/// "Family, Given" spoken as "Given Family".
fn spoken(name: &str) -> String {
    let Some(comma) = name.find(',') else { return name.to_string() };
    let family = trim_spaces(&name[..comma]);
    let given = trim_spaces(&name[comma + 1..]);
    if given.is_empty() { family.to_string() } else { format!("{given} {family}") }
}

/// A name in front of the work: before " - ", or before ", " when it is the
/// family of a stock composer.
fn named_before(title: &str) -> Option<(String, String)> {
    if let Some(d) = title.find(" - ") {
        if let Some(n) = readable_name(&title[..d]) {
            return Some((n, title[d + 3..].to_string()));
        }
    }
    if let Some(c) = title.find(", ") {
        if let Some(n) = readable_name(&title[..c]) {
            let fam = family_key(&spoken(&n));
            if STOCK_COMPOSERS.iter().any(|s| family_key(s) == fam) {
                return Some((n, title[c + 2..].to_string()));
            }
        }
    }
    None
}

pub fn composer(author: Option<&str>, title: &str) -> Option<String> {
    let f = folded(title);
    if let Some((_, who)) = KNOWN_WORKS.iter().find(|(w, _)| f.contains(w)) {
        return Some(who.to_string());
    }
    let field = readable_name(author.unwrap_or("")).map(|n| spoken(&n));
    let Some((named, _)) = named_before(title) else { return field };
    let from_title = spoken(&named);
    match field {
        Some(f) if family_key(&f) == family_key(&from_title) => Some(f),
        _ => Some(from_title),
    }
}

/// Composer, work and performer of a recording (iOS `MusopenOrder.parts`).
pub fn parts(author: Option<&str>, title: &str) -> (Option<String>, String, Option<String>) {
    let composer = composer(author, title);
    let mut work = title.to_string();
    if let (Some((named, rest)), Some(c)) = (named_before(title), composer.as_deref()) {
        if family_key(&spoken(&named)) == family_key(c) {
            work = trim_spaces(&rest).to_string();
        }
    }
    let field = author.unwrap_or("").trim();
    let names_composer = readable_name(field).map(|n| spoken(&n))
        .map(|n| composer.as_deref().map(|c| family_key(c) == family_key(&n)) == Some(true))
        .unwrap_or(false);
    let performer = if field.is_empty() || names_composer { None } else { Some(field.to_string()) };
    (composer, if work.is_empty() { title.to_string() } else { work }, performer)
}

// ---------------------------------------------------------------- answer

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
        let Ok(adresse) = playable(vi) else { continue };
        let mark = licence(text(vi, "/extmetadata/License/value"), text(vi, "/extmetadata/LicenseShortName/value"))
            .expect("playable checked the licence");
        let shown = shown_title(text(vi, "/extmetadata/ObjectName/value"), text(p, "/title").unwrap_or(""));
        // The app shows "Untitled" for an empty title or one that is the
        // identifier itself; the index then has none.
        let titel = Some(shown.trim().to_string()).filter(|t| !t.is_empty() && *t != id.to_string());
        let author = artist_text(text(vi, "/extmetadata/Artist/value"));
        let t = titel.clone().unwrap_or_default();
        let (komponist, werk, interpret) = parts(author.as_deref(), &t);
        let tief = json!({
            "komponist": &komponist, "werk": werk, "interpret": interpret, "satz": movement_number(&t),
            "seite": text(vi, "/descriptionurl").filter(|s| !s.is_empty()),
            "spuren": [{ "nummer": 1, "titel": titel, "dauer": vi.get("duration").cloned().unwrap_or(Value::Null), "adresse": adresse }],
        });
        zeilen.push(Row {
            quelle: "musopen".into(), id: format!("mso:{id}"), art: "musik".into(), titel,
            urheber: komponist.clone(), sprache: None, jahr: None, rechte_kennung: mark.into(),
            rechte_url: text(vi, "/extmetadata/LicenseUrl/value").map(str::to_string),
            laden: true, reihen: vec![], auch: vec![], tief: Some(tief.to_string()),
        });
    }
    Ok(Seite { zeilen, gesehen: pages.len(), weiter: text(&v, "/continue/gcmcontinue").map(str::to_string) })
}
