// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Fassung 6, ein Geheimnis: jede Zusage des Vertrags, die der Dienst traegt.
//!
//! Die Faelle folgen dem Vertrag, nicht dem Code: je Absatz, der eine Antwort
//! verspricht, mindestens ein Zeuge. Wo der Dienst eine Zusage NICHT pruefen
//! kann (die Normalisierung eines Passworts, das er nie sieht), steht das als
//! Enthaltung mit Grund da, nicht als gruener Fall.

use std::sync::{Arc, Mutex};

use maer_sync::authorization::{
    hash_proof, proof_matches, zufall, EINMAL_ALPHABET, EINMAL_LAENGE, MIN_ROUNDS, SALT_BYTES,
};
use maer_sync::frames::{pack, unpack};
use maer_sync::handshake::HandshakeState;
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::store::Store;
use maer_sync::versuche::{adresse_aus, vertraute_aus, Versuche, FENSTER_S, GRENZE};
use maer_sync::wege::{base64, base64_lesen, Anfrage, Antwort, App};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const BESITZER: &str = "anna";
const ALT: &str = "ein hinreichend langes geheimnis";
const QUELLE: &str = "203.0.113.7:40000";

// ------------------------------------------------------------ Aufbau

fn laden() -> Store {
    static ZAEHLER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir().join(format!("ein-geheimnis-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None).expect("Laden")
}

struct Stand {
    app: App,
    uhr: Arc<Mutex<f64>>,
    privat: [u8; 32],
}

/// Ein Konto in der Form von Fassung 5, ein Dienst mit Kennung und eine Uhr,
/// die der Zeuge stellt.
fn aufbauen() -> Stand {
    let mut l = laden();
    l.create_account(BESITZER, ALT).expect("Konto");
    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).expect("Zufall"));
    let abdruck = fingerprint(&public_of(&privat)).expect("Abdruck");
    let uhr = Arc::new(Mutex::new(1_000_000.0));
    let u = Arc::clone(&uhr);
    let mut app = App::neu(l).mit_kennung(privat, &abdruck)
        .mit_uhr(Box::new(move || *u.lock().unwrap()));
    app.plain_ways = true;
    Stand { app, uhr, privat }
}

/// Eine Anfrage, die eine Sitzung entsiegelt hat, von einer Adresse.
fn innen(method: &str, pfad: &str, koerper: Value) -> Anfrage {
    let mut a = Anfrage::neu(method, pfad).mit_koerper(koerper.to_string().as_bytes()).von_innen();
    a.client = Some(QUELLE.into());
    a
}

fn salz_fragen(app: &mut App, owner: &str) -> Value {
    let mut a = Anfrage::neu("GET", "/v1/salt").mit_frage(&format!("owner={owner}")).von_innen();
    a.client = Some(QUELLE.into());
    let gab = app.http(&a);
    assert_eq!(gab.status, 200, "das Salz antwortet fuer jeden Namen");
    gab.als_json().expect("JSON")
}

/// Ein neues Passwort als Beweis: 32 Byte, die hier fuer das stehen, was das
/// Geraet aus einem Passwort ableitet. Der Dienst sieht nie mehr als das.
fn beweis(n: u8) -> Vec<u8> { vec![n; 32] }

fn neu_json(salz: &[u8], b: &[u8]) -> Value {
    json!({ "proof": base64(b), "salt": base64(salz), "kdf": "pbkdf2-hmac-sha256", "rounds": 600000 })
}

fn umziehen(app: &mut App, b: &[u8]) -> Antwort {
    app.http(&innen("POST", "/v1/password", json!({
        "owner": BESITZER, "device": "geraet-neu",
        "current": { "secret": ALT }, "new": neu_json(&[7u8; 16], b) })))
}

fn anmelden_beweis(app: &mut App, b: &[u8]) -> Antwort {
    app.http(&innen("POST", "/v1/token",
                    json!({ "owner": BESITZER, "proof": base64(b), "device": "geraet-neu" })))
}

fn zeichen_aus(gab: &Antwort) -> String {
    gab.als_json().and_then(|j| j["token"].as_str().map(|s| s.to_string())).expect("ein Zeichen")
}

