// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Das Urteil ueber eine geprobte Rueckkehr.
//!
//! Der Zeuge, auf den es ankommt, ist nicht "passt erkennt Gleichheit",
//! sondern: **eine Probe, die nicht laufen konnte, besteht nicht.** Ein Werk
//! mit zwei Ausgaengen meldet eines Tages "passt" fuer eine Probe, die nie
//! stattgefunden hat, und dann ist die Sicherung so gut wie keine.

use maer_sync::restore::{self as rk, restore_verdict};
use serde_json::Value;
use std::collections::BTreeMap;

fn lese() -> Value {
    let t = std::fs::read_to_string("vektoren/rueckkehr.json").expect("rueckkehr.json");
    serde_json::from_str(&t).expect("JSON")
}

fn karte(v: &Value) -> Option<BTreeMap<String, i64>> {
    if v.is_null() { return None; }
    Some(v.as_object().unwrap().iter()
        .map(|(k, x)| (k.clone(), x.as_i64().unwrap())).collect())
}

#[test]
fn die_drei_ausgaenge_heissen_auf_beiden_seiten_gleich() {
    let v = lese();
    assert_eq!(v["ausgaenge"]["FITS"].as_str(), Some(rk::FITS));
    assert_eq!(v["ausgaenge"]["DIFFERS"].as_str(), Some(rk::DIFFERS));
    assert_eq!(v["ausgaenge"]["NOT_MEASURED"].as_str(), Some(rk::NOT_MEASURED));
    assert_eq!(v["faelle"].as_array().map(|a| a.len()), Some(10));
}

#[test]
fn jeder_fall_urteilt_wie_die_python_seite_und_sagt_denselben_satz() {
    let v = lese();
    for f in v["faelle"].as_array().unwrap() {
        let aus = karte(&f["backed_up"]);
        let zurueck = karte(&f["read_back"]);
        let fehler = f["error"].as_str();
        let u = restore_verdict(aus.as_ref(), zurueck.as_ref(), fehler);
        assert_eq!(u.outcome, f["outcome"].as_str().unwrap(), "Ausgang bei {}", f["name"]);
        assert_eq!(u.reason, f["reason"].as_str().unwrap(), "SATZ bei {}", f["name"]);
        assert_eq!(u.passed(), f["passed"].as_bool().unwrap(), "besteht bei {}", f["name"]);
    }
}

#[test]
fn eine_probe_die_nicht_laufen_konnte_besteht_nicht() {
    // DER Zeuge. Drei Wege, auf denen eine Probe nicht stattfindet, und keiner
    // von ihnen darf bestehen.
    let leer: BTreeMap<String, i64> = BTreeMap::new();
    let voll: BTreeMap<String, i64> = [("a".to_string(), 10)].into_iter().collect();

    let gestoppt = restore_verdict(Some(&voll), Some(&voll), Some("Platte weg"));
    assert_eq!(gestoppt.outcome, rk::NOT_MEASURED);
    assert!(!gestoppt.passed(), "eine gestoppte Probe besteht nicht");

    let nichts = restore_verdict(None, None, None);
    assert_eq!(nichts.outcome, rk::NOT_MEASURED);
    assert!(!nichts.passed(), "nichts zu vergleichen besteht nicht");

    let leere_sicherung = restore_verdict(Some(&leer), Some(&leer), None);
    assert_eq!(leere_sicherung.outcome, rk::NOT_MEASURED,
               "nichts mit nichts verglichen ist die Messung, die wie ein Erfolg aussieht");
    assert!(!leere_sicherung.passed());

    // Und die Gegenrichtung, damit der Zeuge nicht einfach alles ablehnt:
    let passt = restore_verdict(Some(&voll), Some(&voll), None);
    assert_eq!(passt.outcome, rk::FITS);
    assert!(passt.passed());
}

#[test]
fn ein_unterschied_wird_benannt_und_nicht_nur_gezaehlt() {
    let a: BTreeMap<String, i64> = [("eins".to_string(), 10), ("zwei".to_string(), 20)]
        .into_iter().collect();
    let b: BTreeMap<String, i64> = [("eins".to_string(), 10), ("zwei".to_string(), 21)]
        .into_iter().collect();
    let u = restore_verdict(Some(&a), Some(&b), None);
    assert_eq!(u.outcome, rk::DIFFERS);
    // Der Satz nennt das Dokument und beide Zahlen: wer nur "1 von 2" liest,
    // weiss nicht, wo er suchen soll.
    assert!(u.reason.contains("zwei"), "{}", u.reason);
    assert!(u.reason.contains("20 bytes out"), "{}", u.reason);
    assert!(u.reason.contains("21 bytes back"), "{}", u.reason);
}
