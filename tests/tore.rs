// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Tore, die gebaut waren und die niemand befahren hat.
//!
//! Die Zählung auf Fall-Ebene hat sie als Klasse sichtbar gemacht: die
//! Körpergrenze, die zwei Umgebungsvariablen, die Prozent-Entschlüsselung im
//! Bereich, ein widerrufenes Zeichen, die Trennung zweier Besitzer über die
//! Wege. Jedes davon ist im Quelltext zu lesen und war von keinem Fall
//! gefahren, und ein Tor, das niemand befährt, ist ein Tor, von dem niemand
//! weiß, ob es hält.
//!
//! **Zwei Fälle prüfen ausdrücklich den LADEN und nicht die Antwort.** Dass
//! eine Abweisung 401 sagt, ist die halbe Zusicherung; die andere ist, dass
//! danach nichts im Laden steht.

use std::sync::{Mutex, MutexGuard};

use maer_sync::authorization::zufall;
use maer_sync::handshake::HandshakeState;
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::store::Store;
use maer_sync::wege::{alte_wege_aus, grenze_aus, Anfrage, App, DEFAULT_MAX_BODY_BYTES,
                      GRENZE, PLAIN_WAYS};

/// Die Umgebung ist EIN Ding je Prozess. Zwei Zeugen, die daran drehen,
/// messen sonst einander statt die Sache.
static UMGEBUNG: Mutex<()> = Mutex::new(());

fn allein() -> MutexGuard<'static, ()> {
    UMGEBUNG.lock().unwrap_or_else(|v| v.into_inner())
}

struct Stand { app: App, zeichen: String, privat: [u8; 32], wo: std::path::PathBuf }

/// Ein Stand, der NICHT von der Umgebung abhaengt.
///
/// Das ist kein Schoenheitsfehler, den ich glaette, sondern der Grund fuer
/// einen roten Lauf: die zwei Umgebungs-Zeugen setzen eine prozessweite
/// Variable, die uebrigen Faelle liefen parallel dagegen, und ploetzlich
/// antwortete der Dienst ihnen mit 426. Ein Schloss zwischen den
/// Umgebungs-Zeugen schuetzt die NACHBARN nicht.
///
/// Also: wer die Umgebung nicht misst, haengt auch nicht an ihr.
fn aufbauen() -> Stand {
    let mut st = aufbauen_mit("anna");
    st.app.plain_ways = true;
    st.app.max_body_bytes = DEFAULT_MAX_BODY_BYTES;
    st
}

/// Und der Stand, der die Umgebung SEHR WOHL liest. Nur die zwei Zeugen, die
/// sie messen, benutzen ihn, und sie halten dabei das Schloss.
fn aufbauen_roh() -> Stand { aufbauen_mit("anna") }

fn aufbauen_mit(besitzer: &str) -> Stand {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir().join(format!("tore-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None)
        .expect("Laden");
    laden.create_account(besitzer, "ein hinreichend langes geheimnis").expect("Konto");
    let zeichen = laden.issue_token(besitzer, "ein hinreichend langes geheimnis", "g1")
        .unwrap().unwrap();
    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).unwrap());
    let abdruck = fingerprint(&public_of(&privat)).unwrap();
    Stand { app: App::neu(laden).mit_kennung(privat, &abdruck), zeichen, privat, wo }
}

fn schrieb(zeichen: &str, frage: &str, koerper: &[u8]) -> Anfrage {
    Anfrage::neu("PUT", "/v1/doc/playback").mit_frage(frage)
        .mit_kopf("authorization", &format!("Bearer {zeichen}"))
        .mit_kopf("x-maer-format", "1")
        .mit_koerper(koerper)
}

// ------------------------------------------------- der Laden, nicht die Antwort

