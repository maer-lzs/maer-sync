// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 6 Teil 6: jeder Zugriff hinterlaesst genau eine Zeile, und sie
//! nennt den INNEREN Weg.
//!
//! **Es gibt keine Tafel dafuer, und das ist kein Versaeumnis.** Die Tafeln
//! dieses Repos sind an der Python-Fassung GEMESSEN; Python schreibt keine
//! Zugriffszeilen mit innerem Weg (gemessen am 19.09.2026: 178-mal
//! `POST /v1/session 200`, nie `/v1/doc/...`, und Erfolge nie). Eine Tafel
//! haette hier also nichts, woran sie sich messen liesse. Deshalb steht die
//! Gestalt Feld fuer Feld in diesem Zeugen, und der Zaun darueber ist derselbe
//! wie fuer die Meldungen: nie ein Zeichen, nie ein Geheimnis, nie ein Koerper.

use std::collections::BTreeMap;

use maer_sync::authorization::zufall;
use maer_sync::frames::pack;
use maer_sync::handshake::HandshakeState;
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::protokoll::Art;
use maer_sync::store::Store;
use maer_sync::wege::{Anfrage, App};

const BESITZER: &str = "anna";
const GEHEIMNIS: &str = "ein hinreichend langes geheimnis";
const QUELLE: &str = "10.0.0.7:51234";
const KOERPER: &[u8] = br#"{"wert": "wert-der-nie-in-einer-zeile-erscheinen-darf"}"#;

struct Stand { app: App, zeichen: String, privat: [u8; 32] }

fn aufbauen() -> Stand {
    static ZAEHLER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir().join(format!("zugriff-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None)
        .expect("Laden");
    laden.create_account(BESITZER, GEHEIMNIS).expect("Konto");
    let zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1").unwrap().unwrap();
    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).unwrap());
    let abdruck = fingerprint(&public_of(&privat)).unwrap();
    let mut app = App::neu(laden).mit_kennung(privat, &abdruck);
    app.plain_ways = true;
    Stand { app, zeichen, privat }
}

fn von(a: Anfrage) -> Anfrage { let mut a = a; a.client = Some(QUELLE.into()); a }

/// Nur die Zugriffszeilen, als Saetze. Die Meldungen zaehlt `tests/protokoll.rs`.
fn zugriffe(app: &mut App) -> Vec<String> {
    app.protokoll.drain(..).filter(|z| z.art == Art::Zugriff).map(|z| z.satz).collect()
}

/// Die Felder einer Zeile als Abbildung, und ihre Reihenfolge.
fn felder(zeile: &str) -> (Vec<String>, BTreeMap<String, String>) {
    assert!(zeile.starts_with("access "), "eine Zugriffszeile beginnt mit `access `: {zeile}");
    let mut folge = Vec::new();
    let mut werte = BTreeMap::new();
    // `reason=` traegt Leerzeichen und steht deshalb zuletzt: alles danach ist der Satz.
    let (vorn, satz) = match zeile.split_once(" reason=") {
        Some((v, r)) => (v, Some(r.to_string())),
        None => (zeile, None),
    };
    for paar in vorn["access ".len()..].split(' ') {
        let (k, v) = paar.split_once('=').unwrap_or_else(|| panic!("kein Paar: {paar} in {zeile}"));
        folge.push(k.to_string());
        werte.insert(k.to_string(), v.to_string());
    }
    if let Some(r) = satz { folge.push("reason".into()); werte.insert("reason".into(), r); }
    (folge, werte)
}

/// Genau EINE Zeile, und sie sagt, was gefragt und was geantwortet wurde.
fn eine(app: &mut App, was: &str, method: &str, pfad: &str, status: u16, scope: &str,
        grund: Option<&str>) -> String
{
    let zeilen = zugriffe(app);
    assert_eq!(zeilen.len(), 1, "{was}: {} Zugriffszeilen statt einer: {zeilen:?}", zeilen.len());
    let (folge, w) = felder(&zeilen[0]);
    assert_eq!(w["from"], QUELLE, "{was}: Quelle");
    assert_eq!(w["method"], method, "{was}: Methode");
    assert_eq!(w["path"], pfad, "{was}: Pfad");
    assert_eq!(w["status"], status.to_string(), "{was}: Status");
    assert_eq!(w["scope"], scope, "{was}: Bereich");
    assert_eq!(w.get("reason").map(|s| s.as_str()), grund, "{was}: Grund");
    assert_eq!(&folge[..7], &["from", "session", "inner", "method", "path", "scope", "status"],
               "{was}: die Felder stehen in fester Reihenfolge");
    zeilen[0].clone()
}

