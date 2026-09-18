// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Aufstiegs-Schluessel gegen VEROEFFENTLICHTE Vektoren.
//!
//! Das ist die staerkste Sorte, die es hier gibt: die Zahlen kommen von
//! ausserhalb (RFC 3174 und RFC 6455) und kennen keine unserer Umsetzungen.
//! Waere SHA-1 hier von uns nachgerechnet, bewiese es nur Gleichheit mit uns
//! selbst.

use maer_sync::aufstieg::{accept_key, sha1};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn sha1_rechnet_die_veroeffentlichten_vektoren() {
    // RFC 3174, Abschnitt 7.3, und der leere Fall aus derselben Familie.
    assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(hex(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
               "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
    // Eine Million 'a': prueft die Bloecke und die Laenge in Bits, nicht nur
    // den ersten Block.
    let million = vec![b'a'; 1_000_000];
    assert_eq!(hex(&sha1(&million)), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
}

/// Die Stellen, an denen eine Fuellung falsch wird: 55 Byte (die Laenge passt
/// gerade noch in den Block), 56 (sie passt nicht mehr, es braucht einen
/// zweiten), 63 und 64 (die Blockgrenze selbst), und dasselbe eine Runde
/// spaeter bei 119 bis 128. Eine Umsetzung, die nur "abc" prueft, faellt hier
/// nie auf.
///
/// **Diese acht Zahlen sind NICHT von mir.** Sie stammen aus Pythons
/// `hashlib`, also aus einer dritten Umsetzung, die weder diese noch die
/// veroeffentlichten Vektoren kennt. Der erste Anlauf hatte sie aus dem Kopf
/// geschrieben, und der Wert fuer 63 Byte war falsch: die veroeffentlichten
/// Vektoren oben blieben gruen, dieser Fall wurde rot, und rot lag die
/// ERWARTUNG, nicht die Rechnung. Genau dafuer steht er hier.
#[test]
fn die_fuellung_stimmt_an_jeder_blockgrenze() {
    let erwartet = [
        (55, "c1c8bbdc22796e28c0e15163d20899b65621d65a"),
        (56, "c2db330f6083854c99d4b5bfb6e8f29f201be699"),
        (63, "03f09f5b158a7a8cdad920bddc29b81c18a551f5"),
        (64, "0098ba824b5c16427bd7a1122a5a442a25ec644d"),
        (119, "ee971065aaa017e0632a8ca6c77bb3bf8b1dfc56"),
        (120, "f34c1488385346a55709ba056ddd08280dd4c6d6"),
        (127, "89d95fa32ed44a7c610b7ee38517ddf57e0bb975"),
        (128, "ad5b3fdbcb526778c2839d2f151ea753995e26a0"),
    ];
    for (n, will) in erwartet {
        assert_eq!(hex(&sha1(&vec![b'a'; n])), will, "{n} Byte");
    }
}

#[test]
fn der_aufstiegs_schluessel_ist_der_aus_rfc_6455() {
    // RFC 6455, Abschnitt 1.3: der Rufer schickt diesen Schluessel, der
    // Dienst antwortet mit diesem Wert.
    assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

/// Ein Schluessel mit Leerraum drumherum ist derselbe Schluessel. Kopfzeilen
/// kommen mit Leerzeichen hinter dem Doppelpunkt, und ein Dienst, der darueber
/// stolpert, weist gueltige Rufer ab.
#[test]
fn leerraum_um_den_schluessel_aendert_nichts() {
    assert_eq!(accept_key(" dGhlIHNhbXBsZSBub25jZQ== "),
               accept_key("dGhlIHNhbXBsZSBub25jZQ=="));
}
