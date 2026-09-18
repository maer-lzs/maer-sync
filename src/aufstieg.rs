// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Aufstieg vom HTTP zur Leitung: `Sec-WebSocket-Accept`.
//!
//! **Warum SHA-1 hier von Hand steht, und warum das in Ordnung ist.**
//! Gemessen: `tokio-tungstenite` mit `default-features = false` bringt kein
//! `handshake`-Modul mit, also auch kein `sha1` im Baum. Das Merkmal
//! einzuschalten zöge neue Crates nach, und das wäre eine Aufnahme (Regel 4),
//! kein Baumwachstum.
//!
//! **Das hier ist keine Sicherheitsfunktion.** RFC 6455 Abschnitt 4.2.2
//! schreibt vor, den Schlüssel des Rufers mit einem FESTEN, öffentlich
//! bekannten GUID zu verketten und den Abdruck zurückzugeben. Jeder kann das
//! nachrechnen, und genau das ist der Zweck: es beweist, dass die Gegenseite
//! den Aufstieg verstanden hat und nicht ein Zwischenspeicher antwortet. Es
//! beweist niemanden und schützt nichts. Für das, was wirklich schützt, steht
//! Noise in `handshake.rs`, und dort ist nichts von Hand.
//!
//! **Und es ist stärker bezeugt als eine eigene Zahl:** die Vektoren unten
//! sind VERÖFFENTLICHT (RFC 3174 für SHA-1, RFC 6455 für den Aufstieg), also
//! sagt jemand Drittes, was richtig ist.

use crate::wege::base64;

/// Der feste GUID aus RFC 6455, Abschnitt 1.3. Er ist keine Zutat, die
/// jemand wählt, sondern Teil des Protokolls.
pub const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Die Antwort auf `Sec-WebSocket-Key`.
pub fn accept_key(schluessel: &str) -> String {
    let mut roh = schluessel.trim().as_bytes().to_vec();
    roh.extend_from_slice(GUID.as_bytes());
    base64(&sha1(&roh))
}

/// SHA-1 nach RFC 3174. Vierzig Zeilen, und sie tun genau das, was die
/// Veröffentlichung sagt.
pub fn sha1(daten: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    // Füllung: ein gesetztes Bit, dann Nullen, dann die Länge in BITS als
    // 64 Bit big endian.
    let mut nachricht = daten.to_vec();
    let bits = (daten.len() as u64) * 8;
    nachricht.push(0x80);
    while nachricht.len() % 64 != 56 {
        nachricht.push(0);
    }
    nachricht.extend_from_slice(&bits.to_be_bytes());

    for block in nachricht.chunks(64) {
        let mut w = [0u32; 80];
        for (i, vier) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([vier[0], vier[1], vier[2], vier[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wort) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a.rotate_left(5)
                .wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wort);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut aus = [0u8; 20];
    for (i, wort) in h.iter().enumerate() {
        aus[i * 4..i * 4 + 4].copy_from_slice(&wort.to_be_bytes());
    }
    aus
}