fn lesen_mit(app: &mut App, zeichen: &str) -> Antwort {
    app.http(&Anfrage::neu("GET", "/v1/doc/playback").mit_frage("scope=g1")
        .mit_kopf("authorization", &format!("Bearer {zeichen}")))
}

// ------------------------------------------------ Die Vektoren

const VEKTOR_DIGEST: &str = "1b796ce806b708a7f9ce07b5c06f6696395915d294d9136c00872d338c410cb9";

fn vektoren() -> Value {
    let roh = std::fs::read("vektoren/sign-in-v6.json").expect("vektoren/sign-in-v6.json");
    let d: String = Sha256::digest(&roh).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(d, VEKTOR_DIGEST, "die Vektoren sind nicht die, die beide Seiten vereinbart haben");
    serde_json::from_slice(&roh).expect("JSON")
}

fn hex(s: &str) -> Vec<u8> {
    maer_sync::authorization::hex_lesen(s).expect("hex")
}

/// PBKDF2-HMAC-SHA256 nach RFC 8018, NUR fuer diesen Zeugen.
///
/// Der Dienst rechnet PBKDF2 im Betrieb nie; hier prueft er nur, dass die
/// Vektoren stimmen, die beide Seiten teilen. Die HMAC kommt aus `hkdf`, das
/// sie weiterreicht: keine neue Zeile in `Cargo.toml`. Geeicht gegen RFC 7914,
/// Abschnitt 11, damit ein Fehler hier nicht wie ein Fehler der Vektoren
/// aussieht.
fn pbkdf2(passwort: &[u8], salz: &[u8], runden: u32, laenge: usize) -> Vec<u8> {
    use hkdf::hmac::{Hmac, KeyInit, Mac};
    let mac = || <Hmac<Sha256> as KeyInit>::new_from_slice(passwort).expect("jede Laenge geht");
    let mut aus = Vec::new();
    let mut block: u32 = 1;
    while aus.len() < laenge {
        let mut m = mac();
        m.update(salz);
        m.update(&block.to_be_bytes());
        let mut u = m.finalize().into_bytes().to_vec();
        let mut t = u.clone();
        for _ in 1..runden {
            let mut m = mac();
            m.update(&u);
            u = m.finalize().into_bytes().to_vec();
            for (a, b) in t.iter_mut().zip(&u) { *a ^= b; }
        }
        aus.extend_from_slice(&t);
        block += 1;
    }
    aus.truncate(laenge);
    aus
}

fn hkdf_info(m: &[u8], info: &str) -> Vec<u8> {
    let hk = hkdf::Hkdf::<Sha256>::new(None, m);
    let mut aus = vec![0u8; 32];
    hk.expand(info.as_bytes(), &mut aus).expect("32 Byte");
    aus
}

#[test]
fn das_eigene_pbkdf2_des_zeugen_ist_geeicht() {
    // RFC 7914, Abschnitt 11: PBKDF2-HMAC-SHA256, P="passwd", S="salt", c=1.
    let will = hex("55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc\
49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783");
    assert_eq!(pbkdf2(b"passwd", b"salt", 1, 64), will,
               "das PBKDF2 dieses Zeugen rechnet falsch; seine Vektor-Urteile gaelten nichts");
}