#[test]
fn ein_schrieb_ohne_zeichen_erreicht_den_laden_nie() {
    let mut st = aufbauen();
    let gab = st.app.http(&Anfrage::neu("PUT", "/v1/doc/playback")
        .mit_frage("scope=g1").mit_kopf("x-maer-format", "1")
        .mit_koerper(br#"{"a": 1}"#));
    assert_eq!(gab.status, 401);
    // Die zweite Haelfte der Zusicherung, und die eigentliche: im Laden steht
    // nichts. Eine 401, nach der das Dokument trotzdem liegt, waere schlimmer
    // als gar keine Pruefung, weil sie beruhigt.
    assert_eq!(st.app.store.current("anna", "playback", "g1").unwrap().is_none(), true,
               "nach einer 401 darf im Laden nichts liegen");
}

#[test]
fn ein_schriftstueck_ist_fuer_das_andere_konto_unsichtbar() {
    let mut st = aufbauen();
    let bodos = st.app.store.create_account("bodo", "ein ganz anderes langes geheimnis")
        .and_then(|_| st.app.store.issue_token("bodo", "ein ganz anderes langes geheimnis", "gx"))
        .expect("Konto").expect("Zeichen");
    let z = st.zeichen.clone();

    assert_eq!(st.app.http(&schrieb(&z, "scope=g1", br#"{"a": 1}"#)).status, 200);

    // Bodo fragt denselben Weg mit demselben Bereich.
    let gab = st.app.http(&Anfrage::neu("GET", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &format!("Bearer {bodos}")));
    assert_eq!(gab.status, 404, "fuer Bodo gibt es das Dokument nicht");
    // Und sein Schrieb beruehrt Annas Stand nicht.
    assert_eq!(st.app.http(&schrieb(&bodos, "scope=g1", br#"{"b": 2}"#)).status, 200);
    let annas = st.app.store.current("anna", "playback", "g1").unwrap().expect("Annas Stand");
    assert_eq!(annas.body, br#"{"a": 1}"#, "Annas Dokument ist unveraendert");
}

#[test]
fn ein_widerrufenes_zeichen_hoert_auf_zu_wirken() {
    let mut st = aufbauen();
    let z = st.zeichen.clone();
    assert_eq!(st.app.http(&schrieb(&z, "scope=g1", br#"{"a": 1}"#)).status, 200);

    let abdruck = maer_sync::authorization::token_digest(&z);
    st.app.store.revoke_token(&abdruck).expect("widerrufen");

    let gab = st.app.http(&Anfrage::neu("GET", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &format!("Bearer {z}")));
    assert_eq!(gab.status, 401, "das widerrufene Zeichen traegt nicht mehr");
    assert_eq!(gab.grund().as_deref(), Some("not authorized"));
}

// ------------------------------------------------------------ die Koerpergrenze

#[test]
fn ein_koerper_ueber_der_grenze_ist_413_und_erreicht_den_laden_nie() {
    let mut st = aufbauen();
    st.app.max_body_bytes = 64;
    let z = st.zeichen.clone();
    let gross = format!("{{\"a\": \"{}\"}}", "x".repeat(200));
    let gab = st.app.http(&schrieb(&z, "scope=g1", gross.as_bytes()));
    assert_eq!(gab.status, 413);
    assert_eq!(gab.grund().as_deref(), Some("the document is larger than this host takes"));
    assert!(st.app.store.current("anna", "playback", "g1").unwrap().is_none(),
            "nach einer 413 darf im Laden nichts liegen");
    // Und knapp darunter geht es durch, sonst misst der Fall nur eine Mauer.
    assert_eq!(st.app.http(&schrieb(&z, "scope=g1", br#"{"a": 1}"#)).status, 200);
}

#[test]
fn die_grenze_kommt_aus_der_umgebung() {
    let _allein = allein();
    // Erst die ENTSCHEIDUNG, ohne Umgebung: sie ist eine reine Funktion.
    assert_eq!(grenze_aus(None), DEFAULT_MAX_BODY_BYTES);
    assert_eq!(grenze_aus(Some("4096".into())), 4096);
    assert_eq!(grenze_aus(Some("  4096  ".into())), 4096);
    // Ein Tippfehler legt den Dienst NICHT still: er faellt auf die Vorgabe
    // zurueck, nicht auf null.
    assert_eq!(grenze_aus(Some("viertausend".into())), DEFAULT_MAX_BODY_BYTES);
    assert_eq!(grenze_aus(Some("".into())), DEFAULT_MAX_BODY_BYTES);

    // Und die Verdrahtung: ein wirklich gesetzter Wert erreicht die Anwendung.
    std::env::set_var(GRENZE, "4096");
    let st = aufbauen_roh();
    assert_eq!(st.app.max_body_bytes, 4096, "die Umgebung wird gelesen");
    std::env::remove_var(GRENZE);
    let st = aufbauen_roh();
    assert_eq!(st.app.max_body_bytes, DEFAULT_MAX_BODY_BYTES);
}

#[test]
fn der_schalter_der_alten_wege_kommt_aus_der_umgebung() {
    let _allein = allein();
    assert!(alte_wege_aus(None), "ohne Angabe bleiben sie offen");
    for aus in ["off", "0", "no", "false", "OFF", "False"] {
        assert!(!alte_wege_aus(Some(aus.into())), "{aus} schliesst sie");
    }
    for an in ["on", "1", "yes", "irgendwas"] {
        assert!(alte_wege_aus(Some(an.into())), "{an} laesst sie offen");
    }

    std::env::set_var(PLAIN_WAYS, "off");
    let st = aufbauen_roh();
    assert!(!st.app.plain_ways, "die Umgebung wird gelesen");
    std::env::remove_var(PLAIN_WAYS);
    let st = aufbauen_roh();
    assert!(st.app.plain_ways);
}

// ----------------------------------------------------- der Bereich auf der Leitung

#[test]
fn ein_bereich_mit_kodiertem_doppelpunkt_findet_sein_eigenes_dokument() {
    // Eine Geraetekennung traegt einen Doppelpunkt, und ein Klient, der ihn
    // kodiert, muss sein eigenes Dokument wiederfinden.
    let mut st = aufbauen();
    let z = st.zeichen.clone();
    // Der Doppelpunkt ist das, was ein Klient kodiert; ein UMLAUT waere kein
    // gueltiger Bereich (der Vertrag laesst `[A-Za-z0-9._:-]` zu), und mein
    // erster Entwurf hat genau daran 400 bekommen. Der Zaun hatte recht,
    // nicht der Zeuge.
    assert_eq!(st.app.http(&schrieb(&z, "scope=geraet%3A1", br#"{"a": 1}"#)).status, 200);

    let gab = st.app.http(&Anfrage::neu("GET", "/v1/doc/playback")
        .mit_frage("scope=geraet%3A1")
        .mit_kopf("authorization", &format!("Bearer {z}")));
    assert_eq!(gab.status, 200, "derselbe kodierte Bereich findet es wieder");
    assert_eq!(gab.body, br#"{"a": 1}"#);

    // Und der UNKODIERTE Name meint dasselbe Dokument, denn entschluesselt
    // wird beim Lesen der Frage. Ohne das laege dasselbe Geraet unter zwei
    // Adressen.
    let roh = st.app.http(&Anfrage::neu("GET", "/v1/doc/playback")
        .mit_frage("scope=geraet:1")
        .mit_kopf("authorization", &format!("Bearer {z}")));
    assert_eq!(roh.status, 200, "kodiert und roh sind derselbe Bereich");
    assert_eq!(roh.body, br#"{"a": 1}"#);
}

// ---------------------------------------------------------------- der Handschlag

#[test]
fn ein_handschlag_fuer_einen_anderen_schluessel_wird_abgewiesen() {
    let mut st = aufbauen();
    let fremder = {
        let mut k = [0u8; 32];
        k.copy_from_slice(&zufall(32).unwrap());
        k
    };
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&fremder)), None)
        .expect("Rufer");
    let erst = rufer.write_message(b"").expect("erste Nachricht");
    let gab = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    assert_eq!(gab.status, 400);
    assert_eq!(gab.grund().as_deref(), Some("this is not a handshake"),
               "und er sagt nicht, WIE weit der Rufer gekommen ist");
}

#[test]
fn ein_leerer_koerper_ist_auch_kein_handschlag() {
    let mut st = aufbauen();
    let gab = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(b""));
    assert_eq!(gab.status, 400);
    assert_eq!(gab.grund().as_deref(), Some("this is not a handshake"));
}

// --------------------------------------------------------------- innen und aussen

#[test]
fn ein_zeichen_von_innen_wirkt_aussen_und_umgekehrt() {
    // Der Tunnel ist ein Weg, keine zweite Welt: was innen gilt, gilt aussen.
    let mut st = aufbauen();
    st.app.plain_ways = true;
    let privat = st.privat;

    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&privat)), None).unwrap();
    let erst = rufer.write_message(b"").unwrap();
    let auf = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    let sname = auf.kopf("x-maer-session").unwrap().to_string();
    rufer.read_message(&auf.body).unwrap();
    let (mut hin, mut her) = rufer.split();

    // Innen anmelden.
    let kopf = serde_json::json!({ "method": "POST", "target": "/v1/token" });
    let koerper = br#"{"owner": "anna", "secret": "ein hinreichend langes geheimnis"}"#;
    let roh = maer_sync::frames::pack(&kopf, koerper).unwrap();
    let gab = st.app.http(&Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(&hin.encrypt_with_ad(b"", &roh).unwrap()));
    let klar = her.decrypt_with_ad(b"", &gab.body).unwrap();
    let (_, innen) = maer_sync::frames::unpack(&klar).unwrap();
    let innen_zeichen = serde_json::from_slice::<serde_json::Value>(&innen).unwrap()
        ["token"].as_str().unwrap().to_string();

    // Und AUSSEN damit lesen.
    assert_eq!(st.app.http(&schrieb(&innen_zeichen, "scope=g1", br#"{"a": 1}"#)).status, 200,
               "das innen geholte Zeichen traegt aussen");

    // Umgekehrt: das aussen geholte Zeichen traegt innen.
    let z = st.zeichen.clone();
    let kopf2 = serde_json::json!({ "method": "GET",
        "target": "/v1/doc/playback?scope=g1",
        "headers": { "authorization": format!("Bearer {z}") } });
    let roh2 = maer_sync::frames::pack(&kopf2, b"").unwrap();
    let gab2 = st.app.http(&Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(&hin.encrypt_with_ad(b"", &roh2).unwrap()));
    let klar2 = her.decrypt_with_ad(b"", &gab2.body).unwrap();
    let (kopf_zurueck, _) = maer_sync::frames::unpack(&klar2).unwrap();
    assert_eq!(kopf_zurueck["status"].as_u64(), Some(200),
               "das aussen geholte Zeichen traegt innen");
    let _ = st.wo;
}

// ------------------------------------------------------------- die Sicherung

#[test]
fn die_konten_reisen_mit_der_kopie() {
    // Die Probe vergleicht DOKUMENTE; dass die Konten mitkommen, sagt sie
    // nicht. Ein zurueckgeholter Laden ohne Konten waere aber unbrauchbar:
    // niemand koennte sich anmelden, und die Dokumente laegen unerreichbar da.
    let mut st = aufbauen();
    let z = st.zeichen.clone();
    assert_eq!(st.app.http(&schrieb(&z, "scope=g1", br#"{"a": 1}"#)).status, 200);

    let kopie = std::env::temp_dir().join(format!("tore-kopie-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&kopie);
    let urteil = maer_sync::befehle::backup_and_rehearse(&st.app.store, kopie.to_str().unwrap());
    assert!(urteil.passed(), "die Probe muss durchgehen: {}", urteil.reason);

    let zurueck = Store::open(kopie.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None)
        .expect("die Kopie oeffnet als Laden");
    assert_eq!(zurueck.owner_of_token(&maer_sync::authorization::token_digest(&z)).unwrap(),
               Some("anna".to_string()),
               "das Zeichen und damit das Konto sind mitgereist");
    let _ = std::fs::remove_file(&kopie);
}
