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

/// Die AUSNAHME dieser Tafel, mit Grund: aufgezeichnet ist der Aufruf der
/// Python-Fassung, und die gibt es seit dem 24.09.2026 nicht mehr. Der Dienst
/// nennt seither seinen eigenen (`identity::ADOPT`, `identity::MAKE_NEW`).
/// Uebersetzt werden genau diese zwei Aufrufe, und der Zeuge unten zaehlt, dass
/// die Uebersetzung genau die zwei abweisenden Saetze trifft: trifft sie
/// weniger, ist die Ausnahme ohne Gegenstand, trifft sie mehr, deckt sie etwas,
/// das niemand gemeint hat.
const AUFGEZEICHNET_ADOPT: &str = "python3 -m service key --adopt";
const AUFGEZEICHNET_MAKE_NEW: &str = "python3 -m service key --new";

#[test]
fn der_stand_eines_dienst_schluessels_ist_der_gemessene() {
    let v = lese();
    // Die Aufzeichnung ist unveraendert, und die Uebersetzung weiss, wovon.
    assert_eq!(v["ADOPT"].as_str(), Some(AUFGEZEICHNET_ADOPT));
    assert_eq!(v["MAKE_NEW"].as_str(), Some(AUFGEZEICHNET_MAKE_NEW));
    assert_eq!(v["decide"].as_array().map(|a| a.len()), Some(5), "fuenf Faelle");
    let mut uebersetzt = Vec::new();
    for f in v["decide"].as_array().unwrap() {
        let s = identity::decide(f["found"].as_str(), f["remembered"].as_str());
        assert_eq!(s.name, f["name"].as_str().unwrap(), "Name bei {f}");
        assert_eq!(s.may_serve, f["may_serve"].as_bool().unwrap(), "may_serve bei {f}");
        let aufgezeichnet = f["reason"].as_str().unwrap();
        let soll = aufgezeichnet
            .replace(AUFGEZEICHNET_ADOPT, identity::ADOPT)
            .replace(AUFGEZEICHNET_MAKE_NEW, identity::MAKE_NEW);
        if soll != aufgezeichnet {
            uebersetzt.push(s.name.clone());
        }
        assert_eq!(s.reason, soll, "SATZ bei {f}");
    }
    assert_eq!(uebersetzt, ["gone", "changed"],
               "die Ausnahme trifft genau die zwei abweisenden Saetze");
}

#[test]
fn kein_satz_des_dienstes_nennt_die_python_fassung() {
    // Die Gegenrichtung der Ausnahme: was der Dienst dem Betreiber sagt, nennt
    // keinen Aufruf, den es nicht mehr gibt.
    for (f, r) in [(None, Some("ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT")),
                   (Some("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"), Some("ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT"))] {
        let s = identity::decide(f, r);
        assert!(!s.reason.contains("python"), "nennt Python: {}", s.reason);
    }
    assert!(!identity::ADOPT.contains("python") && !identity::MAKE_NEW.contains("python"));
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

#[test]
fn ein_kopf_laenger_als_das_laengenfeld_wird_nicht_gepackt() {
    // Das Laengenfeld hat zwei Byte. Ein Kopf darueber laesst sich nicht
    // ankuendigen, und ein Packer, der es trotzdem versucht, schreibt eine
    // gelogene Laenge auf die Leitung: der Leser nimmt dann den Rest des
    // Kopfs fuer den Koerper. Gepackt wird gar nicht, statt falsch.
    let zu_lang = "y".repeat(1 << 16);
    let kopf: Value = serde_json::json!({ "x": zu_lang });
    assert!(frames::pack(&kopf, b"").is_err(), "ein zu langer Kopf wird nicht gepackt");

    // GUTFALL DANEBEN, sonst misst der Fall nur, dass `pack` scheitern kann:
    // knapp darunter geht derselbe Kopf durch.
    let knapp: Value = serde_json::json!({ "x": "y".repeat(60_000) });
    assert!(frames::pack(&knapp, b"").is_ok(), "knapp darunter wird gepackt");
}

#[test]
fn derselbe_kopf_gibt_dieselben_bytes_egal_wie_er_geschrieben_wurde() {
    // Auf der Python-Seite ist das eine echte Gefahr: ein `dict` hat eine
    // Einfuegereihenfolge, und ohne `sort_keys` haengen die Bytes daran. Hier
    // sortiert schon das Einlesen, also wird die Zusicherung dort gemessen,
    // wo sie zaehlt: am Uebergang von TEXT zu Rahmen, in beiden Schreibweisen.
    let eine: Value = serde_json::from_str(r#"{"b":"2","a":"1"}"#).expect("JSON");
    let andere: Value = serde_json::from_str(r#"{"a":"1","b":"2"}"#).expect("JSON");
    let links = frames::pack(&eine, b"").expect("packbar");
    let rechts = frames::pack(&andere, b"").expect("packbar");
    assert_eq!(links, rechts, "dieselbe Bedeutung, dieselben Bytes");

    // Und die Gegenprobe, sonst vergleicht der Fall zwei gleiche Dinge: ein
    // anderer Kopf gibt andere Bytes.
    let dritte: Value = serde_json::from_str(r#"{"a":"1","b":"3"}"#).expect("JSON");
    assert_ne!(links, frames::pack(&dritte, b"").expect("packbar"));
}

