// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wer fragt, und ob er darf.
//!
//! Die Forderung stellt das mit Absicht beiseite: "getrennt zu behandeln und
//! getrennt abzunehmen. Wenn hier ein Fehler ist, stehen die Hoerstaende
//! anderer Leute offen, und das ist die eine Sorte Fehler, die man nicht mit
//! einem Update repariert."
//!
//! Drei Dinge, um die herum das hier gebaut ist, und jedes davon ist ein
//! Fehler, der leicht zu machen und teuer zu finden ist:
//!
//! - **Eine Antwort fuer jedes Scheitern.** Ein fehlendes Konto und ein
//!   falsches Geheimnis geben denselben Satz. Alles andere macht den Zugang
//!   zu einer Liste, wer hier ein Konto hat.
//! - **Zeitkonstanter Vergleich.** Ein Vergleich, der beim ersten falschen
//!   Byte zurueckkehrt, verraet das Geheimnis Byte fuer Byte.
//! - **Ein Zeichen ist kein Name.** Der Rufer legt ein Zeichen vor; der
//!   BESITZER kommt aus der Antwort des Speichers und aus nichts, was der
//!   Rufer geschrieben hat.
//!
//! Die scrypt-Rechnung selbst liegt noch NICHT hier: ihre Crate ist ein
//! eigener Entscheid. Deshalb nimmt `secret_matches_with` die Ableitung als
//! Argument. Alles davor und alle Abweisungen sind damit schon messbar, und
//! das ist der groessere Teil.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Das Einzige, was ein abgewiesener Rufer je hoert. Es sagt nichts darueber,
/// welche Haelfte falsch war.
pub const DENIED: &str = "not authorized";

/// Von wem die Anfrage ist, oder von niemandem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub owner: Option<String>,
    pub reason: String,
}

impl Caller {
    pub fn authorized(&self) -> bool { self.owner.is_some() }
    fn niemand() -> Self { Self { owner: None, reason: DENIED.to_string() } }
}

/// Was fuer ein Zeichen gespeichert wird. Ein Zeichen hat schon viel Entropie,
/// ein einfacher Abdruck genuegt also; ein langsamer Hash bei JEDER Anfrage
/// waere eine selbst zugefuegte Dienstverweigerung, also genau der umgekehrte
/// Handel zu dem, den ein Passwort braucht.
pub fn token_digest(token: &str) -> String {
    let d = Sha256::digest(token.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Die zerlegte gespeicherte Form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub n: u64,
    pub r: u32,
    pub p: u32,
    pub salt: Vec<u8>,
    pub key: Vec<u8>,
}

fn hex_zu_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None; }
    (0..s.len()).step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Zerlegt die gespeicherte Form, oder nichts.
///
/// Ein unbekanntes Verfahren ist eine Abweisung und KEIN Rueckfall auf ein
/// schwaecheres. Ein Speicher mit einer Form, die dieser Bau nicht lesen kann,
/// heisst, dass jemand zurueckgerollt hat, und die Antwort ist "nein" und
/// nicht "dann eben durchlassen".
pub fn parse_stored(stored: &str) -> Option<Stored> {
    let teile: Vec<&str> = stored.split('$').collect();
    if teile.len() != 6 || teile[0] != "scrypt" { return None; }
    Some(Stored {
        n: teile[1].parse().ok()?,
        r: teile[2].parse().ok()?,
        p: teile[3].parse().ok()?,
        salt: hex_zu_bytes(teile[4])?,
        key: hex_zu_bytes(teile[5])?,
    })
}

/// Ob `offered` das Geheimnis hinter `stored` ist, mit der Ableitung als
/// Argument.
///
/// Eine unlesbare gespeicherte Form ist `false` und nie ein Fehler, den ein
/// Behandler in eine andere Antwort verwandeln koennte als ein falsches
/// Geheimnis: die GESTALT des Scheiterns darf sich auch nicht unterscheiden.
pub fn secret_matches_with<F>(stored: &str, offered: &str, ableitung: F) -> bool
where
    F: Fn(&str, &[u8], u64, u32, u32, usize) -> Option<Vec<u8>>,
{
    if stored.is_empty() || offered.is_empty() { return false; }
    let Some(s) = parse_stored(stored) else { return false };
    let Some(berechnet) = ableitung(offered, &s.salt, s.n, s.r, s.p, s.key.len()) else {
        return false;
    };
    berechnet.ct_eq(&s.key).into()
}

/// Macht aus einer Kopfzeile einen Rufer.
///
/// `besitzer_von` nimmt den Abdruck eines Zeichens und antwortet mit dem
/// Besitzer. Der Besitzer kommt von DORT und von nirgends sonst.
pub fn caller_for<F>(header: Option<&str>, besitzer_von: F) -> Caller
where
    F: Fn(&str) -> Option<String>,
{
    let Some(h) = header else { return Caller::niemand() };
    if h.is_empty() { return Caller::niemand(); }
    let mut teile = h.splitn(2, ' ');
    let (Some(art), Some(rest)) = (teile.next(), teile.next()) else {
        return Caller::niemand();
    };
    if !art.eq_ignore_ascii_case("bearer") || rest.trim().is_empty() {
        return Caller::niemand();
    }
    match besitzer_von(&token_digest(rest.trim())) {
        Some(owner) if !owner.is_empty() => Caller { owner: Some(owner), reason: "authorized".into() },
        _ => Caller::niemand(),
    }
}
