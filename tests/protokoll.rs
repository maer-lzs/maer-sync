// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Was der Betreiber sieht, gegen die an der Python-Fassung gemessene Tafel.
//!
//! **Regel 3, und sie hat zwei Haelften.** Die eine: kein Fehler verschwindet
//! spurlos, also hinterlaesst jede Abweisung ihre Zeile. Die andere, und sie
//! ist die gefaehrlichere: eine Zeile nennt NIE ein Zeichen, ein Geheimnis
//! oder einen Koerper. Das Protokoll ist die Stelle, an der ein Wert in
//! Dateien wandert, die jemand anders liest.

use std::collections::BTreeMap;

use maer_sync::authorization::zufall;
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::store::Store;
use maer_sync::wege::{Anfrage, App};
use serde_json::Value;

const BESITZER: &str = "anna";
const GEHEIMNIS: &str = "ein hinreichend langes geheimnis";
const QUELLE: &str = "10.0.0.7:51234";

fn tafel() -> Value {
    let t = std::fs::read_to_string("vektoren/protokoll.json").expect("protokoll.json");
    serde_json::from_str(&t).expect("JSON")
}

struct Stand { app: App, zeichen: String }

fn aufbauen() -> Stand {
    static ZAEHLER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir().join(format!("protokoll-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None)
        .expect("Laden");
    laden.create_account(BESITZER, GEHEIMNIS).expect("Konto");
    let zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1").unwrap().unwrap();
    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).unwrap());
    let abdruck = fingerprint(&public_of(&privat)).unwrap();
    Stand { app: App::neu(laden).mit_kennung(privat, &abdruck), zeichen }
}

fn mit_quelle(mut a: Anfrage) -> Anfrage { a.client = Some(QUELLE.into()); a }

/// Was die Rust-Fassung fuer einen Fall schreibt, als (Stufe, Satz).
fn zeilen(app: &mut App) -> Vec<(String, String)> {
    app.protokoll.drain(..)
        .map(|z| (z.stufe.name().to_string(), z.satz))
        .collect()
}