#[test]
fn die_gemeinsamen_vektoren_stimmen_soweit_der_dienst_sie_rechnen_kann() {
    let v = vektoren();
    assert_eq!(v["kdf"], "pbkdf2-hmac-sha256");
    let liste = v["vectors"].as_array().expect("vectors");
    assert!(liste.len() >= 3, "nur {} Vektoren", liste.len());
    let (mut gedehnt_geprueft, mut enthalten) = (0, 0);
    for f in liste {
        let name = f["name"].as_str().unwrap();
        let m = hex(f["stretched_hex"].as_str().unwrap());
        let sign_in = hex(f["sign_in_hex"].as_str().unwrap());
        let feed = hex(f["feed_hex"].as_str().unwrap());
        assert_eq!(f["rounds"].as_u64(), Some(MIN_ROUNDS), "{name}: Runden");

        // Was der Dienst selbst rechnen kann: die beiden Ableitungen aus M,
        // und dass `proof` auf der Leitung genau `sign_in` ist.
        assert_eq!(hkdf_info(&m, "maer sign in v1"), sign_in, "{name}: sign_in aus M");
        assert_eq!(hkdf_info(&m, "maer feed auth v2"), feed, "{name}: feed aus M");
        assert_ne!(sign_in, feed, "{name}: Anmelden und Feed muessen verschieden sein");
        assert_eq!(base64(&sign_in), f["proof_base64"].as_str().unwrap(), "{name}: base64");
        assert_eq!(base64_lesen(f["proof_base64"].as_str().unwrap()).unwrap(), sign_in);

        // Und was er daraus speichert: der Beweis passt zu seiner Form, ein
        // Bit daneben nicht.
        let form = hash_proof(&sign_in, &[3u8; SALT_BYTES]).expect("Form");
        assert!(proof_matches(&form, &sign_in), "{name}: der gespeicherte Beweis passt nicht");
        let mut daneben = sign_in.clone();
        daneben[0] ^= 1;
        assert!(!proof_matches(&form, &daneben), "{name}: ein Bit daneben passt trotzdem");

        // P, S -> M. Die Normalisierung nach NFC kann der Dienst nicht
        // rechnen, er sieht nie ein Passwort. Fuer eine Eingabe aus reinem
        // ASCII ist NFC die Eingabe selbst; fuer jede andere ENTHAELT sich
        // dieser Zeuge, statt die Rohbytes zu dehnen und das "gruen" zu nennen.
        let roh = hex(f["password_utf8_hex"].as_str().unwrap());
        if roh.is_ascii() {
            let salz = hex(f["salt_hex"].as_str().unwrap());
            assert_eq!(pbkdf2(&roh, &salz, MIN_ROUNDS as u32, 32), m, "{name}: M aus P und S");
            gedehnt_geprueft += 1;
        } else {
            println!("enthalten: {name}: die Eingabe ist nicht ASCII, und NFC rechnet der Dienst nicht");
            enthalten += 1;
        }
    }
    // Der Loch-Melder: eine Enthaltung darf nie alles sein.
    assert!(gedehnt_geprueft >= 2, "nur {gedehnt_geprueft} Vektoren gedehnt, {enthalten} enthalten");
}

// ------------------------------------------------ Die Wege der Fassung 6

#[test]
fn der_alte_schluesselweg_antwortet_404_fuer_jede_methode() {
    let mut st = aufbauen();
    for m in ["GET", "POST", "PUT"] {
        let gab = st.app.http(&Anfrage::neu(m, "/v1/key"));
        assert_eq!(gab.status, 404, "{m} /v1/key");
        assert_eq!(gab.grund().as_deref(), Some("no such path"));
    }
    // Auch mit geschlossenen alten Wegen: das 404 steht VOR diesem Tor, sonst
    // bekaeme eine App der Fassung 5 eine 426 und liefe weiter.
    st.app.plain_ways = false;
    assert_eq!(st.app.http(&Anfrage::neu("GET", "/v1/key")).status, 404);
}

#[test]
fn der_neue_schluesselweg_nennt_die_fassung() {
    let mut st = aufbauen();
    let gab = st.app.http(&Anfrage::neu("GET", "/v2/key"));
    assert_eq!(gab.status, 200);
    // Version 7 since 25.11.2026: the catalog paths.
    assert_eq!(gab.kopf("x-maer-contract"), Some("7"));
    assert_eq!(gab.kopf("x-maer-contract"), Some(maer_sync::contract::VERSION));
    let j = gab.als_json().unwrap();
    assert!(j["key"].is_string() && j["fingerprint"].is_string());
    assert_eq!(st.app.http(&Anfrage::neu("POST", "/v2/key")).status, 405);
}

#[test]
fn die_drei_anmeldewege_sprechen_nur_innen_was_immer_der_schalter_sagt() {
    let mut st = aufbauen();
    assert!(st.app.plain_ways, "Vorbedingung: die alten Wege sind AN");
    for (m, p) in [("POST", "/v1/token"), ("GET", "/v1/salt"), ("POST", "/v1/password")] {
        let gab = st.app.http(&Anfrage::neu(m, p).mit_koerper(
            json!({ "owner": BESITZER, "secret": ALT }).to_string().as_bytes()));
        assert_eq!(gab.status, 426, "{p} von aussen");
        assert_eq!(gab.grund().as_deref(), Some("the sign-in paths speak only inside a session"));
    }
}

