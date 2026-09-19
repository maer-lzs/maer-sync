// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Handschlag gegen sich selbst, nachdem er gegen fremde Zahlen bestanden hat.
//!
//! Die Reihenfolge ist dieselbe wie auf der Python-Seite und sie ist der Punkt:
//! `tests/vektoren.rs` spielt zuerst die zwei veröffentlichten Vektoren byte
//! für byte nach, denn ein Rundlauf beweist nur, dass eine Konstruktion mit
//! sich selbst übereinstimmt, und eine Konstruktion, die nicht ganz Noise ist,
//! stimmt mit sich selbst vollkommen überein.
//!
//! Was hier steht, sind die drei Fälle, die ein Vektor NICHT erreicht, weil er
//! feste Schlüssel mitbringt: ein frisch erzeugtes Paar, zwei Handschläge
//! nebeneinander, und der Zähler, der verhindert, dass ein Nonce zweimal gilt.
//! Die Zählung der 126 hat sie als ungedeckt ausgewiesen.

use maer_sync::authorization::zufall;
use maer_sync::handshake::HandshakeState;
use maer_sync::identity::public_of;

fn schluessel() -> [u8; 32] {
    let mut k = [0u8; 32];
    k.copy_from_slice(&zufall(32).expect("Zufall"));
    k
}

/// Ein Paar mit frischen Schlüsseln, so wie ein Gerät und ein Dienst es machen.
fn paar() -> (HandshakeState, HandshakeState) {
    let statisch = schluessel();
    let rufer = HandshakeState::new(true, b"", None, Some(public_of(&statisch)), None)
        .expect("Rufer");
    let dienst = HandshakeState::new(false, b"", Some(statisch), None, None)
        .expect("Dienst");
    (rufer, dienst)
}

#[test]
fn ein_frisches_paar_versteht_sich_in_beide_richtungen() {
    let (mut rufer, mut dienst) = paar();
    dienst.read_message(&rufer.write_message(b"hallo").expect("erste")).expect("gelesen");
    rufer.read_message(&dienst.write_message(b"da").expect("zweite")).expect("gelesen");

    let (mut hin, mut zurueck_lesen) = rufer.split();
    let (mut zurueck, mut hin_lesen) = dienst.split();

    let versiegelt = hin.encrypt_with_ad(b"", b"eine Position").expect("siegeln");
    assert_eq!(hin_lesen.decrypt_with_ad(b"", &versiegelt).expect("oeffnen"),
               b"eine Position", "die Richtung zum Dienst");

    let antwort = zurueck.encrypt_with_ad(b"", b"eine Antwort").expect("siegeln");
    assert_eq!(zurueck_lesen.decrypt_with_ad(b"", &antwort).expect("oeffnen"),
               b"eine Antwort", "und die Richtung zurueck");
}

#[test]
fn zwei_handschlaege_sehen_nie_gleich_aus() {
    // Ohne ein frisches Kurzzeit-Paar je Handschlag faellt die ganze
    // Konstruktion zusammen, und kein Rundlauf wuerde es bemerken: beide
    // Seiten kaemen weiter sauber zueinander.
    let (mut erster, _) = paar();
    let (mut zweiter, _) = paar();
    assert_ne!(erster.write_message(b"x").expect("erste"),
               zweiter.write_message(b"x").expect("zweite"),
               "derselbe Text, zwei Handschlaege, zwei verschiedene Nachrichten");

    // Und auch dann, wenn beide DENSELBEN Dienst rufen: der Unterschied darf
    // nicht am statischen Schluessel haengen, sondern am Kurzzeit-Schluessel.
    let statisch = schluessel();
    let oeffentlich = public_of(&statisch);
    let mut a = HandshakeState::new(true, b"", None, Some(oeffentlich), None).expect("a");
    let mut b = HandshakeState::new(true, b"", None, Some(oeffentlich), None).expect("b");
    assert_ne!(a.write_message(b"x").expect("a"), b.write_message(b"x").expect("b"),
               "zwei Anrufe an denselben Dienst sehen trotzdem verschieden aus");
}

#[test]
fn der_zaehler_rueckt_vor_damit_kein_nonce_zweimal_gilt() {
    // Ein Nonce, der sich wiederholt, gibt bei ChaCha20-Poly1305 den
    // Schluesselstrom preis. Ein Rundlauf mit EINER Nachricht sieht das nie.
    let (mut rufer, mut dienst) = paar();
    dienst.read_message(&rufer.write_message(b"").expect("erste")).expect("gelesen");
    rufer.read_message(&dienst.write_message(b"").expect("zweite")).expect("gelesen");

    let (mut hin, _) = rufer.split();
    let (_, mut hin_lesen) = dienst.split();

    let erst = hin.encrypt_with_ad(b"", b"gleich").expect("siegeln");
    let dann = hin.encrypt_with_ad(b"", b"gleich").expect("siegeln");
    assert_ne!(erst, dann, "derselbe Klartext darf nicht zweimal gleich versiegelt werden");

    // Und beide gehen beim Empfaenger der Reihe nach auf. Ohne diese zwei
    // Zeilen koennte der Zaehler auf beiden Seiten auseinanderlaufen und der
    // Fall waere trotzdem gruen.
    assert_eq!(hin_lesen.decrypt_with_ad(b"", &erst).expect("erste"), b"gleich");
    assert_eq!(hin_lesen.decrypt_with_ad(b"", &dann).expect("zweite"), b"gleich");
}
