// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 4: die Reihenfolge der Tore, und der Tunnel.
//!
//! Die Datei `vektoren/wege.json` ist nicht von Hand geschrieben. Sie ist an
//! der laufenden Python-Anwendung GEMESSEN: jeder Fall wurde gerufen und die
//! Antwort aufgeschrieben. Deshalb steht hier nichts, was ich über `app.py`
//! glaube, sondern nur, was sie tut.
//!
//! Die Fälle laufen der REIHE NACH gegen EINEN Laden, weil einige schreiben
//! und die späteren den Stand der früheren sehen. Wer sie umsortiert, misst
//! etwas anderes.

use std::collections::BTreeMap;

use maer_sync::authorization::zufall;
use maer_sync::frames::{pack, unpack};
use maer_sync::handshake::HandshakeState;
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::store::Store;
use maer_sync::wege::{Anfrage, App};
use serde_json::Value;

const BESITZER: &str = "anna";
const GEHEIMNIS: &str = "ein hinreichend langes geheimnis";

fn vektoren() -> Value {
    let t = std::fs::read_to_string("vektoren/wege.json").expect("vektoren/wege.json");
    serde_json::from_str(&t).expect("JSON")
}

struct Stand {
    app: App,
    zeichen: String,
    privat: [u8; 32],
}

/// Derselbe Ausgangszustand wie im Erzeuger: ein Konto, ein Zeichen, ein
/// Schriftstück.
///
/// Der Erzeuger hielt DREI Anwendungen über einem Laden. Hier ist es eine, und
/// die zwei Schalter werden je Fall gestellt: die drei unterschieden sich
/// genau in diesen zwei Feldern, und ein geteilter Laden ginge in Rust nur
/// über eine Hülle, die nichts beweist.
fn aufbauen() -> Stand {
    // Je Zeuge eine eigene Datei: die Zeugen laufen nebeneinander, und zwei
    // auf derselben SQLite-Datei messen einander statt die Sache.
    static ZAEHLER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir()
        .join(format!("wege-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None)
        .expect("Laden");
    laden.create_account(BESITZER, GEHEIMNIS).expect("Konto");
    let zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1").expect("Zeichen").expect("da");

    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).expect("Zufall"));
    let abdruck = fingerprint(&public_of(&privat)).expect("Abdruck");
    let mut app = App::neu(laden).mit_kennung(privat, &abdruck);
    app.plain_ways = true;

    // Derselbe erste Schrieb, BYTE-gleich mit dem des Erzeugers: Pythons
    // `json.dumps` setzt ein Leerzeichen hinter den Doppelpunkt, serde_json
    // nicht. Für die Fassungszählung ist das gleichgültig, für die Grösse
    // nicht, und die Grösse geht ins Schrumpf-Tor.
    let erst = Anfrage::neu("PUT", "/v1/doc/playback")
        .mit_frage("scope=geraet-1")
        .mit_kopf("authorization", &format!("Bearer {zeichen}"))
        .mit_kopf("x-maer-format", "1")
        .mit_koerper(b"{\"a\": 1}");
    let gab = app.http(&erst);
    assert_eq!(gab.status, 200, "der erste Schrieb muss durchgehen");

    Stand { app, zeichen, privat }
}

/// Die drei Anwendungen des Erzeugers als zwei Schalter. `kennung` wird
/// mitgegeben statt vergessen: `ohne_kennung` nimmt sie weg, und der naechste
/// Fall braucht sie zurueck.
fn stellen(app: &mut App, welche: &str, kennung: &([u8; 32], String)) {
    match welche {
        "ohne_kennung" => { app.identity = None; app.plain_ways = true; }
        "mit_kennung" => { app.identity = Some(kennung.clone()); app.plain_ways = true; }
        "geschlossen" => { app.identity = Some(kennung.clone()); app.plain_ways = false; }
        anderes => panic!("unbekannte Anwendung: {anderes}"),
    }
}