#[test]
fn das_salz_hat_fuer_jeden_namen_dieselbe_gestalt_und_bleibt_stehen() {
    let mut st = aufbauen();
    let fremd1 = salz_fragen(&mut st.app, "niemand");
    let fremd2 = salz_fragen(&mut st.app, "niemand");
    let fremd3 = salz_fragen(&mut st.app, "auchniemand");
    assert_eq!(fremd1, fremd2, "dasselbe Schein-Salz bei jeder Frage");
    assert_ne!(fremd1["salt"], fremd3["salt"], "zwei Namen, zwei Schein-Salze");
    assert_eq!(base64_lesen(fremd1["salt"].as_str().unwrap()).unwrap().len(), 16);
    assert_eq!(fremd1["kdf"], "pbkdf2-hmac-sha256");
    assert_eq!(fremd1["rounds"], 600000);
    assert_eq!(fremd1["form"], "proof", "ein unbekannter Name sieht aus wie ein umgezogenes Konto");

    // Das Konto in der Form von Fassung 5 ist die eine Unterscheidung.
    assert_eq!(salz_fragen(&mut st.app, BESITZER)["form"], "secret");

    // Nach dem Umzug: das Salz, das das Geraet gewaehlt hat, und dieselben
    // Schluessel wie ein unbekannter Name.
    assert_eq!(umziehen(&mut st.app, &beweis(1)).status, 200);
    let echt = salz_fragen(&mut st.app, BESITZER);
    assert_eq!(echt["salt"], base64(&[7u8; 16]));
    assert_eq!(echt["form"], "proof");
    let schluessel = |v: &Value| { let mut k: Vec<_> = v.as_object().unwrap().keys().cloned().collect(); k.sort(); k };
    assert_eq!(schluessel(&echt), schluessel(&fremd1));

    // Und das Schein-Salz ueberlebt einen Neustart: es steht im Laden.
    let wieder = salz_fragen(&mut st.app, "niemand");
    assert_eq!(wieder, fremd1);
}

#[test]
fn ohne_owner_ist_das_salz_eine_400() {
    let mut st = aufbauen();
    let gab = st.app.http(&Anfrage::neu("GET", "/v1/salt").von_innen());
    assert_eq!(gab.status, 400);
}

// ------------------------------------------------ Umzug, Anmeldung, Wechsel

#[test]
fn der_umzug_setzt_den_beweis_und_zieht_jedes_andere_zeichen_zurueck() {
    let mut st = aufbauen();
    let alt_zeichen = st.app.store.issue_token(BESITZER, ALT, "geraet-alt").unwrap().expect("Zeichen");
    assert_ne!(lesen_mit(&mut st.app, &alt_zeichen).status, 401, "Vorbedingung: das alte Zeichen gilt");

    let gab = umziehen(&mut st.app, &beweis(1));
    assert_eq!(gab.status, 200, "{:?}", gab.grund());
    let neu_zeichen = zeichen_aus(&gab);

    // Das alte Zeichen sagt, WARUM es nicht mehr gilt.
    let alt = lesen_mit(&mut st.app, &alt_zeichen);
    assert_eq!(alt.status, 401);
    assert_eq!(alt.kopf("x-maer-refusal"), Some("password-changed"));
    // Ein unbekanntes Zeichen sagt das NICHT.
    let fremd = lesen_mit(&mut st.app, "unsinnszeichen");
    assert_eq!(fremd.status, 401);
    assert_eq!(fremd.kopf("x-maer-refusal"), None);
    // Das neue Zeichen gilt (kein Dokument da: 404, aber nicht 401).
    assert_eq!(lesen_mit(&mut st.app, &neu_zeichen).status, 404);

    // Danach meldet nur noch der Beweis an.
    assert_eq!(anmelden_beweis(&mut st.app, &beweis(1)).status, 200);
    assert_eq!(anmelden_beweis(&mut st.app, &beweis(2)).status, 401);
    let alt_form = st.app.http(&innen("POST", "/v1/token",
        json!({ "owner": BESITZER, "secret": ALT, "device": "x" })));
    assert_eq!(alt_form.status, 401, "das alte Passwort meldet ein umgezogenes Konto nicht mehr an");
    // Und ein zweiter Umzug mit dem alten Passwort ist keiner mehr.
    assert_eq!(umziehen(&mut st.app, &beweis(3)).status, 401);

    let k = st.app.store.konto(BESITZER).unwrap().unwrap();
    assert_eq!(k.form, "proof");
    assert!(k.umgezogen.is_some());
    let formen: Vec<String> = st.app.store.geraete(BESITZER).unwrap().into_iter()
        .map(|(g, f, _)| format!("{g}={f}")).collect();
    assert!(formen.contains(&"geraet-neu=proof".to_string()), "{formen:?}");
    assert!(formen.contains(&"geraet-alt=secret".to_string()), "{formen:?}");
}

