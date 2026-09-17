// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 1 gegen die drei Vektor-Dateien.
//!
//! Die Zeugen lesen ihre Sollwerte aus `vektoren/`, nicht aus einer zweiten
//! Umsetzung derselben Absicht. Jede Datei hat ihre Vorbedingung: ohne sie
//! koennte diese Datei gruen sein, ohne etwas gemessen zu haben.

use maer_sync::{frames, handshake, identity};
use serde_json::Value;

fn lese(pfad: &str) -> Value {
    let txt = std::fs::read_to_string(pfad)
        .unwrap_or_else(|e| panic!("{pfad} ist nicht lesbar: {e}"));
    serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{pfad} ist kein JSON: {e}"))
}

fn roh(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------- Abdruecke

#[test]
fn abdruecke_stimmen_byte_gleich() {
    let v = lese("vektoren/abdruecke.json");
    let faelle = v["faelle"].as_array().expect("faelle");
    // Vorbedingung: ohne sie ist eine leere Liste ein gruener Lauf.
    assert_eq!(faelle.len(), 4, "vier Faelle, sonst ist die Datei nicht die erwartete");
    for fall in faelle {
        let key = roh(fall["key_hex"].as_str().unwrap());
        let soll = fall["fingerprint"].as_str().unwrap();
        let ist = identity::fingerprint(&key).expect("32 Byte");
        assert_eq!(ist, soll, "Abdruck von {}", fall["name"]);
    }
}

#[test]
fn ein_schluessel_falscher_laenge_wird_abgewiesen() {
    // Die Gegenrichtung: ein abgeschnittener Schluessel darf nicht einfach
    // einen anderen Abdruck ergeben, er darf ueberhaupt keinen ergeben.
    assert!(identity::fingerprint(&[0u8; 31]).is_err());
    assert!(identity::fingerprint(&[0u8; 33]).is_err());
    assert!(identity::fingerprint(&[]).is_err());
}

#[test]
fn zwei_verschiedene_schluessel_geben_zwei_abdruecke() {
    let a = identity::fingerprint(&[0u8; 32]).unwrap();
    let b = identity::fingerprint(&[1u8; 32]).unwrap();
    assert_ne!(a, b);
}

// ------------------------------------------------------------------- Rahmen

#[test]
fn rahmen_entstehen_byte_gleich() {
    let v = lese("vektoren/rahmen.json");
    let gueltig = v["gueltig"].as_array().expect("gueltig");
    assert_eq!(gueltig.len(), 4, "vier gueltige Rahmen erwartet");
    for fall in gueltig {
        let body = roh(fall["body_hex"].as_str().unwrap());
        let frame = frames::pack(&fall["head"], &body).expect("packbar");
        assert_eq!(hex(&frame), fall["frame_hex"].as_str().unwrap(),
                   "Rahmen {}", fall["name"]);
        // und zurueck
        let (kopf, koerper) = frames::unpack(&frame).expect("lesbar");
        assert_eq!(&kopf, &fall["head"], "Kopf von {}", fall["name"]);
        assert_eq!(koerper, body, "Koerper von {}", fall["name"]);
        // die Kopflaenge im Rahmen ist die gemessene
        assert_eq!(u16::from_be_bytes([frame[0], frame[1]]) as u64,
                   fall["head_len"].as_u64().unwrap());
    }
}

#[test]
fn was_kein_rahmen_ist_ergibt_einen_fehler() {
    let v = lese("vektoren/rahmen.json");
    let ungueltig = v["ungueltig"].as_array().expect("ungueltig");
    assert_eq!(ungueltig.len(), 4, "vier Fehlerformen erwartet");
    for fall in ungueltig {
        let bytes = roh(fall["frame_hex"].as_str().unwrap());
        assert!(frames::unpack(&bytes).is_err(), "sollte scheitern: {}", fall["name"]);
    }
}

#[test]
fn ein_kopf_der_kein_objekt_ist_ist_kein_kopf() {
    // Gueltiges JSON, aber kein Kopf. Ohne diesen Zeugen wuerde eine Zahl
    // oder eine Liste als Kopf durchgehen.
    let frame = {
        let inhalt = b"[1,2]";
        let mut f = (inhalt.len() as u16).to_be_bytes().to_vec();
        f.extend_from_slice(inhalt);
        f
    };
    assert!(frames::unpack(&frame).is_err());
}

// --------------------------------------------------------------- Handschlag

fn schluessel(hex_text: &str) -> [u8; 32] {
    let mut k = [0u8; 32];
    k.copy_from_slice(&roh(hex_text));
    k
}

#[test]
fn beide_veroeffentlichten_vektoren_werden_byte_fuer_byte_nachgefahren() {
    let v = lese("vektoren/noise-nk.json");
    let vektoren = v["vectors"].as_array().expect("vectors");
    // Die Vorbedingung, und ohne sie koennte diese Datei gruen sein, ohne zu
    // messen: es sind zwei, und nicht zweimal derselbe.
    assert_eq!(vektoren.len(), 2);
    assert_ne!(vektoren[0]["init_ephemeral"], vektoren[1]["init_ephemeral"],
               "zwei Vektoren mit denselben Schluesseln waeren einer");

    for vektor in vektoren {
        assert_eq!(vektor["protocol_name"].as_str().unwrap(),
                   "Noise_NK_25519_ChaChaPoly_SHA256");
        abspielen(vektor);
    }
}

fn abspielen(vektor: &Value) {
    let mut init = handshake::HandshakeState::new(
        true,
        &roh(vektor["init_prologue"].as_str().unwrap()),
        None,
        Some(schluessel(vektor["init_remote_static"].as_str().unwrap())),
        Some(schluessel(vektor["init_ephemeral"].as_str().unwrap())),
    ).expect("Anrufer");
    let mut resp = handshake::HandshakeState::new(
        false,
        &roh(vektor["resp_prologue"].as_str().unwrap()),
        Some(schluessel(vektor["resp_static"].as_str().unwrap())),
        None,
        Some(schluessel(vektor["resp_ephemeral"].as_str().unwrap())),
    ).expect("Antwortender");

    let nachrichten = vektor["messages"].as_array().expect("messages");
    assert!(nachrichten.len() >= 2, "ein Handschlag hat zwei Nachrichten");

    let erste = init.write_message(&roh(nachrichten[0]["payload"].as_str().unwrap())).unwrap();
    assert_eq!(hex(&erste), nachrichten[0]["ciphertext"].as_str().unwrap(),
               "die erste Nachricht ist nicht die veroeffentlichte");
    assert_eq!(hex(&resp.read_message(&erste).unwrap()),
               nachrichten[0]["payload"].as_str().unwrap());

    let zweite = resp.write_message(&roh(nachrichten[1]["payload"].as_str().unwrap())).unwrap();
    assert_eq!(hex(&zweite), nachrichten[1]["ciphertext"].as_str().unwrap(),
               "die Antwort ist nicht die veroeffentlichte");
    assert_eq!(hex(&init.read_message(&zweite).unwrap()),
               nachrichten[1]["payload"].as_str().unwrap());

    if let Some(soll) = vektor["handshake_hash"].as_str() {
        assert_eq!(hex(&init.handshake_hash()), soll);
        assert_eq!(hex(&resp.handshake_hash()), soll);
    }

    let (mut i_send, mut i_recv) = init.split();
    let (mut r_send, mut r_recv) = resp.split();
    for (index, nachricht) in nachrichten.iter().enumerate().skip(2) {
        let (writer, reader): (&mut handshake::CipherState, &mut handshake::CipherState) =
            if index % 2 == 0 { (&mut i_send, &mut r_recv) } else { (&mut r_send, &mut i_recv) };
        let versiegelt = writer.encrypt_with_ad(b"", &roh(nachricht["payload"].as_str().unwrap())).unwrap();
        assert_eq!(hex(&versiegelt), nachricht["ciphertext"].as_str().unwrap(),
                   "Sitzungsnachricht {index} ist nicht die veroeffentlichte");
        assert_eq!(hex(&reader.decrypt_with_ad(b"", &roh(nachricht["ciphertext"].as_str().unwrap())).unwrap()),
                   nachricht["payload"].as_str().unwrap());
    }
}

#[test]
fn ein_veraendertes_siegel_oeffnet_nicht() {
    // Die Gegenrichtung zum Handschlag: wenn ein gekipptes Byte trotzdem
    // aufgeht, ist die Pruefsumme nicht geprueft worden.
    let v = lese("vektoren/noise-nk.json");
    let vektor = &v["vectors"][0];
    let mut init = handshake::HandshakeState::new(
        true, &roh(vektor["init_prologue"].as_str().unwrap()), None,
        Some(schluessel(vektor["init_remote_static"].as_str().unwrap())),
        Some(schluessel(vektor["init_ephemeral"].as_str().unwrap())),
    ).unwrap();
    let mut resp = handshake::HandshakeState::new(
        false, &roh(vektor["resp_prologue"].as_str().unwrap()),
        Some(schluessel(vektor["resp_static"].as_str().unwrap())), None,
        Some(schluessel(vektor["resp_ephemeral"].as_str().unwrap())),
    ).unwrap();
    let mut erste = init.write_message(b"etwas").unwrap();
    let letztes = erste.len() - 1;
    erste[letztes] ^= 0x01;
    assert!(resp.read_message(&erste).is_err(), "ein gekipptes Byte muss auffallen");
}

#[test]
fn ein_falscher_dienst_schluessel_haelt_den_handschlag_an() {
    // Der ganze Gewinn von NK: wer den privaten Teil nicht hat, kann die
    // zweite Nachricht nicht erzeugen. Hier von der anderen Seite geprueft:
    // glaubt der Anrufer einen falschen statischen Schluessel, passt nichts
    // mehr zusammen.
    let v = lese("vektoren/noise-nk.json");
    let vektor = &v["vectors"][0];
    let mut falsch = [0u8; 32];
    falsch.copy_from_slice(&roh(vektor["init_remote_static"].as_str().unwrap()));
    falsch[0] ^= 0x01;
    let mut init = handshake::HandshakeState::new(
        true, b"", None, Some(falsch),
        Some(schluessel(vektor["init_ephemeral"].as_str().unwrap())),
    ).unwrap();
    let mut resp = handshake::HandshakeState::new(
        false, b"", Some(schluessel(vektor["resp_static"].as_str().unwrap())), None,
        Some(schluessel(vektor["resp_ephemeral"].as_str().unwrap())),
    ).unwrap();
    let erste = init.write_message(b"anmeldung").unwrap();
    assert!(resp.read_message(&erste).is_err(),
            "ein falsch geglaubter Dienst-Schluessel muss den Handschlag anhalten");
}

#[test]
fn zwei_schreibweisen_eines_kopfs_haben_eine_bedeutung() {
    // VERDIKT des Gegenlesers (18.09.2026), gemessen an Vertrag, Hashes und der
    // Swift-Seite: gleiche BEDEUTUNG genuegt, es gibt keine Byte-Kanonik. Der
    // Kopf ist ein JSON-Objekt in UTF-8 mit sortierten Schluesseln; Nicht-ASCII
    // darf roh oder entwertet stehen.
    //
    // Deshalb steht hier keine Byte-Gleichheit mehr, sondern die Pflicht, die
    // wirklich gilt: BEIDE Schreibweisen muessen dieselbe Bedeutung ergeben, und
    // die eigene muss eine der beiden sein. Ein Leser, der nur eine annimmt,
    // ist der Befund.
    let v = lese("vektoren/rahmen.json");
    let fall = &v["gleiche_bedeutung"];
    let bedeutung = &fall["bedeutung"];
    let body = roh(fall["body_hex"].as_str().unwrap());
    let schreibweisen = fall["schreibweisen"].as_array().expect("schreibweisen");
    assert_eq!(schreibweisen.len(), 2, "zwei Schreibweisen, sonst messe ich nichts");
    assert_ne!(schreibweisen[0]["frame_hex"], schreibweisen[1]["frame_hex"],
               "zwei gleiche Schreibweisen waeren eine");

    let mut treffer = 0;
    for s in schreibweisen {
        let rahmen = roh(s["frame_hex"].as_str().unwrap());
        let (kopf, koerper) = frames::unpack(&rahmen)
            .unwrap_or_else(|e| panic!("Schreibweise {} ist nicht lesbar: {e}", s["name"]));
        assert_eq!(&kopf, bedeutung, "Bedeutung der Schreibweise {}", s["name"]);
        assert_eq!(koerper, body, "Koerper der Schreibweise {}", s["name"]);
        assert_eq!(u16::from_be_bytes([rahmen[0], rahmen[1]]) as u64,
                   s["head_len"].as_u64().unwrap());
        if frames::pack(bedeutung, &body).expect("packbar") == rahmen { treffer += 1; }
    }
    assert_eq!(treffer, 1, "die eigene Schreibweise muss genau eine der beiden sein");
}
