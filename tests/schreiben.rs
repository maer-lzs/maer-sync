// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 2, reine Haelfte: die Tore eines Schriebs gegen `vektoren/schreiben.json`.
//!
//! Jeder Fall prueft Urteil UND SATZ. Ein Urteil ohne seinen Satz waere die
//! stille Abweisung, gegen die diese Tore gebaut sind, und ein Zeuge, der nur
//! den Status vergleicht, liesse eine falsche Begruendung durch.

use maer_sync::{admission, formats, paths, versions, writes};
use serde_json::Value;

fn lese() -> Value {
    let txt = std::fs::read_to_string("vektoren/schreiben.json").expect("schreiben.json");
    serde_json::from_str(&txt).expect("JSON")
}

fn zahl(v: &Value) -> Option<i64> { if v.is_null() { None } else { v.as_i64() } }
fn text(v: &Value) -> Option<&str> { if v.is_null() { None } else { v.as_str() } }

#[test]
fn die_vektordatei_traegt_alle_gruppen() {
    // Vorbedingung: ohne sie koennte eine halbe Datei alle Faelle gruen machen.
    let v = lese();
    for (name, soll) in [("rotate", 7), ("admit", 14), ("write_verdict", 6),
                         ("serve_verdict", 5), ("decide", 11), ("paths", 14), ("scopes", 8)] {
        assert_eq!(v[name].as_array().map(|a| a.len()), Some(soll),
                   "Gruppe {name} hat nicht {soll} Faelle");
    }
    assert_eq!(v["konstanten"]["DEFAULT_SHRINK_FLOOR"].as_f64(),
               Some(admission::DEFAULT_SHRINK_FLOOR));
    assert_eq!(v["konstanten"]["VERSION"].as_str(), Some(paths::VERSION));
    assert_eq!(v["status"]["STALE"].as_u64(), Some(writes::STALE as u64));
    assert_eq!(v["status"]["REFUSED"].as_u64(), Some(writes::REFUSED as u64));
    assert_eq!(v["status"]["WRITTEN"].as_u64(), Some(writes::WRITTEN as u64));
}

#[test]
fn fassungen_rotieren_wie_die_python_seite() {
    let v = lese();
    for fall in v["rotate"].as_array().unwrap() {
        let ids: Vec<String> = fall["ids"].as_array().unwrap().iter()
            .map(|x| x.as_str().unwrap().to_string()).collect();
        let keep = fall["keep"].as_i64().unwrap();
        let (kept, dropped) = versions::rotate(&ids, keep).expect("gueltig");
        let soll_k: Vec<String> = fall["kept"].as_array().unwrap().iter()
            .map(|x| x.as_str().unwrap().to_string()).collect();
        let soll_d: Vec<String> = fall["dropped"].as_array().unwrap().iter()
            .map(|x| x.as_str().unwrap().to_string()).collect();
        assert_eq!(kept, soll_k, "kept bei keep={keep}");
        assert_eq!(dropped, soll_d, "dropped bei keep={keep}");
        // Und die Zusicherung, die kein Fall einzeln ausspricht: nichts geht
        // verloren und nichts entsteht.
        let mut zusammen = dropped.clone(); zusammen.extend(kept.clone());
        assert_eq!(zusammen, ids, "rotate darf nichts erfinden und nichts verlieren");
    }
    for fall in v["rotate_fehler"].as_array().unwrap() {
        let keep = fall["keep"].as_i64().unwrap();
        assert!(versions::rotate(&[], keep).is_err(), "keep={keep} muss scheitern");
    }
}

#[test]
fn das_groessen_tor_urteilt_wie_die_python_seite() {
    let v = lese();
    for fall in v["admit"].as_array().unwrap() {
        let u = admission::admit(zahl(&fall["stored_bytes"]),
                                 fall["incoming_bytes"].as_i64().unwrap(),
                                 fall["declared_shrink"].as_bool().unwrap(),
                                 admission::DEFAULT_SHRINK_FLOOR).expect("gueltig");
        assert_eq!(u.accepted, fall["verdict"]["accepted"].as_bool().unwrap(),
                   "Urteil bei {fall}");
        assert_eq!(u.reason, fall["verdict"]["reason"].as_str().unwrap(),
                   "SATZ bei {fall}");
    }
    for fall in v["admit_fehler"].as_array().unwrap() {
        assert!(admission::admit(zahl(&fall["stored_bytes"]),
                                 fall["incoming_bytes"].as_i64().unwrap(),
                                 false, admission::DEFAULT_SHRINK_FLOOR).is_err(),
                "muss scheitern: {fall}");
    }
    // Der Boden selbst ist ein Wert, und ein Wert ausserhalb (0,1] ist ein
    // Rufer-Fehler, keine Vorliebe.
    for boden in [0.0, -0.5, 1.5] {
        assert!(admission::admit(Some(100), 10, false, boden).is_err(), "Boden {boden}");
    }
}

