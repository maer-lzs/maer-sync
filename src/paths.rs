// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Einen Anfragepfad lesen, ohne zu wissen, was davon etwas bedeutet.
//!
//! Der Dienst prueft die FORM einer Sorte und eines Bereichs, nie die
//! Zugehoerigkeit. Eine Liste der fuenf bekannten Sorten wuerde Unsinn
//! abweisen, machte aber "eine sechste Sorte" zu einem Dienst-Update, und die
//! Forderung ist, dass der Dienst so wenig wie moeglich versteht.
//!
//! Die Python-Seite schreibt die zwei Formen als Regex. Hier stehen sie als
//! Zeichenpruefung, weil keine Regex-Crate abgenommen ist und eine dafuer zu
//! holen mehr waere als die Frage wert ist. Die Formen sind klein genug, dass
//! man sie ohne Muster lesen kann, und die Vektoren halten beide Seiten gleich.

pub const VERSION: &str = "v1";

/// Kurz, klein geschrieben, ohne Satzzeichen ausser einem Strich: erstes
/// Zeichen a bis z, danach bis zu 31 weitere aus a bis z, 0 bis 9 und Strich.
fn ist_name(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 32 { return false; }
    if !b[0].is_ascii_lowercase() { return false; }
    b[1..].iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Ein Bereich ist heute eine Geraetekennung, darf also mehr Formen tragen und
/// darf fehlen. Er darf nur nicht lang genug sein, um eine Ladung zu sein.
fn ist_bereich(s: &str) -> bool {
    if s.len() > 64 { return false; }
    s.as_bytes().iter().all(|c| {
        c.is_ascii_alphanumeric() || matches!(*c, b'.' | b'_' | b':' | b'-')
    })
}

fn teile(path: &str) -> Vec<&str> {
    path.split('/').filter(|p| !p.is_empty()).collect()
}

fn sorte_aus(path: &str, mitte: &str) -> Option<String> {
    let parts = teile(path);
    if parts.len() != 3 || parts[0] != VERSION || parts[1] != mitte {
        return None;
    }
    if ist_name(parts[2]) { Some(parts[2].to_string()) } else { None }
}

/// Aus einem Dokumentpfad die Sorte, oder None, wenn es keiner ist.
///
/// None statt eines Fehlers: ein nicht passender Pfad ist eine 404, und das
/// ist eine gewoehnliche Antwort und kein Fehler.
pub fn parse_document_path(path: &str) -> Option<String> {
    sorte_aus(path, "doc")
}

/// Aus einem Listenpfad die Sorte. Die Liste gibt Namen und Fassungen zurueck,
/// nie Koerper, damit der Dienst weiter nichts darueber lernt, was er haelt.
pub fn parse_listing_path(path: &str) -> Option<String> {
    sorte_aus(path, "docs")
}

/// Ob ein Bereich als Adresse taugt. Der leere Bereich ist der gewoehnliche
/// Fall: ein Dokument, das nicht je Geraet liegt, hat einen.
pub fn valid_scope(scope: Option<&str>) -> bool {
    ist_bereich(scope.unwrap_or(""))
}

/// The catalog path of contract version 7: `/v1/katalog/<source>` for a
/// source the catalog knows, else None (a 404).
pub const KATALOG_PATH: &str = "/v1/katalog";

pub fn parse_katalog_path(path: &str) -> Option<String> {
    let parts = teile(path);
    if parts.len() != 3 || parts[0] != VERSION || parts[1] != "katalog" {
        return None;
    }
    crate::katalog::SOURCES.contains(&parts[2]).then(|| parts[2].to_string())
}