/// Prüft eine Antwort gegen einen `erwartet`-Block. Gibt zurück, was geprüft
/// wurde, damit ein Fall, der NICHTS prüft, auffällt statt still zu bestehen.
fn pruefen(name: &str, e: &Value, status: u16, kopf: &dyn Fn(&str) -> Option<String>,
           koerper: &[u8]) -> usize
{
    let mut geprueft = 0;
    assert_eq!(status as u64, e["status"].as_u64().unwrap(), "{name}: Status");
    geprueft += 1;

    if let Some(zeilen) = e.get("kopfzeilen").and_then(|v| v.as_object()) {
        for (k, v) in zeilen {
            assert_eq!(kopf(k).as_deref(), v.as_str(), "{name}: Kopfzeile {k}");
            geprueft += 1;
        }
    }
    if let Some(satz) = e.get("grund").and_then(|v| v.as_str()) {
        let gelesen: Value = serde_json::from_slice(koerper)
            .unwrap_or_else(|_| panic!("{name}: Körper ist kein JSON"));
        assert_eq!(gelesen["reason"].as_str(), Some(satz), "{name}: Satz");
        geprueft += 1;
    }
    if let Some(felder) = e.get("felder").and_then(|v| v.as_array()) {
        let gelesen: Value = serde_json::from_slice(koerper)
            .unwrap_or_else(|_| panic!("{name}: Körper ist kein JSON"));
        let mut haben: Vec<String> =
            gelesen.as_object().expect("Objekt").keys().cloned().collect();
        haben.sort();
        let wollen: Vec<String> =
            felder.iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert_eq!(haben, wollen, "{name}: Felder");
        geprueft += 1;
    }
    if let Some(wollen) = e.get("koerper") {
        let gelesen: Value = serde_json::from_slice(koerper)
            .unwrap_or_else(|_| panic!("{name}: Körper ist kein JSON"));
        assert_eq!(&gelesen, wollen, "{name}: Körper");
        geprueft += 1;
    }
    geprueft
}

/// Die Faelle der eingefrorenen Tafel, die Fassung 6 BEWUSST bewegt.
///
/// `vektoren/wege.json` ist die Aufzeichnung der Fassung 5 und bleibt, wie sie
/// gemessen wurde. Wo Fassung 6 anders antwortet, steht der Fall hier: mit der
/// NEUEN Antwort, die statt der aufgezeichneten zugesichert wird, und mit der
/// Stelle im Vertrag, die das verlangt. Ausgezaehlt am 24.09.2026, indem jeder
/// rote Fall einzeln herausgenommen wurde, bis der Zeuge gruen war: genau diese
/// zwoelf, und jeder andere Fall der Tafel haelt unter Fassung 6 unveraendert.
///
/// Die Liste sperrt in beide Richtungen (`die_ausnahmen_der_fassung_6_...`):
/// ein Name, den es in der Tafel nicht gibt, ist rot, und ein Fall, dessen
/// Aufzeichnung schon die neue Antwort traegt, auch; dann waere die Ausnahme
/// ohne Gegenstand.
struct Bewegt {
    name: &'static str,
    tunnel: bool,
    status: u16,
    innen_status: Option<u16>,
    satz: &'static str,
    grund: &'static str,
}

const KEY_404: &str = "CONTRACT.md: /v1/key answers 404 in version 6, on purpose (Riegel c)";
const NUR_INNEN: &str = "CONTRACT.md: the three sign-in paths speak only inside a session, 426 whatever the plain ways switch says";