#[test]
fn ein_wechsel_mit_dem_beweis_zieht_auch_die_zeichen_nach_dem_umzug_zurueck() {
    let mut st = aufbauen();
    let erst = zeichen_aus(&umziehen(&mut st.app, &beweis(1)));
    let zweit = zeichen_aus(&anmelden_beweis(&mut st.app, &beweis(1)));
    let gab = st.app.http(&innen("POST", "/v1/password", json!({
        "owner": BESITZER, "device": "geraet-w",
        "current": { "proof": base64(&beweis(1)) }, "new": neu_json(&[9u8; 16], &beweis(4)) })));
    assert_eq!(gab.status, 200, "{:?}", gab.grund());
    for z in [&erst, &zweit] {
        let a = lesen_mit(&mut st.app, z);
        assert_eq!((a.status, a.kopf("x-maer-refusal")), (401, Some("password-changed")));
    }
    assert_eq!(anmelden_beweis(&mut st.app, &beweis(1)).status, 401, "der alte Beweis gilt nicht mehr");
    assert_eq!(anmelden_beweis(&mut st.app, &beweis(4)).status, 200);
    assert_eq!(salz_fragen(&mut st.app, BESITZER)["salt"], base64(&[9u8; 16]));
}

#[test]
fn ein_falsches_current_ist_401_und_aendert_nichts() {
    let mut st = aufbauen();
    let gab = st.app.http(&innen("POST", "/v1/password", json!({
        "owner": BESITZER, "device": "d", "current": { "secret": "falsch" },
        "new": neu_json(&[1u8; 16], &beweis(1)) })));
    assert_eq!(gab.status, 401);
    assert_eq!(st.app.store.konto(BESITZER).unwrap().unwrap().form, "secret");
    // Ein Beweis an ein Konto, das noch nicht umgezogen ist: auch 401.
    let gab = st.app.http(&innen("POST", "/v1/password", json!({
        "owner": BESITZER, "device": "d", "current": { "proof": base64(&beweis(1)) },
        "new": neu_json(&[1u8; 16], &beweis(1)) })));
    assert_eq!(gab.status, 401);
    // Und ein unbekanntes Konto.
    let gab = st.app.http(&innen("POST", "/v1/password", json!({
        "owner": "niemand", "device": "d", "current": { "secret": ALT },
        "new": neu_json(&[1u8; 16], &beweis(1)) })));
    assert_eq!(gab.status, 401);
}

#[test]
fn ein_unlesbares_new_ist_400_bevor_irgendetwas_geprueft_wird() {
    let mut st = aufbauen();
    let gut = neu_json(&[1u8; 16], &beweis(1));
    let mut faelle = Vec::new();
    let mut f = gut.clone(); f["salt"] = json!(base64(&[1u8; 15])); faelle.push(("Salz 15 Byte", f));
    let mut f = gut.clone(); f["rounds"] = json!(599999); faelle.push(("Runden unter 600000", f));
    let mut f = gut.clone(); f["proof"] = json!(base64(&[1u8; 31])); faelle.push(("Beweis 31 Byte", f));
    let mut f = gut.clone(); f["kdf"] = json!("argon2id"); faelle.push(("anderes Verfahren", f));
    let mut f = gut.clone(); f.as_object_mut().unwrap().remove("salt"); faelle.push(("ohne Salz", f));
    for (name, neu) in faelle {
        let gab = st.app.http(&innen("POST", "/v1/password", json!({
            "owner": BESITZER, "device": "d", "current": { "secret": ALT }, "new": neu })));
        assert_eq!(gab.status, 400, "{name}");
    }
    // Keiner davon hat das Konto angefasst, und keiner war ein Fehlversuch.
    assert_eq!(st.app.store.konto(BESITZER).unwrap().unwrap().form, "secret");
    assert_eq!(st.app.versuche.anzahl(), 0);
}