#[test]
fn die_beiden_fassungs_tore_urteilen_wie_die_python_seite() {
    let v = lese();
    for fall in v["write_verdict"].as_array().unwrap() {
        let u = formats::write_verdict(zahl(&fall["stored_format"]),
                                       zahl(&fall["writer_format"])).expect("gueltig");
        assert_eq!(u.accepted, fall["verdict"]["accepted"].as_bool().unwrap(), "{fall}");
        assert_eq!(u.reason, fall["verdict"]["reason"].as_str().unwrap(), "SATZ bei {fall}");
    }
    for fall in v["serve_verdict"].as_array().unwrap() {
        let u = formats::serve_verdict(zahl(&fall["store_schema"]),
                                       zahl(&fall["service_schema"])).expect("gueltig");
        assert_eq!(u.accepted, fall["verdict"]["accepted"].as_bool().unwrap(), "{fall}");
        assert_eq!(u.reason, fall["verdict"]["reason"].as_str().unwrap(), "SATZ bei {fall}");
    }
    // Die Gegenrichtung: eine fehlende oder unmoegliche Nummer ist ein Fehler,
    // kein stilles Ja.
    assert!(formats::write_verdict(None, None).is_err());
    assert!(formats::write_verdict(None, Some(0)).is_err());
    assert!(formats::write_verdict(Some(0), Some(1)).is_err());
    assert!(formats::serve_verdict(None, None).is_err());
    assert!(formats::serve_verdict(Some(0), Some(1)).is_err());
}

#[test]
fn ein_schrieb_wird_beurteilt_wie_von_der_python_seite() {
    let v = lese();
    for fall in v["decide"].as_array().unwrap() {
        let e = &fall["eingaben"];
        let w = writes::Write {
            stored_bytes: zahl(&e["stored_bytes"]),
            stored_format: zahl(&e["stored_format"]),
            stored_etag: text(&e["stored_etag"]),
            incoming_bytes: e["incoming_bytes"].as_i64().unwrap(),
            writer_format: zahl(&e["writer_format"]),
            if_match: e.get("if_match").and_then(|x| text(x)),
            declared_shrink: e.get("declared_shrink").and_then(|x| x.as_bool()).unwrap_or(false),
            shrink_floor: admission::DEFAULT_SHRINK_FLOOR,
        };
        let u = writes::decide(&w).expect("gueltig");
        assert_eq!(u.status, fall["status"].as_u64().unwrap() as u16, "Status bei {e}");
        assert_eq!(u.reason, fall["reason"].as_str().unwrap(), "SATZ bei {e}");
        assert_eq!(u.accepted(), fall["accepted"].as_bool().unwrap(), "accepted bei {e}");
    }
}

#[test]
fn die_ordnung_der_tore_ist_die_gemessene() {
    // Der Fall, der die Ordnung wirklich prueft: veraltete Fassung UND zu
    // alter Schreiber UND Einbruch, alles zugleich. Wer hier die Groesse
    // zuerst nennt, schickt den Klienten das Falsche reparieren.
    let w = writes::Write {
        stored_bytes: Some(100), stored_format: Some(2), stored_etag: Some("3"),
        incoming_bytes: 1, writer_format: Some(1), if_match: Some("2"),
        declared_shrink: false, shrink_floor: admission::DEFAULT_SHRINK_FLOOR,
    };
    let u = writes::decide(&w).unwrap();
    assert_eq!(u.status, writes::STALE, "die Vorbedingung spricht zuerst");
    assert!(u.reason.contains("read it, merge"), "und sie sagt, was zu tun ist: {}", u.reason);

    // Nur Format und Groesse falsch: dann spricht das Format, nicht die Groesse.
    let w2 = writes::Write { if_match: Some("3"), ..w.clone() };
    let u2 = writes::decide(&w2).unwrap();
    assert_eq!(u2.status, writes::REFUSED);
    assert!(u2.reason.contains("must not overwrite"), "Format zuerst: {}", u2.reason);
}

#[test]
fn pfade_werden_gelesen_wie_von_der_python_seite() {
    let v = lese();
    for fall in v["paths"].as_array().unwrap() {
        let p = fall["pfad"].as_str().unwrap();
        assert_eq!(paths::parse_document_path(p).as_deref(), text(&fall["document"]),
                   "Dokumentpfad {p}");
        assert_eq!(paths::parse_listing_path(p).as_deref(), text(&fall["listing"]),
                   "Listenpfad {p}");
    }
    for fall in v["scopes"].as_array().unwrap() {
        assert_eq!(paths::valid_scope(text(&fall["scope"])),
                   fall["gueltig"].as_bool().unwrap(), "Bereich {fall}");
    }
}