const FASSUNG_6: &[Bewegt] = &[
    Bewegt { name: "Schluessel ohne Kennung", tunnel: false, status: 404, innen_status: None, satz: "no such path", grund: KEY_404 },
    Bewegt { name: "Schluessel mit Kennung", tunnel: false, status: 404, innen_status: None, satz: "no such path", grund: KEY_404 },
    Bewegt { name: "Schluessel mit falscher Methode", tunnel: false, status: 404, innen_status: None, satz: "no such path", grund: KEY_404 },
    Bewegt { name: "Schluessel auch bei geschlossenen Wegen", tunnel: false, status: 404, innen_status: None, satz: "no such path", grund: KEY_404 },
    Bewegt { name: "Zeichen mit falscher Methode", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "Zeichen ohne JSON", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "Zeichen ohne Geheimnis", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "Zeichen, falsches Geheimnis", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "Zeichen, Konto gibt es nicht", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "Zeichen, richtig", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "geschlossen: das Zeichen selbst ist hinter dem Tor", tunnel: false, status: 426, innen_status: None, satz: "the sign-in paths speak only inside a session", grund: NUR_INNEN },
    Bewegt { name: "innen: der Schluessel wird innen nicht beantwortet", tunnel: true, status: 200, innen_status: Some(404), satz: "no such path",
             grund: "CONTRACT.md: inside, only /v1/session and /v2/key are not answered; /v1/key is no path in version 6 and answers the 404 of an unknown path, sealed" },
];

fn bewegt(name: &str) -> Option<&'static Bewegt> {
    FASSUNG_6.iter().find(|b| b.name == name)
}

#[test]
fn jeder_gemessene_weg_antwortet_genauso() {
    let v = vektoren();
    let faelle = v["faelle"].as_array().expect("faelle");
    assert!(faelle.len() >= 38, "die Vektordatei ist geschrumpft: {}", faelle.len());
    let mut st = aufbauen();
    let kennung = st.app.identity.clone().expect("Kennung");
    let mut geprueft = 0;
    let mut angewandt = 0;

    for fall in faelle {
        let name = fall["name"].as_str().unwrap();
        stellen(&mut st.app, fall["anwendung"].as_str().unwrap(), &kennung);

        let an = &fall["anfrage"];
        let mut a = Anfrage::neu(an["method"].as_str().unwrap(), an["path"].as_str().unwrap());
        if let Some(q) = an.get("query").and_then(|x| x.as_str()) {
            a = a.mit_frage(q);
        }
        if let Some(h) = an.get("headers").and_then(|x| x.as_object()) {
            for (k, val) in h {
                let val = val.as_str().unwrap();
                // Das echte Zeichen steht in der Datei NIE. Nur DER Wert ist
                // maskiert, nicht jede `authorization`-Zeile: "Bearer
                // nichtvergeben" und "Basic irgendwas" tragen kein Geheimnis,
                // und sie alle gleich zu maskieren machte aus drei
                // verschiedenen Faellen einen.
                let echt = if val == "<zeichen>" {
                    format!("Bearer {}", st.zeichen)
                } else {
                    val.to_string()
                };
                a = a.mit_kopf(k, &echt);
            }
        }
        if let Some(b) = an.get("koerper_b64").and_then(|x| x.as_str()) {
            a = a.mit_koerper(&b64_lesen(b));
        }
        if an.get("innen").and_then(|x| x.as_bool()).unwrap_or(false) {
            a = a.von_innen();
        }

        let gab = st.app.http(&a);
        if let Some(b) = bewegt(name) {
            assert!(!b.tunnel, "{name}: steht als Tunnelfall in der Liste");
            assert_eq!(gab.status, b.status, "{name}: Status nach Fassung 6 ({})", b.grund);
            assert_eq!(gab.grund().as_deref(), Some(b.satz), "{name}: Satz nach Fassung 6 ({})", b.grund);
            assert!(gab.ansagen.is_empty(), "{name}: sagt an");
            angewandt += 1;
            geprueft += 1;
            continue;
        }
        let kopf = |k: &str| gab.kopf(k).map(|s| s.to_string());
        geprueft += pruefen(name, &fall["erwartet"], gab.status, &kopf, &gab.body);

        // Die Ansage: steht sie in der Datei, muss sie genauso herauskommen;
        // steht sie NICHT da, darf keine entstehen.
        let gewollt = fall.get("ansage").and_then(|x| x.as_array());
        match gewollt {
            Some(liste) => {
                assert_eq!(gab.ansagen.len(), liste.len(), "{name}: Zahl der Ansagen");
                for (hat, will) in gab.ansagen.iter().zip(liste) {
                    let gelesen: Value = serde_json::from_str(&hat.text).expect("Ansage ist JSON");
                    assert_eq!(&gelesen, will, "{name}: Ansage");
                    assert_eq!(hat.owner, BESITZER, "{name}: Empfänger der Ansage");
                }
                geprueft += 1;
            }
            None => assert!(gab.ansagen.is_empty(),
                            "{name}: sagt an, obwohl der Erzeuger nichts gehört hat"),
        }
    }
    assert!(geprueft >= faelle.len(), "zu wenig geprüft: {geprueft}");
    assert_eq!(angewandt, FASSUNG_6.iter().filter(|b| !b.tunnel).count(),
               "jede Ausnahme der Wegefaelle muss genau einmal gegriffen haben");
}