// ------------------------------------------------ Das Einmal-Passwort

#[test]
fn das_einmal_passwort_taugt_nur_zum_wechsel() {
    let mut st = aufbauen();
    let erst = zeichen_aus(&umziehen(&mut st.app, &beweis(1)));
    let einmal = st.app.store.zuruecksetzen(BESITZER).unwrap().expect("es gibt das Konto");
    assert_eq!(einmal.len(), EINMAL_LAENGE);
    assert!(einmal.bytes().all(|b| EINMAL_ALPHABET.contains(&b)), "{einmal}");
    assert!(einmal.chars().count() >= 12, "mindestens zwoelf Zeichen, auch hier");

    // Jedes Zeichen ist zurueckgezogen, und das Konto nennt sich `secret`.
    let a = lesen_mit(&mut st.app, &erst);
    assert_eq!((a.status, a.kopf("x-maer-refusal")), (401, Some("password-changed")));
    assert_eq!(salz_fragen(&mut st.app, BESITZER)["form"], "secret");
    // Der alte Beweis meldet nicht mehr an.
    assert_eq!(anmelden_beweis(&mut st.app, &beweis(1)).status, 401);

    // Anmelden mit dem Einmal-Passwort: 428, kein Zeichen.
    let gab = st.app.http(&innen("POST", "/v1/token",
        json!({ "owner": BESITZER, "secret": einmal, "device": "d" })));
    assert_eq!(gab.status, 428);
    assert!(gab.als_json().unwrap().get("token").is_none(), "aus 428 entsteht kein Zeichen");
    // Ein falsches ist die gewoehnliche Abweisung.
    let gab = st.app.http(&innen("POST", "/v1/token",
        json!({ "owner": BESITZER, "secret": "FALSCHFALSCHFALSCHFALSCH", "device": "d" })));
    assert_eq!(gab.status, 401);

    // Der Wechsel damit geht, und danach gilt nur der neue Beweis.
    let gab = st.app.http(&innen("POST", "/v1/password", json!({
        "owner": BESITZER, "device": "geraet-r", "current": { "secret": einmal },
        "new": neu_json(&[5u8; 16], &beweis(6)) })));
    assert_eq!(gab.status, 200, "{:?}", gab.grund());
    let k = st.app.store.konto(BESITZER).unwrap().unwrap();
    assert!(!k.betreiber, "die Marke des Betreibers ist weg");
    assert_eq!(anmelden_beweis(&mut st.app, &beweis(6)).status, 200);
    let formen: Vec<(String, String, f64)> = st.app.store.geraete(BESITZER).unwrap();
    assert!(formen.iter().any(|(g, f, _)| g == "geraet-r" && f == "reset"), "{formen:?}");
}

#[test]
fn ein_einmal_passwort_fuer_ein_konto_das_es_nicht_gibt_ist_keines() {
    let mut st = aufbauen();
    assert_eq!(st.app.store.zuruecksetzen("niemand").unwrap(), None);
}

// ------------------------------------------------ Die Versuchsgrenze

fn falsch(app: &mut App, owner: &str, von: &str) -> Antwort {
    let mut a = Anfrage::neu("POST", "/v1/token")
        .mit_koerper(json!({ "owner": owner, "secret": "falsch", "device": "d" }).to_string().as_bytes())
        .von_innen();
    a.client = Some(von.into());
    app.http(&a)
}

