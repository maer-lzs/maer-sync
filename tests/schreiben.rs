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

// ---------------------------------------------------------------------------
// Die Luecken, die die Zaehlung der 126 auf Fall-Ebene gefunden hat.
//
// Alle vier gehoeren derselben Klasse an: die Tafel faehrt jeden Fall mit der
// VORGABE. Ein Parameter, den ein Betreiber wirklich verstellt, war damit
// nirgends gefahren, und die zwei Fassungsnummern waren nie gegeneinander
// gehalten. Die Sollwerte stehen nicht hier, sie sind an der Python-Fassung
// gemessen.
// ---------------------------------------------------------------------------

#[test]
fn der_boden_ist_ein_wert_und_kein_naturgesetz() {
    // Eine Aufstellung, deren Klienten hart aufraeumen, setzt ihn tiefer; eine,
    // die nie aufraeumt, hoeher. Die Zahl ist nicht die Regel. Die Tafel oben
    // faehrt ausschliesslich die Vorgabe, also war ein verstellter Boden bis
    // hierher unbezeugt.
    let tief = admission::admit(Some(1000), 400, false, 0.1).expect("gueltig");
    assert!(tief.accepted, "unter einem tiefen Boden geht derselbe Schrieb durch");
    assert_eq!(tief.reason, "400 bytes, 40% of the stored state");

    let hoch = admission::admit(Some(1000), 900, false, 0.95).expect("gueltig");
    assert!(!hoch.accepted, "unter einem hohen Boden faellt er");
    assert_eq!(hoch.reason,
               "state shrank to 90%, below the 95% floor (1000 to 900 bytes)",
               "und der Satz nennt den GESETZTEN Boden, nicht die Vorgabe");

    // Die Gegenprobe, ohne die der Fall nur zwei Zahlen vergleicht: mit der
    // Vorgabe urteilt derselbe Schrieb andersherum.
    let vorgabe = admission::admit(Some(1000), 400, false,
                                   admission::DEFAULT_SHRINK_FLOOR).expect("gueltig");
    assert!(!vorgabe.accepted, "sonst misst der Fall den Boden gar nicht");
}

#[test]
fn der_boden_reicht_durch_die_tuer_des_schriebs() {
    // Dass `admit` einen Boden kennt, heisst nicht, dass `decide` ihn
    // weiterreicht. Genau so verliert eine Zusammensetzung still ein Tor.
    let durch = writes::Write {
        stored_bytes: Some(1000), stored_format: Some(2), stored_etag: Some("v7"),
        incoming_bytes: 400, writer_format: Some(2), if_match: Some("v7"),
        declared_shrink: false, shrink_floor: 0.1,
    };
    let u = writes::decide(&durch).expect("gueltig");
    assert_eq!(u.status, writes::WRITTEN, "der gesetzte Boden erreicht das Tor");
    assert_eq!(u.reason, "400 bytes, 40% of the stored state");

    // GUTFALL DANEBEN: derselbe Schrieb mit der Vorgabe wird abgewiesen. Ohne
    // ihn koennte `decide` den Boden verschlucken und dieser Fall waere
    // trotzdem gruen.
    let vorgabe = writes::Write { shrink_floor: admission::DEFAULT_SHRINK_FLOOR, ..durch.clone() };
    let v = writes::decide(&vorgabe).expect("gueltig");
    assert_eq!(v.status, writes::REFUSED);
    assert_eq!(v.reason, "state shrank to 40%, below the 50% floor (1000 to 400 bytes)");
}

#[test]
fn die_zwei_zahlen_sind_nicht_dieselbe_zahl() {
    // Wuerden Dokumentfassung und Speichergestalt verwechselt, saehe ein
    // Fassungssprung des Klienten aus wie eine Wanderung des Speichers, und
    // ein App-Update sperrte jedes Geraet aus. Die Tafel prueft jede Zahl
    // einzeln; dieser Fall haelt sie GEGENEINANDER.
    assert!(formats::serve_verdict(Some(1), Some(1)).expect("gueltig").accepted,
            "eine weit vorausliegende Dokumentfassung sagt nichts ueber das Bedienen");
    assert!(!formats::write_verdict(Some(9), Some(1)).expect("gueltig").accepted);

    assert!(!formats::serve_verdict(Some(9), Some(1)).expect("gueltig").accepted,
            "eine weit vorausliegende Speichergestalt sagt nichts ueber das Schreiben");
    assert!(formats::write_verdict(Some(1), Some(1)).expect("gueltig").accepted);

    // Und die Stelle, an der die Python-Fassung ausdruecklich abweist: eine
    // fehlende Schreiberfassung neben einer VORHANDENEN gespeicherten. Sie
    // stillschweigend als 1 zu lesen liesse jeden Klienten, der das Feld
    // vergessen hat, alles ueberschreiben.
    assert!(formats::write_verdict(Some(3), None).is_err());
    assert!(formats::write_verdict(Some(3), Some(0)).is_err());
    assert!(formats::serve_verdict(Some(4), None).is_err());
}

#[test]
fn rotate_laesst_die_liste_des_rufers_in_ruhe() {
    // In Python ist das eine Zusicherung, die brechen KANN: `rotate` bekommt
    // die Liste des Rufers und koennte sie sortieren oder kuerzen. Hier
    // verbietet der Typ es schon, und genau deshalb steht der Zeuge da: er
    // haelt das VERSPRECHEN fest, nicht die Gefahr. Wer die Signatur eines
    // Tages auf `&mut Vec<String>` oder `Vec<String>` zieht, faellt hier.
    let gegeben: Vec<String> = ["v1", "v2", "v3", "v4"].iter().map(|s| s.to_string()).collect();
    let (kept, dropped) = versions::rotate(&gegeben, 2).expect("gueltig");
    assert_eq!(gegeben, vec!["v1", "v2", "v3", "v4"], "die Liste des Rufers ist unberuehrt");
    assert_eq!(kept, vec!["v3", "v4"]);
    assert_eq!(dropped, vec!["v1", "v2"]);
}

#[test]
fn ein_leerer_stand_ueber_einem_leeren_verliert_nichts() {
    // Der Rand, den die Tafel nicht faehrt: sie kennt "leer gespeichert, etwas
    // kommt" (0 -> 50), aber nicht "leer ueber leer". Der Unterschied ist
    // nicht akademisch: wer das leere EINGEHENDE zuerst prueft, weist hier ab
    // und verteidigt einen leeren Speicher gegen seinen einzigen Benutzer.
    let u = admission::admit(Some(0), 0, false, admission::DEFAULT_SHRINK_FLOOR)
        .expect("gueltig");
    assert!(u.accepted, "leer ueber leer verliert nichts: {}", u.reason);
    assert_eq!(u.reason, "stored state was empty, 0 bytes accepted");
}