#[test]
fn jede_anfrage_von_aussen_hinterlaesst_genau_eine_zugriffszeile() {
    let mut st = aufbauen();
    let mit = format!("Bearer {}", st.zeichen);
    let mut alle = Vec::new();

    st.app.http(&von(Anfrage::neu("GET", "/v1/health")));
    alle.push(eine(&mut st.app, "Gesundheit", "GET", "/v1/health", 200, "-", None));

    st.app.http(&von(Anfrage::neu("GET", "/v2/key")));
    alle.push(eine(&mut st.app, "Schluessel", "GET", "/v2/key", 200, "-", None));

    // Fassung 6, Riegel (c): der alte Schluesselweg sagt 404, und auch das
    // hinterlaesst genau eine Zeile.
    st.app.http(&von(Anfrage::neu("GET", "/v1/key")));
    alle.push(eine(&mut st.app, "alter Schluessel", "GET", "/v1/key", 404, "-",
                   Some("no such path")));

    // Fassung 6: die Anmeldung von aussen ist 426, richtig oder falsch. Die
    // Zeile nennt den Grund, und sie verraet nicht, ob das Geheimnis stimmte.
    st.app.http(&von(Anfrage::neu("POST", "/v1/token")
        .mit_koerper(br#"{"owner": "anna", "secret": "falschgeheim"}"#)));
    alle.push(eine(&mut st.app, "Anmeldung falsch", "POST", "/v1/token", 426, "-",
                   Some("the sign-in paths speak only inside a session")));

    st.app.http(&von(Anfrage::neu("POST", "/v1/token").mit_koerper(
        br#"{"owner": "anna", "secret": "ein hinreichend langes geheimnis", "device": "geraet-9"}"#)));
    alle.push(eine(&mut st.app, "Anmeldung richtig", "POST", "/v1/token", 426, "-",
                   Some("the sign-in paths speak only inside a session")));

    st.app.http(&von(Anfrage::neu("GET", "/v1/doc/playback").mit_frage("scope=g1")));
    alle.push(eine(&mut st.app, "ohne Zeichen", "GET", "/v1/doc/playback", 401, "g1",
                   Some(maer_sync::authorization::DENIED)));

    st.app.http(&von(Anfrage::neu("GET", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &mit)));
    alle.push(eine(&mut st.app, "noch kein Dokument", "GET", "/v1/doc/playback", 404, "g1",
                   Some("no such document")));

    st.app.http(&von(Anfrage::neu("PUT", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &mit).mit_kopf("x-maer-format", "1").mit_koerper(KOERPER)));
    alle.push(eine(&mut st.app, "erster Schrieb", "PUT", "/v1/doc/playback", 200, "g1", None));

    st.app.http(&von(Anfrage::neu("PUT", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &mit).mit_kopf("x-maer-format", "1").mit_koerper(KOERPER)));
    alle.push(eine(&mut st.app, "Schrieb ohne Fassung", "PUT", "/v1/doc/playback", 412, "g1",
                   Some("this document exists and the write named no version; read it, merge, and write again naming what you read")));

    st.app.http(&von(Anfrage::neu("DELETE", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &mit)));
    alle.push(eine(&mut st.app, "falsche Methode", "DELETE", "/v1/doc/playback", 405, "g1",
                   Some("DELETE is not what this path is for")));

    st.app.http(&von(Anfrage::neu("GET", "/v1/nichts").mit_kopf("authorization", &mit)));
    alle.push(eine(&mut st.app, "unbekannter Weg", "GET", "/v1/nichts", 404, "-",
                   Some("no such path")));

    nie_ein_wert(&alle, &st.zeichen);
}

/// Der Tunnel: die innere Anfrage schreibt die Zeile, der Umschlag schweigt,
/// ausser er scheitert selbst.
#[test]
fn innen_nennt_die_sitzung_und_der_getragene_umschlag_schweigt() {
    let mut st = aufbauen();
    st.app.plain_ways = false;
    let mut alle = Vec::new();

    // Der Handschlag ist ein Vorgang mit eigener Zeile.
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&st.privat)), None).unwrap();
    let erst = rufer.write_message(b"").unwrap();
    let auf = st.app.http(&von(Anfrage::neu("POST", "/v1/session").mit_koerper(&erst)));
    assert_eq!(auf.status, 200);
    let sname = auf.kopf("x-maer-session").unwrap().to_string();
    rufer.read_message(&auf.body).unwrap();
    let (mut hin, _her) = rufer.split();
    let z = eine(&mut st.app, "Handschlag", "POST", "/v1/session", 200, "-", None);
    assert!(z.contains(" inner=false "), "der Handschlag ist aussen: {z}");
    assert!(z.contains(" session=- "), "vor dem Handschlag gibt es keine Sitzung: {z}");
    alle.push(z);

    // Eine getragene Anfrage: GENAU eine Zeile, die innere. Keine fuer den Umschlag.
    let roh = pack(&serde_json::json!({ "method": "GET", "target": "/v1/health" }), b"").unwrap();
    let gab = st.app.http(&von(Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(&hin.encrypt_with_ad(b"", &roh).unwrap())));
    assert_eq!(gab.status, 200, "der Umschlag ist angekommen");
    let z = eine(&mut st.app, "innen", "GET", "/v1/health", 200, "-", None);
    assert!(z.contains(" inner=true "), "die Zeile ist die innere: {z}");
    assert!(z.contains(&format!(" session={} ", &sname[..8])),
            "die Zeile nennt die Sitzung, gekuerzt auf acht Zeichen: {z}");
    assert!(!z.contains(&sname), "aber nie den ganzen Namen: {z}");
    alle.push(z);

    // Ein Umschlag, der SELBST scheitert, bekommt seine Zeile: fremder Name.
    st.app.http(&von(Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", "0000000000000000000000000000000000000000")
        .mit_koerper(b"egal")));
    let z = eine(&mut st.app, "fremde Sitzung", "POST", "/v1/session", 410, "-",
                 Some("this session has ended"));
    assert!(z.contains(" inner=false "), "{z}");
    alle.push(z);

    // Und Rauschen unter dem echten Namen.
    st.app.http(&von(Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(b"das ist kein siegel")));
    let z = eine(&mut st.app, "Rauschen", "POST", "/v1/session", 400, "-",
                 Some("this is not a sealed message"));
    alle.push(z);

    nie_ein_wert(&alle, &st.zeichen);
}

/// Der Zaun ueber den Zeilen: was NIE darin steht.
fn nie_ein_wert(zeilen: &[String], zeichen: &str) {
    assert!(!zeilen.is_empty());
    for z in zeilen {
        assert!(!z.contains(zeichen), "eine Zugriffszeile traegt das Zeichen: {z}");
        assert!(!z.contains(GEHEIMNIS), "eine Zugriffszeile traegt das Geheimnis: {z}");
        assert!(!z.contains("wert-der-nie"), "eine Zugriffszeile traegt einen Koerper: {z}");
        assert!(!z.contains("Bearer"), "eine Zugriffszeile traegt eine Kopfzeile: {z}");
    }
}

/// Der Zaun ueber dem QUELLTEXT: die Zugriffszeile wird an genau einer Stelle
/// gebaut, und sie greift auf nichts, was ein Wert sein koennte. Dieselbe
/// Wortgrenzen-Regel wie in `tests/protokoll.rs`.
#[test]
fn die_zugriffszeile_entsteht_an_einer_stelle_und_nennt_keinen_wert() {
    let text = std::fs::read_to_string("src/wege.rs").expect("src/wege.rs");
    let stellen: Vec<usize> = text.match_indices("Zeile::zugriff(").map(|(i, _)| i).collect();
    assert_eq!(stellen.len(), 1, "die Zugriffszeile wird an {} Stellen gebaut", stellen.len());
    let start = stellen[0];
    let ende = text[start..].find(")));").map(|e| start + e + 4).expect("Ende der Anweisung");
    let anweisung = text[start..ende].to_lowercase();
    fn nennt_wort(text: &str, wort: &str) -> bool {
        let ist_wortzeichen = |c: char| c.is_alphanumeric() || c == '_';
        let mut i = 0;
        while let Some(pos) = text[i..].find(wort) {
            let (s, e) = (i + pos, i + pos + wort.len());
            let davor = s == 0 || !text[..s].chars().next_back().map(ist_wortzeichen).unwrap_or(false);
            let danach = e >= text.len() || !text[e..].chars().next().map(ist_wortzeichen).unwrap_or(false);
            if davor && danach { return true; }
            i = e;
        }
        false
    }
    for quelle in ["headers", "secret", "token", "geheimnis", "zeichen", "body", "koerper"] {
        assert!(!nennt_wort(&anweisung, quelle),
                "die Zugriffszeile greift auf '{quelle}' zu: {anweisung}");
    }
}

/// Der Doppel-Logger der Python-Fassung wird NICHT nachgebaut: `serving as`
/// und `serving on` stehen je genau einmal im Quelltext. Gemessen an 8812
/// am 19.09.2026: 66 Zeilen, 58 eindeutig, `serving as` zweimal.
#[test]
fn serving_steht_je_genau_einmal_im_quelltext() {
    let mut als = 0;
    let mut on = 0;
    for eintrag in std::fs::read_dir("src").expect("src") {
        let p = eintrag.unwrap().path();
        if p.extension().map(|e| e == "rs").unwrap_or(false) {
            let t = std::fs::read_to_string(&p).unwrap();
            als += t.matches("\"serving as ").count();
            on += t.matches("\"serving on ").count();
        }
    }
    assert_eq!(als, 1, "`serving as` steht {als}-mal");
    assert_eq!(on, 1, "`serving on` steht {on}-mal");
}
