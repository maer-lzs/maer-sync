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