#[test]
fn nach_zehn_fehlversuchen_ist_das_paar_gesperrt_vor_jeder_ableitung() {
    let mut st = aufbauen();
    for i in 0..GRENZE {
        assert_eq!(falsch(&mut st.app, BESITZER, QUELLE).status, 401, "Versuch {}", i + 1);
    }
    let gab = falsch(&mut st.app, BESITZER, QUELLE);
    assert_eq!(gab.status, 429);
    let warte: u64 = gab.kopf("retry-after").expect("Retry-After").parse().expect("Sekunden");
    assert!(warte >= 1 && warte as f64 <= FENSTER_S, "{warte}");

    // Gesperrt heisst gesperrt, auch fuer das RICHTIGE Passwort: die Sperre
    // steht vor dem Vergleich.
    let mut richtig = innen("POST", "/v1/token", json!({ "owner": BESITZER, "secret": ALT, "device": "d" }));
    richtig.client = Some(QUELLE.into());
    assert_eq!(st.app.http(&richtig).status, 429);
    // Und fuer den Wechsel.
    let gab = st.app.http(&innen("POST", "/v1/password", json!({
        "owner": BESITZER, "device": "d", "current": { "secret": ALT },
        "new": neu_json(&[1u8; 16], &beweis(1)) })));
    assert_eq!(gab.status, 429);

    // Eine andere Adresse ist ein anderes Paar.
    assert_eq!(falsch(&mut st.app, BESITZER, "198.51.100.9:1").status, 401);

    // Nach dem Fenster wieder offen.
    *st.uhr.lock().unwrap() += FENSTER_S + 1.0;
    assert_eq!(st.app.http(&richtig).status, 200, "nach dem Fenster geht es wieder");
}

#[test]
fn ein_unbekannter_name_zaehlt_genauso() {
    let mut st = aufbauen();
    for _ in 0..GRENZE { assert_eq!(falsch(&mut st.app, "niemand", QUELLE).status, 401); }
    assert_eq!(falsch(&mut st.app, "niemand", QUELLE).status, 429,
               "sonst verriete die Grenze, wen es gibt");
}

#[test]
fn ein_erfolg_setzt_das_paar_zurueck() {
    let mut st = aufbauen();
    for _ in 0..GRENZE - 1 { falsch(&mut st.app, BESITZER, QUELLE); }
    let mut richtig = innen("POST", "/v1/token", json!({ "owner": BESITZER, "secret": ALT, "device": "d" }));
    richtig.client = Some(QUELLE.into());
    assert_eq!(st.app.http(&richtig).status, 200);
    for _ in 0..GRENZE - 1 {
        assert_eq!(falsch(&mut st.app, BESITZER, QUELLE).status, 401,
                   "nach dem Erfolg zaehlt das Paar von vorn");
    }
}

#[test]
fn die_adresse_kommt_aus_x_real_ip_nur_hinter_der_schleife() {
    // Mit der Vorgabe-Liste, also dem Verhalten ohne `MAER_SYNC_TRUSTED_PROXIES`.
    let v = vertraute_aus(None).unwrap();
    assert_eq!(adresse_aus(Some("127.0.0.1:51234"), Some("203.0.113.5"), &v), "203.0.113.5");
    assert_eq!(adresse_aus(Some("[::1]:51234"), Some("203.0.113.5"), &v), "203.0.113.5");
    // Von aussen direkt: die Kopfzeile schreibt der Rufer selbst, sie zaehlt nicht.
    assert_eq!(adresse_aus(Some("198.51.100.2:4000"), Some("203.0.113.5"), &v), "198.51.100.2");
    // Ohne Port, sonst waere jede Verbindung ein neues Paar.
    assert_eq!(adresse_aus(Some("198.51.100.2:4000"), None, &v), "198.51.100.2");
    assert_eq!(adresse_aus(Some("[2001:db8::1]:4000"), None, &v), "2001:db8::1");
    assert_eq!(adresse_aus(None, None, &v), "-");
    // Hinter der Schleife ohne Kopfzeile: die Schleife selbst.
    assert_eq!(adresse_aus(Some("127.0.0.1:5"), None, &v), "127.0.0.1");
}

#[test]
fn die_grenze_hinter_dem_proxy_trennt_die_echten_adressen() {
    let mut st = aufbauen();
    let hinter = |app: &mut App, echt: &str| {
        let mut a = Anfrage::neu("POST", "/v1/token")
            .mit_koerper(json!({ "owner": BESITZER, "secret": "falsch" }).to_string().as_bytes())
            .mit_kopf("x-real-ip", echt).von_innen();
        a.client = Some("127.0.0.1:61000".into());
        app.http(&a).status
    };
    for _ in 0..GRENZE { hinter(&mut st.app, "203.0.113.1"); }
    assert_eq!(hinter(&mut st.app, "203.0.113.1"), 429);
    assert_eq!(hinter(&mut st.app, "203.0.113.2"), 401, "eine andere echte Adresse ist frei");
}

