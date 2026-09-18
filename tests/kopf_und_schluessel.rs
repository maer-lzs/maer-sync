// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die drei Stuecke, die bei der Zaehlung der 317 Faelle ohne Gegenpart
//! dastanden und rein sind: das Lesen eines Antrags- und eines Antwort-Kopfs,
//! und das Urteil ueber einen Dienst-Schluessel.
//!
//! Dass es sie gibt, ist das Ergebnis der Zaehlung. Ohne sie waere "elf von
//! elf Modulen portiert" wahr gewesen und trotzdem irrefuehrend.

use maer_sync::{frames, identity};
use serde_json::Value;

fn lese() -> Value {
    let t = std::fs::read_to_string("vektoren/kopf-und-schluessel.json").expect("Datei");
    serde_json::from_str(&t).expect("JSON")
}

#[test]
fn ein_antrags_kopf_wird_gelesen_wie_von_der_python_seite() {
    let v = lese();
    for f in v["request_of"].as_array().unwrap() {
        let (m, p, q, h) = frames::request_of(&f["head"]).expect("lesbar");
        assert_eq!(m, f["method"].as_str().unwrap(), "Methode bei {}", f["name"]);
        assert_eq!(p, f["path"].as_str().unwrap(), "Pfad bei {}", f["name"]);
        assert_eq!(String::from_utf8(q).unwrap(), f["query"].as_str().unwrap(),
                   "Frage bei {}", f["name"]);
        let soll: std::collections::BTreeMap<String, String> = f["headers"].as_object().unwrap()
            .iter().map(|(k, x)| (k.clone(), x.as_str().unwrap().to_string())).collect();
        assert_eq!(h, soll, "Kopfzeilen bei {}", f["name"]);
    }
}

#[test]
fn was_kein_antrags_kopf_ist_wird_abgewiesen() {
    let v = lese();
    for f in v["request_of_fehler"].as_array().unwrap() {
        assert!(frames::request_of(&f["head"]).is_err(), "durchgelassen: {}", f["name"]);
    }
}

#[test]
fn ein_antwort_kopf_wird_gelesen_und_ein_falscher_abgewiesen() {
    let v = lese();
    for f in v["reply_of"].as_array().unwrap() {
        let (s, h) = frames::reply_of(&f["head"]).expect("lesbar");
        assert_eq!(s as u64, f["status"].as_u64().unwrap(), "Status bei {}", f["name"]);
        assert_eq!(h.len(), f["headers"].as_object().unwrap().len());
    }
    for f in v["reply_of_fehler"].as_array().unwrap() {
        assert!(frames::reply_of(&f["head"]).is_err(), "durchgelassen: {}", f["name"]);
    }
}

#[test]
fn ein_wahrheitswert_ist_kein_status() {
    // Eigener Zeuge, weil die Sprachen hier auseinandergehen: in Python IST
    // `True` eine Zahl, und die Python-Fassung weist ihn deshalb ausdruecklich
    // ab. Waere das hier nur ein Nebeneffekt der Typen, koennte es beim
    // naechsten Umbau still verschwinden.
    let kopf: Value = serde_json::json!({"status": true});
    assert!(frames::reply_of(&kopf).is_err());
}

#[test]
fn der_stand_eines_dienst_schluessels_ist_der_gemessene() {
    let v = lese();
    assert_eq!(v["ADOPT"].as_str(), Some(identity::ADOPT));
    assert_eq!(v["MAKE_NEW"].as_str(), Some(identity::MAKE_NEW));
    assert_eq!(v["decide"].as_array().map(|a| a.len()), Some(5), "fuenf Faelle");
    for f in v["decide"].as_array().unwrap() {
        let s = identity::decide(f["found"].as_str(), f["remembered"].as_str());
        assert_eq!(s.name, f["name"].as_str().unwrap(), "Name bei {f}");
        assert_eq!(s.may_serve, f["may_serve"].as_bool().unwrap(), "may_serve bei {f}");
        assert_eq!(s.reason, f["reason"].as_str().unwrap(), "SATZ bei {f}");
    }
}

#[test]
fn die_beiden_abweisenden_faelle_sagen_was_zu_tun_ist() {
    // Der Grund, warum `reason` mitreist: eine Abweisung darf kein Terminal
    // erreichen, ohne zu sagen, was zu tun ist.
    let weg = identity::decide(None, Some("ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT"));
    assert!(!weg.may_serve);
    assert!(weg.reason.contains(identity::MAKE_NEW), "nennt den Befehl: {}", weg.reason);

    let anders = identity::decide(Some("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"),
                                  Some("ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT"));
    assert!(!anders.may_serve);
    assert!(anders.reason.contains(identity::ADOPT), "nennt den Befehl: {}", anders.reason);
    // und beide nennen BEIDE Abdruecke, damit man vergleichen kann
    assert!(anders.reason.contains("AAAA") && anders.reason.contains("ZZZZ"));
}
