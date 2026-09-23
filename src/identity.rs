// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wer der Dienst IST, in einer Form, die ein Mensch vergleichen kann.

use sha2::{Digest, Sha256};

/// Crockfords base32. Nicht unsere Tabelle, mit Absicht.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Byte des Abdrucks. 15 Byte sind genau 24 Zeichen, ohne Rest.
const FINGERPRINT_BYTES: usize = 15;
const GROUP: usize = 4;

/// Der geschriebene Abdruck eines rohen oeffentlichen Schluessels.
///
/// Ein Schluessel falscher Laenge wird abgewiesen statt abgedruckt: jeder
/// Rufer hat einen 32-Byte-X25519-Schluessel, und ein kuerzerer heisst, dass
/// unterwegs etwas abgeschnitten wurde. Genau dann darf ein Abdruck nicht
/// in Ordnung aussehen.
/// Der oeffentliche Schluessel zu einem privaten. Woertlich Pythons
/// `public_of`: X25519, nichts dazwischen.
pub fn public_of(private: &[u8; 32]) -> [u8; 32] {
    use x25519_dalek::{PublicKey, StaticSecret};
    *PublicKey::from(&StaticSecret::from(*private)).as_bytes()
}

pub fn fingerprint(public_key: &[u8]) -> Result<String, String> {
    if public_key.len() != 32 {
        return Err(format!(
            "a static key is 32 bytes, this one is {}",
            public_key.len()
        ));
    }
    let digest = Sha256::digest(public_key);
    let mut bits: u128 = 0;
    for b in &digest[..FINGERPRINT_BYTES] {
        bits = (bits << 8) | u128::from(*b);
    }
    let anzahl = FINGERPRINT_BYTES * 8 / 5;
    let mut zeichen = Vec::with_capacity(anzahl);
    for i in (0..anzahl).rev() {
        zeichen.push(ALPHABET[((bits >> (i * 5)) & 0x1F) as usize]);
    }
    let text = String::from_utf8(zeichen).expect("Tabelle ist ASCII");
    Ok(text
        .as_bytes()
        .chunks(GROUP)
        .map(|c| std::str::from_utf8(c).expect("ASCII"))
        .collect::<Vec<_>>()
        .join("-"))
}

/// Was der Betreiber tippt, um zu sagen, dass eine Aenderung gewollt ist.
/// Einmal benannt, damit die Abweisungen und die Hilfe nicht auseinanderlaufen.
///
/// Bis zum 24.09.2026 stand hier der Aufruf der Python-Fassung, byte-gleich mit
/// ihr, damit die zwei Umsetzungen von aussen nicht zu unterscheiden waren. Seit
/// sie das Repo verlassen hat, nannte der Satz einen Befehl, den es nicht mehr
/// gibt, und zwar genau dort, wo jemand ihn braucht: vor einem Dienst, der sich
/// weigert hochzukommen. Die Aufzeichnung in `vektoren/kopf-und-schluessel.json`
/// bleibt, wie sie gemessen wurde; der Zeuge uebersetzt dort genau diese zwei
/// Aufrufe und sonst nichts.
pub const ADOPT: &str = "maer-sync key --adopt";
pub const MAKE_NEW: &str = "maer-sync key --new";

/// Was mit dem Schluessel zu tun ist, der hier liegt, und warum.
///
/// `may_serve` ist das Einzige, worauf der Rufer verzweigt, und `reason` ist
/// der Satz, den der Betreiber liest. Sie reisen zusammen, damit eine
/// Abweisung nie ein Terminal erreicht, ohne zu sagen, was zu tun ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub name: String,
    pub may_serve: bool,
    pub reason: String,
}

/// Der Stand des Schluessels auf der Platte gegen den, den der Speicher sich
/// gemerkt hat.
///
/// Fuenf Faelle, und die zwei, die abweisen, sind der Grund fuer dieses Stueck:
/// **ein statischer Schluessel, der sich still aendert, ist keiner.** Jedes
/// Geraet, das den alten festgehalten hat, wuerde seinen Handschlag
/// scheitern sehen, ohne eine Drehung von einem Dazwischenstehenden
/// unterscheiden zu koennen.
///
/// Der bewusst nachgiebige Fall ist `remember`: ein Schluessel liegt hier und
/// der Speicher hat nie einen vermerkt. Das ist ein aus einer Sicherung
/// zurueckgeholter Speicher auf der Maschine, die ihren Schluessel noch hat.
pub fn decide(found: Option<&str>, remembered: Option<&str>) -> Standing {
    match (found, remembered) {
        (None, None) => Standing {
            name: "make_one".into(), may_serve: true,
            reason: "no static key here and none remembered, so this service is new".into() },
        (Some(f), None) => Standing {
            name: "remember".into(), may_serve: true,
            reason: format!("the key {f} is here and this store has never recorded one, \
so it is recorded now") },
        (Some(f), Some(r)) if f == r => Standing {
            name: "carry_on".into(), may_serve: true,
            reason: format!("the key {f} is the one this store expects") },
        (None, Some(r)) => Standing {
            name: "gone".into(), may_serve: false,
            reason: format!("this store expects the key {r} and there is no key file. \
Making a new one would silently break every device that pinned the old one. Put the key file \
back, or run `{MAKE_NEW}` to say that a new identity is wanted and every device should ask again.") },
        (Some(f), Some(r)) => Standing {
            name: "changed".into(), may_serve: false,
            reason: format!("this store expects the key {r} but the key file is {f}. \
Serving anyway would hand a different identity to devices that pinned the first. Put the \
expected key back, or run `{ADOPT}` to say that this key is the right one now.") },
    }
}