#[test]
fn der_tunnel_traegt_dieselben_antworten() {
    let v = vektoren();
    let tunnel = v["tunnel"].as_array().expect("tunnel");
    assert!(tunnel.len() >= 11, "die Tunnelliste ist geschrumpft: {}", tunnel.len());
    let mut st = aufbauen();
    st.app.plain_ways = false;

    // Ein echter Handschlag, von der Client-Seite gefahren, wie im Erzeuger.
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&st.privat)), None)
        .expect("Rufer");
    let erst = rufer.write_message(b"").expect("erste Nachricht");
    let auf = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    assert_eq!(auf.status, 200, "der Handschlag muss angenommen werden");
    let sname = auf.kopf("x-maer-session").expect("Sitzungsname").to_string();
    rufer.read_message(&auf.body).expect("Antwort");
    let (mut hin, mut her) = rufer.split();
    let mut angewandt = 0;

    for fall in tunnel {
        let name = fall["name"].as_str().unwrap();
        let aussen = &fall["aussen"];

        // Vier Gestalten, und WELCHE steht in der Datei, nicht im Namen.
        let gesendet = match fall["gestalt"].as_str().expect("gestalt") {
            "rahmen" => {
                let (kopf, koerper) = rahmen_aus(fall, &st.zeichen);
                hin.encrypt_with_ad(b"", &pack(&kopf, &koerper).expect("packen")).expect("siegeln")
            }
            "versiegelter_unsinn" =>
                hin.encrypt_with_ad(b"", b"kein rahmen").expect("siegeln"),
            "kein_siegel" => b"kein siegel".to_vec(),
            "fremde_sitzung" => b"egal".to_vec(),
            andere => panic!("{name}: unbekannte Gestalt {andere}"),
        };
        let an_wen = fall.get("fremder_name").and_then(|x| x.as_str())
            .map(|s| s.to_string()).unwrap_or_else(|| sname.clone());

        let gab = st.app.http(&Anfrage::neu("POST", "/v1/session")
            .mit_kopf("x-maer-session", &an_wen)
            .mit_koerper(&gesendet));

        if let Some(b) = bewegt(name) {
            assert!(b.tunnel, "{name}: steht als Wegefall in der Liste");
            assert_eq!(gab.status, b.status, "{name}: Status aussen nach Fassung 6 ({})", b.grund);
            let klar = her.decrypt_with_ad(b"", &gab.body).expect("entsiegeln");
            let (kopf, koerper) = unpack(&klar).expect("Rahmen");
            assert_eq!(kopf["status"].as_u64(), b.innen_status.map(u64::from),
                       "{name}: Status innen nach Fassung 6 ({})", b.grund);
            let gelesen: Value = serde_json::from_slice(&koerper).expect("JSON innen");
            assert_eq!(gelesen["reason"].as_str(), Some(b.satz), "{name}: Satz innen nach Fassung 6");
            assert!(gab.ansagen.is_empty(), "{name}: sagt an");
            angewandt += 1;
            continue;
        }
        assert_eq!(gab.status as u64, aussen["status"].as_u64().unwrap(), "{name}: Status aussen");
        if let Some(satz) = aussen.get("grund").and_then(|x| x.as_str()) {
            assert_eq!(gab.grund().as_deref(), Some(satz), "{name}: Satz aussen");
        }

        if let Some(innen) = fall.get("innen") {
            let klar = her.decrypt_with_ad(b"", &gab.body).expect("entsiegeln");
            let (kopf, koerper) = unpack(&klar).expect("Rahmen");
            let status = kopf["status"].as_u64().unwrap() as u16;
            let zeilen: BTreeMap<String, String> = kopf["headers"].as_object()
                .map(|o| o.iter().map(|(k, val)| (k.clone(), val.as_str().unwrap().to_string()))
                    .collect())
                .unwrap_or_default();
            let hol = |k: &str| zeilen.get(k).cloned();
            pruefen(name, innen, status, &hol, &koerper);
        }

        match fall.get("ansage").and_then(|x| x.as_array()) {
            Some(liste) => {
                assert_eq!(gab.ansagen.len(), liste.len(), "{name}: Zahl der Ansagen im Tunnel");
                for (hat, will) in gab.ansagen.iter().zip(liste) {
                    let gelesen: Value = serde_json::from_str(&hat.text).expect("JSON");
                    assert_eq!(&gelesen, will, "{name}: Ansage im Tunnel");
                }
            }
            None => assert!(gab.ansagen.is_empty(), "{name}: sagt an, obwohl nichts gehört wurde"),
        }
    }
    assert_eq!(angewandt, FASSUNG_6.iter().filter(|b| b.tunnel).count(),
               "jede Ausnahme der Tunnelfaelle muss genau einmal gegriffen haben");
}

