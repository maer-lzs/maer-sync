// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Vertrags-Zaun von der Rust-Seite.
//!
//! Der wichtigste Fall hier misst nicht den Vertrag, sondern die ZWEI SEITEN
//! gegeneinander. Eine Seite, die nur ihre Datei gegen ihre eigene Konstante
//! haelt, bleibt gruen, waehrend die andere wandert. Das ist in diesem Repo
//! am 18.09.2026 vorgekommen, und dieser Zeuge macht es unmoeglich.

use maer_sync::contract::{self, declared, digest_of, DIGEST, VERSION};
use maer_sync::{paths, writes};

fn vertrag() -> Vec<u8> {
    std::fs::read("CONTRACT.md").expect("CONTRACT.md liegt neben dem Zweig")
}

#[test]
fn der_vertrag_hat_den_digest_den_diese_seite_erwartet() {
    assert_eq!(digest_of(&vertrag()), DIGEST,
               "der Vertrag ist nicht der, den dieser Bau bedient");
}

#[test]
fn beide_seiten_halten_DIESELBE_zahl() {
    // DER Zeuge. Die Python-Seite traegt ihren Erwartungswert in
    // tests/test_contract.py; laufen die zwei auseinander, ist die Zusage
    // "byte-gleich in beiden Repos" gebrochen, ohne dass eine der beiden
    // Suiten es merkt.
    let py = std::fs::read_to_string("tests/test_contract.py").expect("die Python-Seite liegt hier");
    let zeile = py.lines()
        .find(|z| z.trim_start().starts_with("CONTRACT_DIGEST"))
        .expect("die Python-Seite nennt einen Digest");
    let ihre = zeile.split('"').nth(1).expect("in Anfuehrungszeichen");
    assert_eq!(ihre, DIGEST,
               "die beiden Seiten dieses Repos halten verschiedene Digests: \
Python {ihre}, Rust {DIGEST}");
    // Und beide gegen die Datei, sonst waeren zwei gleiche falsche Zahlen gruen.
    assert_eq!(digest_of(&vertrag()), ihre);
}

#[test]
fn der_vertrag_nennt_seine_fassung_und_es_ist_unsere() {
    let text = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&text).expect("der Vertrag hat einen maschinenlesbaren Block");
    assert_eq!(block.get("version").map(|v| v.as_slice()), Some(&[VERSION.to_string()][..]),
               "der Vertrag nennt eine andere Fassung als dieser Bau bedient");
}

#[test]
fn was_dieser_bau_beantwortet_steht_im_vertrag() {
    // Die andere Richtung: nicht "der Vertrag ist unveraendert", sondern
    // "was der Code tut, ist darin genannt". Ohne das kann der Vertrag
    // unveraendert und trotzdem unerfuellt sein.
    let text = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&text).unwrap();
    let status = block.get("status").expect("der Vertrag nennt Statuswerte");
    for s in [writes::WRITTEN, writes::STALE, writes::REFUSED] {
        assert!(status.contains(&s.to_string()),
                "dieser Bau antwortet mit {s}, der Vertrag nennt es nicht");
    }
    let pfade = block.get("paths").expect("der Vertrag nennt Pfade");
    for p in ["/v1/doc", "/v1/docs"] {
        assert!(pfade.contains(&p.to_string()), "Pfad {p} fehlt im Vertrag");
    }
    // und die Pfadform dieses Baus passt zu den genannten Pfaden
    assert_eq!(paths::parse_document_path("/v1/doc/stands").as_deref(), Some("stands"));
    assert_eq!(paths::parse_listing_path("/v1/docs/stands").as_deref(), Some("stands"));
}

#[test]
fn ein_vertrag_ohne_block_faellt_auf() {
    // Der leere Fall darf keine leere Karte sein: eine leere Karte machte
    // jede Pruefung darueber gruen, ohne etwas gemessen zu haben.
    assert!(declared("nur Prosa, kein Block").is_none());
    assert!(declared("<!-- maer-sync-contract\n-->").is_none());
    assert!(contract::declared("<!-- maer-sync-contract\nversion: 5\n-->").is_some());
}