#[test]
fn der_deckel_haelt_die_menge_klein() {
    let mut v = Versuche::neu();
    for i in 0..(maer_sync::versuche::DECKEL + 50) {
        v.fehlschlag(&format!("n{i}"), "a", i as f64);
    }
    assert!(v.anzahl() <= maer_sync::versuche::DECKEL, "{}", v.anzahl());
}

// ------------------------------------------------ Im Tunnel

#[test]
fn im_tunnel_meldet_der_wechsel_an_und_der_neue_schluesselweg_bleibt_draussen() {
    let mut st = aufbauen();
    st.app.plain_ways = false;
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&st.privat)), None).expect("Rufer");
    let erst = rufer.write_message(b"").expect("erste Nachricht");
    let auf = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    assert_eq!(auf.status, 200);
    let sname = auf.kopf("x-maer-session").unwrap().to_string();
    rufer.read_message(&auf.body).expect("Antwort");
    let (mut hin, mut her) = rufer.split();

    let mut senden = |app: &mut App, kopf: Value, koerper: &[u8]| -> (u16, Value, Vec<u8>) {
        let roh = pack(&kopf, koerper).unwrap();
        let versiegelt = hin.encrypt_with_ad(b"", &roh).unwrap();
        let gab = app.http(&Anfrage::neu("POST", "/v1/session")
            .mit_kopf("x-maer-session", &sname).mit_koerper(&versiegelt));
        if gab.status != 200 { return (gab.status, Value::Null, gab.body); }
        let offen = her.decrypt_with_ad(b"", &gab.body).unwrap();
        let (k, b) = unpack(&offen).unwrap();
        (200, k, b)
    };

    let (aussen, _, _) = senden(&mut st.app, json!({ "method": "GET", "target": "/v2/key", "headers": {} }), b"");
    assert_eq!(aussen, 400, "der neue Schluesselweg wird innen nicht beantwortet");

    let koerper = json!({ "owner": BESITZER, "device": "tunnel", "current": { "secret": ALT },
                          "new": neu_json(&[2u8; 16], &beweis(8)) }).to_string();
    let (aussen, kopf, b) = senden(&mut st.app,
        json!({ "method": "POST", "target": "/v1/password", "headers": {} }), koerper.as_bytes());
    assert_eq!(aussen, 200);
    assert_eq!(kopf["status"], 200, "{}", String::from_utf8_lossy(&b));
    let z: Value = serde_json::from_slice(&b).unwrap();
    assert!(z["token"].is_string());

    // Die Sitzung hat bewiesen, WER fragt, und traegt weiter: eine Anfrage
    // mit dem neuen Zeichen geht durch dieselbe Sitzung.
    let (aussen, kopf, _) = senden(&mut st.app, json!({
        "method": "GET", "target": "/v1/doc/playback?scope=g1",
        "headers": { "Authorization": format!("Bearer {}", z["token"].as_str().unwrap()) } }), b"");
    assert_eq!((aussen, kopf["status"].as_u64()), (200, Some(404)));
}

// ------------------------------------------------ Was nie im Protokoll steht

#[test]
fn kein_passwort_und_kein_beweis_steht_je_im_protokoll() {
    let mut st = aufbauen();
    umziehen(&mut st.app, &beweis(1));
    st.app.http(&innen("POST", "/v1/token", json!({ "owner": BESITZER, "secret": ALT })));
    anmelden_beweis(&mut st.app, &beweis(2));
    for _ in 0..GRENZE + 1 { falsch(&mut st.app, BESITZER, QUELLE); }
    let alles: String = st.app.protokoll.iter().map(|z| z.satz.clone()).collect::<Vec<_>>().join("\n");
    assert!(!alles.is_empty(), "Vorbedingung: es gibt Zeilen");
    for geheim in [ALT.to_string(), base64(&beweis(1)), base64(&beweis(2)), "falsch".to_string()] {
        assert!(!alles.contains(&geheim), "im Protokoll steht ein Geheimnis: {geheim}");
    }
}