/// Die Gegenrichtung der Ausnahmen: jede nennt einen Fall, den es in der
/// eingefrorenen Tafel GIBT, genau einmal, an der richtigen Stelle, mit einem
/// Grund, und die Aufzeichnung sagt dort etwas ANDERES als die neue Antwort.
#[test]
fn die_ausnahmen_der_fassung_6_haben_je_einen_gegenstand() {
    let v = vektoren();
    for b in FASSUNG_6 {
        assert!(!b.grund.trim().is_empty(), "{}: ohne Grund", b.name);
        let liste = if b.tunnel { "tunnel" } else { "faelle" };
        let treffer: Vec<&Value> = v[liste].as_array().unwrap().iter()
            .filter(|f| f["name"].as_str() == Some(b.name)).collect();
        assert_eq!(treffer.len(), 1, "{}: steht {} mal in {liste}", b.name, treffer.len());
        let alt = if b.tunnel { &treffer[0]["aussen"] } else { &treffer[0]["erwartet"] };
        let alt_status = alt["status"].as_u64();
        let alt_satz = alt.get("grund").and_then(|x| x.as_str());
        assert!(alt_status != Some(b.status as u64) || alt_satz != Some(b.satz) || b.innen_status.is_some(),
                "{}: die Aufzeichnung traegt schon die neue Antwort, die Ausnahme hat keinen Gegenstand", b.name);
    }
    assert_eq!(FASSUNG_6.len(), 12, "ausgezaehlt waren zwoelf; eine neue Ausnahme braucht eine neue Zaehlung");
}