#[test]
fn jede_abweisung_hinterlaesst_die_zeile_der_python_fassung() {
    let t = tafel();
    let faelle = t["faelle"].as_array().expect("faelle");
    assert!(faelle.len() >= 11, "die Tafel ist geschrumpft: {}", faelle.len());
    let mut st = aufbauen();
    let z = st.zeichen.clone();
    let mit = format!("Bearer {z}");

    // Die Faelle in der Reihenfolge der Tafel, mit denselben Anfragen.
    let mut tun: Vec<(&str, Box<dyn Fn(&mut App)>)> = Vec::new();
    tun.push(("ohne Zeichen an ein Schriftstueck", Box::new(|app: &mut App| {
        app.http(&mit_quelle(Anfrage::neu("GET", "/v1/doc/playback")));
    })));
    let m1 = mit.clone();
    let _ = m1;
    tun.push(("mit einem Zeichen, das niemandem gehoert", Box::new(|app: &mut App| {
        app.http(&mit_quelle(Anfrage::neu("GET", "/v1/doc/playback")
            .mit_kopf("authorization", "Bearer unsinnszeichen")));
    })));
    tun.push(("eine Anmeldung mit falschem Geheimnis", Box::new(|app: &mut App| {
        app.http(&mit_quelle(Anfrage::neu("POST", "/v1/token")
            .mit_koerper(br#"{"owner": "anna", "secret": "falschgeheim"}"#)));
    })));
    tun.push(("eine Anmeldung, die glueckt", Box::new(|app: &mut App| {
        app.http(&mit_quelle(Anfrage::neu("POST", "/v1/token").mit_koerper(
            br#"{"owner": "anna", "secret": "ein hinreichend langes geheimnis", "device": "geraet-9"}"#)));
    })));
    tun.push(("ein Handschlag, der keiner ist", Box::new(|app: &mut App| {
        app.http(&mit_quelle(Anfrage::neu("POST", "/v1/session")
            .mit_koerper(b"kein handschlag")));
    })));

    for (name, was) in tun {
        let will = faelle.iter().find(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("die Tafel kennt '{name}' nicht"));
        was(&mut st.app);
        pruefe(name, will, zeilen(&mut st.app));
    }

    // Die Schrieb-Faelle brauchen den Besitzer und teils einen Stand.
    let schrieb = |pfad: &str, frage: &str, koerper: &[u8]| -> Anfrage {
        mit_quelle(Anfrage::neu("PUT", pfad).mit_frage(frage)
            .mit_kopf("authorization", &mit).mit_kopf("x-maer-format", "1")
            .mit_koerper(koerper))
    };
    st.app.http(&schrieb("/v1/doc/playback", "scope=g1", b"nicht json"));
    pruefe("ein unlesbarer Schrieb",
           faelle.iter().find(|f| f["name"] == "ein unlesbarer Schrieb").unwrap(),
           zeilen(&mut st.app));

    let mut a = schrieb("/v1/doc/playback", "scope=g1", br#"{"a": 1}"#);
    a.headers.insert("content-length".into(), "99".into());
    st.app.http(&a);
    pruefe("ein Schrieb, der mitten im Koerper abbricht",
           faelle.iter().find(|f| f["name"] == "ein Schrieb, der mitten im Koerper abbricht").unwrap(),
           zeilen(&mut st.app));

    st.app.http(&schrieb("/v1/doc/playback", "scope=g2", br#"{"a": 1}"#));
    let _ = zeilen(&mut st.app);
    st.app.http(&schrieb("/v1/doc/playback", "scope=g3", br#"{"a": 2}"#));
    pruefe("ein angenommener Schrieb sagt NICHTS",
           faelle.iter().find(|f| f["name"] == "ein angenommener Schrieb sagt NICHTS").unwrap(),
           zeilen(&mut st.app));

    st.app.http(&schrieb("/v1/doc/playback", "scope=g2", b""));
    pruefe("ein Schrieb, den das Tor abweist",
           faelle.iter().find(|f| f["name"] == "ein Schrieb, den das Tor abweist").unwrap(),
           zeilen(&mut st.app));

    // Und die zwei Horcher.
    let leer: BTreeMap<String, String> = BTreeMap::new();
    let (_h, _) = st.app.horcher_oeffnen("/v1/changes", &leer, Some(QUELLE));
    pruefe("ein Horcher ohne Zeichen",
           faelle.iter().find(|f| f["name"] == "ein Horcher ohne Zeichen").unwrap(),
           zeilen(&mut st.app));
    let fremd = BTreeMap::from([
        ("authorization".to_string(), "Bearer anderesunsinnszeichen".to_string())]);
    let (_h, _) = st.app.horcher_oeffnen("/v1/changes", &fremd, Some(QUELLE));
    pruefe("ein Horcher mit fremdem Zeichen",
           faelle.iter().find(|f| f["name"] == "ein Horcher mit fremdem Zeichen").unwrap(),
           zeilen(&mut st.app));
}

fn pruefe(name: &str, will: &Value, hat: Vec<(String, String)>) {
    let soll = will["zeilen"].as_array().expect("zeilen");
    assert_eq!(hat.len(), soll.len(),
               "{name}: {} Zeilen statt {}: {hat:?}", hat.len(), soll.len());
    for ((stufe, satz), w) in hat.iter().zip(soll) {
        assert_eq!(stufe, w["stufe"].as_str().unwrap(), "{name}: Stufe");
        assert_eq!(satz, w["nachricht"].as_str().unwrap(), "{name}: Satz");
    }
    // Die gefaehrlichere Haelfte: was NIE in einer Zeile steht.
    if let Some(nie) = will.get("darf_nicht_enthalten").and_then(|x| x.as_array()) {
        for wort in nie {
            let w = wort.as_str().unwrap();
            for (_, satz) in &hat {
                assert!(!satz.contains(w),
                        "{name}: eine Protokollzeile traegt einen Wert: {satz}");
            }
        }
    }
}

/// Ein Zaun ueber dem QUELLTEXT, nicht ueber einem Lauf: die Woerter, die nie
/// in eine Zeile gehoeren, stehen in keiner Protokoll-Anweisung. Ein Lauf
/// prueft die Faelle, die er faehrt; dieser Zaun prueft die, die noch niemand
/// gefahren hat.
#[test]
fn keine_protokoll_anweisung_nennt_ein_zeichen_ein_geheimnis_oder_einen_koerper() {
    let mut anweisungen = Vec::new();
    for datei in ["src/wege.rs", "src/horcher.rs"] {
        let text = std::fs::read_to_string(datei).expect(datei);
        let mut i = 0;
        while let Some(pos) = text[i..].find("Zeile::") {
            let start = i + pos;
            let ende = text[start..].find(")));").map(|e| start + e + 4)
                .unwrap_or(text.len());
            anweisungen.push(text[start..ende].to_string());
            i = ende;
        }
    }
    assert!(anweisungen.len() >= 8,
            "der Zaun findet nur {} Protokoll-Anweisungen, er sieht zu wenig",
            anweisungen.len());
    // **Nach der QUELLE fragen, und an GANZEN Woertern.** Der erste Entwurf
    // suchte " token", "{token" und "=token", und eine eingeschmuggelte Zeile,
    // die mit `token` ANFING, ging glatt durch: das Muster griff enger als die
    // Frage. Der zweite suchte das Wort ueberall und warf `self.max_body_bytes`
    // um, eine ZAHL: er griff weiter als die Frage. Beide Male derselbe
    // Fehler, nur in zwei Richtungen.
    //
    // Die Python-Seite loest es an Wortgrenzen, und `_` zaehlt dort als
    // Wortzeichen: `\bbody\b` greift deshalb nicht in `max_body_bytes`.
    // Dieselbe Regel, von Hand, weil es keine Regex-Crate im Baum gibt.
    fn nennt_wort(text: &str, wort: &str) -> bool {
        let ist_wortzeichen = |c: char| c.is_alphanumeric() || c == '_';
        let mut i = 0;
        while let Some(pos) = text[i..].find(wort) {
            let start = i + pos;
            let ende = start + wort.len();
            let davor_frei = start == 0
                || !text[..start].chars().next_back().map(ist_wortzeichen).unwrap_or(false);
            let danach_frei = ende >= text.len()
                || !text[ende..].chars().next().map(ist_wortzeichen).unwrap_or(false);
            if davor_frei && danach_frei {
                return true;
            }
            i = ende;
        }
        false
    }

    for a in &anweisungen {
        let klein = a.to_lowercase();
        for quelle in ["headers", "secret", "token", "geheimnis", "zeichen"] {
            assert!(!nennt_wort(&klein, quelle),
                    "eine Protokoll-Anweisung greift auf '{quelle}' zu: {a}");
        }
        // Der Koerper darf gezaehlt, nie gezeigt werden. `a.body.len()` ist
        // die eine erlaubte Gestalt, und sie wird ausdruecklich genannt statt
        // erschlossen.
        let rest = klein.replace("a.body.len()", "").replace("body.len()", "");
        assert!(!nennt_wort(&rest, "body") && !nennt_wort(&rest, "koerper"),
                "eine Protokoll-Anweisung zeigt einen Koerper statt ihn zu zaehlen: {a}");
    }
}
