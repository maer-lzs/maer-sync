// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 3, reine Haelfte: Abdruecke, Kopfzeilen, und die Zerlegung der
//! gespeicherten Form.
//!
//! Der Kniff in diesem Werk: wo die scrypt-Rechnung stehen wird, steht hier
//! eine eingesetzte Ableitung. Fuer die Abweisungen wird sie so gewaehlt, dass
//! sie IMMER den richtigen Schluessel liefert. Was dann trotzdem abgewiesen
//! wird, wird von der Zerlegung abgewiesen und nicht vom Vergleich, und genau
//! das ist die Zusicherung, die geprueft gehoert.

use maer_sync::authorization as az;
use serde_json::Value;

fn lese() -> Value {
    let t = std::fs::read_to_string("vektoren/zeichen.json").expect("zeichen.json");
    serde_json::from_str(&t).expect("JSON")
}

fn text(v: &Value) -> Option<&str> { if v.is_null() { None } else { v.as_str() } }

#[test]
fn die_datei_traegt_alle_gruppen_und_den_einen_satz() {
    let v = lese();
    assert_eq!(v["DENIED"].as_str(), Some(az::DENIED),
               "der eine Satz muss auf beiden Seiten derselbe sein");
    for (name, soll) in [("token_digest", 6), ("caller_for", 12),
                         ("gespeicherte_formen", 2), ("unlesbare_formen", 8)] {
        assert_eq!(v[name].as_array().map(|a| a.len()), Some(soll), "Gruppe {name}");
    }
}

#[test]
fn abdruecke_von_zeichen_stimmen() {
    let v = lese();
    for f in v["token_digest"].as_array().unwrap() {
        assert_eq!(az::token_digest(f["token"].as_str().unwrap()),
                   f["digest"].as_str().unwrap(), "Abdruck von {:?}", f["token"]);
    }
}

#[test]
fn kopfzeilen_werden_gelesen_wie_von_der_python_seite() {
    let v = lese();
    let bekannt = az::token_digest("gueltiges-zeichen");
    let sucher = |d: &str| if d == bekannt { Some("lzs".to_string()) } else { None };
    for f in v["caller_for"].as_array().unwrap() {
        let c = az::caller_for(text(&f["header"]), sucher);
        assert_eq!(c.owner.as_deref(), text(&f["owner"]), "Besitzer bei {:?}", f["header"]);
        assert_eq!(c.reason, f["reason"].as_str().unwrap(), "Satz bei {:?}", f["header"]);
        assert_eq!(c.authorized(), f["authorized"].as_bool().unwrap());
    }
}

#[test]
fn die_gespeicherte_form_wird_zerlegt_wie_sie_geschrieben_wurde() {
    let v = lese();
    let p = &v["parameter"];
    for f in v["gespeicherte_formen"].as_array().unwrap() {
        let s = az::parse_stored(f["stored"].as_str().unwrap()).expect("lesbar");
        assert_eq!(s.n, p["n"].as_u64().unwrap());
        assert_eq!(s.r as u64, p["r"].as_u64().unwrap());
        assert_eq!(s.p as u64, p["p"].as_u64().unwrap());
        assert_eq!(s.salt.len() as u64, p["salt_bytes"].as_u64().unwrap());
        assert_eq!(s.key.len() as u64, p["key_bytes"].as_u64().unwrap());
    }
}

#[test]
fn eine_unlesbare_form_wird_abgewiesen_auch_wenn_die_ableitung_ja_sagt() {
    // DER Zeuge dieser Scheibe. Die Ableitung liefert hier IMMER den
    // gespeicherten Schluessel; was trotzdem abgewiesen wird, wird von der
    // Zerlegung abgewiesen. Ohne diesen Kniff koennte eine Abweisung auch vom
    // Vergleich kommen, und die Zusicherung "unbekanntes Verfahren ist nie
    // ein Rueckfall" waere ungemessen.
    let v = lese();
    for f in v["unlesbare_formen"].as_array().unwrap() {
        let stored = f["stored"].as_str().unwrap();
        // Die Ableitung gibt den Schluessel zurueck, der in DIESER Form steht,
        // sofern sich einer herauslesen laesst. Der erste Anlauf gab Nullen,
        // und damit bewies der Fall nichts: eine Abweisung konnte auch vom
        // Vergleich kommen. Die Rot-Probe "unbekanntes Verfahren wird gelesen"
        // blieb deshalb gruen, und genau daran ist es aufgefallen.
        let aus_der_form: Vec<u8> = stored.rsplit('$').next()
            .and_then(|h| (0..h.len()).step_by(2)
                .map(|i| u8::from_str_radix(h.get(i..i + 2).unwrap_or("zz"), 16).ok())
                .collect::<Option<Vec<u8>>>())
            .unwrap_or_default();
        let gibt_den_gespeicherten = |_: &str, _: &[u8], _: u64, _: u32, _: u32, _: usize| {
            Some(aus_der_form.clone())
        };
        assert!(!az::secret_matches_with(stored, "egal", gibt_den_gespeicherten),
                "unlesbare Form durchgelassen: {stored:?}");
        assert_eq!(f["matches"].as_bool().unwrap(), false, "die Python-Seite sagt auch nein");
    }
}

#[test]
fn ein_leeres_angebot_und_eine_leere_form_sind_nein() {
    let v = lese();
    let stored = v["gespeicherte_formen"][0]["stored"].as_str().unwrap();
    let immer_ja = |_: &str, _: &[u8], _: u64, _: u32, _: u32, l: usize| Some(vec![0u8; l]);
    assert!(!az::secret_matches_with(stored, "", immer_ja));
    assert!(!az::secret_matches_with("", "etwas", immer_ja));
}

#[test]
fn der_vergleich_sagt_ja_nur_beim_richtigen_schluessel() {
    let v = lese();
    let stored = v["gespeicherte_formen"][0]["stored"].as_str().unwrap();
    let echt = az::parse_stored(stored).unwrap().key;
    let richtig = |_: &str, _: &[u8], _: u64, _: u32, _: u32, _: usize| Some(echt.clone());
    assert!(az::secret_matches_with(stored, "hallo", richtig));

    // Gegenrichtung: ein Schluessel, der um EIN Byte abweicht, ist nein.
    let mut fast = echt.clone();
    let letztes = fast.len() - 1;
    fast[letztes] ^= 0x01;
    let knapp_daneben = |_: &str, _: &[u8], _: u64, _: u32, _: u32, _: usize| Some(fast.clone());
    assert!(!az::secret_matches_with(stored, "hallo", knapp_daneben));

    // Und eine Ableitung, die scheitert, ist ebenfalls nein und kein Fehler.
    let scheitert = |_: &str, _: &[u8], _: u64, _: u32, _: u32, _: usize| None;
    assert!(!az::secret_matches_with(stored, "hallo", scheitert));
}