/// Eine Weigerung der ÄUSSEREN Schicht reist unversiegelt. Ein eigener Zeuge,
/// weil das die Stelle ist, an der eine zweite Umsetzung aus Gewohnheit
/// versiegelt: die Anfrage kam versiegelt an, also läge es nahe. Sie ist aber
/// keine Antwort auf die innere Anfrage, sondern darauf, dass es keine gibt.
#[test]
fn eine_weigerung_der_aeusseren_schicht_reist_unversiegelt() {
    let mut st = aufbauen();
    st.app.plain_ways = false;
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&st.privat)), None).unwrap();
    let erst = rufer.write_message(b"").unwrap();
    let auf = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    let sname = auf.kopf("x-maer-session").unwrap().to_string();
    rufer.read_message(&auf.body).unwrap();
    let (mut hin, _her) = rufer.split();

    let roh = pack(&serde_json::json!({ "method": "POST", "target": "/v1/session" }), b"").unwrap();
    let gab = st.app.http(&Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(&hin.encrypt_with_ad(b"", &roh).unwrap()));

    assert_eq!(gab.status, 400);
    assert_eq!(gab.grund().as_deref(), Some("this path is not answered inside a session"));
    assert_eq!(gab.kopf("content-type"), Some("application/json"),
               "unversiegelt heisst: als JSON lesbar, ohne den Sitzungsschluessel");
}

/// Die Sitzung überlebt Rauschen unter ihrem Namen, und der Zähler rückt
/// nicht vor. Ohne diesen Zeugen könnte ein Fremder, der einen Namen kennt,
/// die Sitzung ihres Besitzers unbrauchbar machen, indem er einmal Unsinn
/// schickt.
#[test]
fn rauschen_unter_einem_namen_kostet_die_sitzung_nichts() {
    let mut st = aufbauen();
    st.app.plain_ways = false;
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&st.privat)), None).unwrap();
    let erst = rufer.write_message(b"").unwrap();
    let auf = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    let sname = auf.kopf("x-maer-session").unwrap().to_string();
    rufer.read_message(&auf.body).unwrap();
    let (mut hin, mut her) = rufer.split();

    let unsinn = st.app.http(&Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(b"das ist kein siegel"));
    assert_eq!(unsinn.status, 400);
    assert_eq!(unsinn.grund().as_deref(), Some("this is not a sealed message"));

    // Und danach trägt dieselbe Sitzung eine echte Anfrage, mit dem Zähler,
    // den sie vorher hatte.
    let roh = pack(&serde_json::json!({ "method": "GET", "target": "/v1/health" }), b"").unwrap();
    let gab = st.app.http(&Anfrage::neu("POST", "/v1/session")
        .mit_kopf("x-maer-session", &sname)
        .mit_koerper(&hin.encrypt_with_ad(b"", &roh).unwrap()));
    assert_eq!(gab.status, 200, "die Sitzung hat das Rauschen überlebt");
    let klar = her.decrypt_with_ad(b"", &gab.body).expect("entsiegeln");
    let (kopf, _) = unpack(&klar).unwrap();
    assert_eq!(kopf["status"].as_u64(), Some(200));
}

fn rahmen_aus(fall: &Value, zeichen: &str) -> (Value, Vec<u8>) {
    let kopf = fall["rahmen"].clone();
    let koerper = fall.get("rahmen_koerper_b64").and_then(|x| x.as_str())
        .map(b64_lesen).unwrap_or_default();
    (ersetze_zeichen(kopf, zeichen), koerper)
}

fn ersetze_zeichen(mut kopf: Value, zeichen: &str) -> Value {
    if let Some(h) = kopf.get_mut("headers").and_then(|x| x.as_object_mut()) {
        if let Some(a) = h.get_mut("authorization") {
            if a.as_str() == Some("<zeichen>") {
                *a = Value::String(format!("Bearer {zeichen}"));
            }
        }
    }
    kopf
}

fn b64_lesen(s: &str) -> Vec<u8> {
    const TAFEL: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits: Vec<u8> = Vec::new();
    let mut puffer: u32 = 0;
    let mut wie_viel = 0;
    for c in s.bytes() {
        if c == b'=' { break; }
        let Some(i) = TAFEL.iter().position(|&t| t == c) else { continue };
        puffer = (puffer << 6) | i as u32;
        wie_viel += 6;
        if wie_viel >= 8 {
            wie_viel -= 8;
            bits.push((puffer >> wie_viel) as u8);
        }
    }
    bits
}
