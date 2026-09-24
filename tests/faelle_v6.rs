// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die erwarteten Antworten der Fassung 6, von einem ZWEITEN Autor aus dem
//! Vertragstext geschrieben, gegen diesen Dienst abgespielt.
//!
//! Fuer die Fassungen bis 5 gab es eine zweite Umsetzung, und `wege.json` war
//! an ihr gemessen. Fuer Fassung 6 gibt es sie nicht mehr. An ihre Stelle tritt
//! `vektoren/faelle-v6.json`: geschrieben aus `CONTRACT.md`, nicht aus diesem
//! Code, jeder Fall mit der Vertragszeile, die ihn verspricht. Wer den Dienst
//! baut, schreibt diese Datei nicht und aendert sie nicht; weicht der Dienst
//! ab, ist das ein Befund, keine Datei zum Nachziehen.
//!
//! Zwei Regeln der Datei, die sie selbst ausspricht (seit ihrer dritten
//! Fassung; vorher waren es Lesarten dieses Zeugen):
//!
//! - **Das Einmal-Passwort von `carla`** steht als Platzhalter
//!   `<einmal_passwort>` im dekodierten Koerper. Der Dienst wuerfelt es
//!   (`reset`), und eine Tuer, durch die man es setzen koennte, gibt es mit
//!   Absicht nicht. Der Zeuge setzt das tatsaechlich erzeugte ein, wie
//!   `wege.rs` das echte Zeichen fuer `<zeichen>`, und haelt an, wenn ein
//!   Platzhalter uebrig bleibt.
//! - **`gleich_wie` und `salt_ungleich_wie`** laufen auf DEMSELBEN Stand wie
//!   der genannte Fall, unmittelbar nach ihm: das Schein-Salz haengt an einem
//!   Geheimnis, das jeder Laden bei seiner Anlage selbst zieht.

use std::collections::BTreeMap;

use maer_sync::authorization::zufall;
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::store::Store;
use maer_sync::wege::{base64_lesen, Anfrage, Antwort, App};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DATEI: &str = "vektoren/faelle-v6.json";
/// Der Stand der Datei, der uebernommen wurde. Eine neue Fassung der Datei ist
/// eine neue Lieferung und wird hier bewusst eingetragen, nicht still gelesen.
const DATEI_DIGEST: &str = "6f8a37923cc6a622d68106a2a5a87fa2fb88cd0d9d7b86404b92c0d1967d8d12";
const QUELLE: &str = "203.0.113.7:40000";

/// Wie die Datei die Konten beschreibt (`anwendungen.v6`).
const ANNA: &str = "anna6";
const ANNA_AUFBAU: &str = "ein passwort nur fuer den aufbau dieses zeugen";
const BERTA: &str = "berta";
const BERTA_ALT: &str = "ein hinreichend langes geheimnis";
const CARLA: &str = "carla";
const CARLA_IN_DER_DATEI: &str = "<einmal_passwort>";

/// Was ein `erwartet`-Block tragen darf. Eine Pruefart, die hier nicht steht,
/// ist rot: sonst ginge eine neue Erweiterung der Datei still durch.
const BEKANNT: &[&str] = &["status", "kopfzeilen", "felder", "felder_gleich", "salt_bytes",
                           "koerper", "gleich_wie", "salt_ungleich_wie", "merke"];

fn datei() -> Value {
    let roh = std::fs::read(DATEI).expect(DATEI);
    let d: String = Sha256::digest(&roh).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(d, DATEI_DIGEST, "{DATEI} ist nicht die uebernommene Lieferung");
    serde_json::from_slice(&roh).expect("JSON")
}

fn vektor_1() -> (Vec<u8>, String) {
    let v: Value = serde_json::from_slice(&std::fs::read("vektoren/sign-in-v6.json").unwrap()).unwrap();
    let e = &v["vectors"][0];
    let salz = (0..16).map(|i| u8::from_str_radix(&e["salt_hex"].as_str().unwrap()[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    (salz, e["proof_base64"].as_str().unwrap().to_string())
}

fn b64(roh: &[u8]) -> String { maer_sync::wege::base64(roh) }

struct Stand {
    app: App,
    carla_einmal: String,
}

fn laden() -> Store {
    static ZAEHLER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir().join(format!("faelle-v6-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None).expect("Laden")
}

/// Ein frischer Stand mit den drei Konten der Datei. `anna6` zieht dabei um,
/// wie ein Geraet es taete: `/v1/password` mit dem alten Passwort als
/// `current.secret` und dem Beweis aus Vektor 1 als `new`.
fn aufbauen(anwendung: &str) -> Stand {
    let mut l = laden();
    l.create_account(ANNA, ANNA_AUFBAU).expect("anna6");
    l.create_account(BERTA, BERTA_ALT).expect("berta");
    l.create_account(CARLA, "ein passwort, das niemand mehr braucht").expect("carla");
    let carla_einmal = l.zuruecksetzen(CARLA).expect("reset").expect("carla gibt es");

    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).expect("Zufall"));
    let abdruck = fingerprint(&public_of(&privat)).expect("Abdruck");
    let mut app = App::neu(l).mit_kennung(privat, &abdruck);
    app.plain_ways = true;

    let (salz, beweis) = vektor_1();
    let umzug = json!({ "owner": ANNA, "device": "aufbau",
        "current": { "secret": ANNA_AUFBAU },
        "new": { "proof": beweis, "salt": b64(&salz), "kdf": "pbkdf2-hmac-sha256", "rounds": 600000 } });
    let mut a = Anfrage::neu("POST", "/v1/password").mit_koerper(umzug.to_string().as_bytes()).von_innen();
    a.client = Some(QUELLE.into());
    let gab = app.http(&a);
    assert_eq!(gab.status, 200, "Aufbau: anna6 zieht um wie ein Geraet");

    match anwendung {
        "v6" => {}
        "v6_ohne_kennung" => app.identity = None,
        andere => panic!("unbekannte Anwendung {andere}"),
    }
    Stand { app, carla_einmal }
}

/// Setzt `<name>` in einer Kopfzeile durch ein gemerktes Zeichen. Ein
/// Platzhalter, den es nicht gibt, haelt an: sonst reiste er woertlich mit.
fn einsetzen(wert: &str, gemerkt: &BTreeMap<String, String>) -> String {
    let mut w = wert.to_string();
    for (k, v) in gemerkt {
        w = w.replace(&format!("<{k}>"), v);
    }
    assert!(!(w.contains('<') && w.contains('>')), "Platzhalter ohne Wert: {wert}");
    w
}

fn anfrage(an: &Value, st: &Stand, gemerkt: &BTreeMap<String, String>) -> Anfrage {
    let mut a = Anfrage::neu(an["method"].as_str().unwrap(), an["path"].as_str().unwrap());
    if let Some(q) = an.get("query").and_then(|x| x.as_str()) {
        a = a.mit_frage(q);
    }
    if let Some(h) = an.get("headers").and_then(|x| x.as_object()) {
        for (k, v) in h {
            a = a.mit_kopf(k, &einsetzen(v.as_str().unwrap(), gemerkt));
        }
    }
    if let Some(b) = an.get("koerper_b64").and_then(|x| x.as_str()) {
        let roh = base64_lesen(b).expect("koerper_b64");
        let text = String::from_utf8(roh).expect("UTF-8")
            .replace(CARLA_IN_DER_DATEI, &st.carla_einmal);
        assert!(!text.contains(CARLA_IN_DER_DATEI), "Platzhalter blieb im Koerper");
        a = a.mit_koerper(text.as_bytes());
    }
    if an.get("innen").and_then(|x| x.as_bool()).unwrap_or(false) {
        a = a.von_innen();
    }
    a.client = Some(QUELLE.into());
    a
}

/// Prueft eine Antwort gegen `erwartet` und gibt zurueck, wie viele Dinge
/// geprueft wurden. Ein Block, der nichts prueft, ist rot.
fn pruefen(name: &str, e: &Value, gab: &Antwort, frueher: &BTreeMap<String, Vec<u8>>) -> usize {
    for k in e.as_object().expect("erwartet ist ein Objekt").keys() {
        assert!(BEKANNT.contains(&k.as_str()), "{name}: unbekannte Pruefart {k}");
    }
    let mut n = 0;
    assert_eq!(gab.status as u64, e["status"].as_u64().unwrap(), "{name}: Status");
    n += 1;
    if let Some(z) = e.get("kopfzeilen").and_then(|x| x.as_object()) {
        for (k, v) in z {
            let hat = gab.kopf(k).map(|s| s.to_string());
            if v.as_str() == Some("<sekunden>") {
                let s: u64 = hat.as_deref().and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| panic!("{name}: {k} ist keine ganze Zahl: {hat:?}"));
                assert!(s > 0, "{name}: {k} ist 0");
            } else {
                assert_eq!(hat.as_deref(), v.as_str(), "{name}: Kopfzeile {k}");
            }
            n += 1;
        }
    }
    let json = || gab.als_json().unwrap_or_else(|| panic!("{name}: Koerper ist kein JSON"));
    if let Some(f) = e.get("felder").and_then(|x| x.as_array()) {
        let mut haben: Vec<String> = json().as_object().expect("Objekt").keys().cloned().collect();
        haben.sort();
        let mut wollen: Vec<String> = f.iter().map(|x| x.as_str().unwrap().to_string()).collect();
        wollen.sort();
        assert_eq!(haben, wollen, "{name}: Felder");
        n += 1;
    }
    if let Some(g) = e.get("felder_gleich").and_then(|x| x.as_object()) {
        let j = json();
        for (k, v) in g {
            assert_eq!(&j[k], v, "{name}: Feld {k}");
            n += 1;
        }
    }
    if let Some(b) = e.get("salt_bytes").and_then(|x| x.as_u64()) {
        let salz = json()["salt"].as_str().map(|s| s.to_string()).expect("salt");
        assert_eq!(base64_lesen(&salz).expect("salt ist base64").len() as u64, b, "{name}: salt_bytes");
        n += 1;
    }
    if let Some(k) = e.get("koerper") {
        assert_eq!(&json(), k, "{name}: Koerper");
        n += 1;
    }
    if let Some(w) = e.get("gleich_wie").and_then(|x| x.as_str()) {
        let vorher = frueher.get(w).unwrap_or_else(|| panic!("{name}: {w} lief nicht vorher"));
        assert_eq!(&gab.body, vorher, "{name}: Koerper wie {w}");
        n += 1;
    }
    if let Some(w) = e.get("salt_ungleich_wie").and_then(|x| x.as_str()) {
        let vorher = frueher.get(w).unwrap_or_else(|| panic!("{name}: {w} lief nicht vorher"));
        let jenes: Value = serde_json::from_slice(vorher).expect("JSON des verwiesenen Falls");
        let dieses = json();
        assert!(dieses["salt"].is_string() && jenes["salt"].is_string(), "{name}: ohne salt");
        assert_ne!(dieses["salt"], jenes["salt"], "{name}: dasselbe Salz wie {w}");
        n += 1;
    }
    n
}

#[test]
fn die_datei_ist_die_uebernommene_und_gehoert_zu_diesem_vertrag() {
    let d = datei();
    assert_eq!(d["vertrag_sha256"].as_str(), Some(maer_sync::contract::DIGEST),
               "die Faelle sind aus einem anderen Vertrag geschrieben");
    assert_eq!(d["faelle"].as_array().unwrap().len(), 25, "Faelle");
    assert_eq!(d["ablaeufe"].as_array().unwrap().len(), 6, "Ablaeufe");
    for f in d["faelle"].as_array().unwrap() {
        assert!(!f["vertrag"].as_str().unwrap_or("").trim().is_empty(), "ohne Vertragszeile: {}", f["name"]);
    }
}

#[test]
fn jeder_fall_des_zweiten_autors_antwortet_so() {
    let d = datei();
    let faelle = d["faelle"].as_array().unwrap();
    let nach_name: BTreeMap<&str, &Value> =
        faelle.iter().map(|f| (f["name"].as_str().unwrap(), f)).collect();
    let mut geprueft = 0;
    for f in faelle {
        let name = f["name"].as_str().unwrap();
        let mut st = aufbauen(f["anwendung"].as_str().unwrap());
        let leer = BTreeMap::new();
        let mut frueher = BTreeMap::new();
        // Beide Verweise laufen auf DEMSELBEN Stand unmittelbar davor (siehe oben).
        for art in ["gleich_wie", "salt_ungleich_wie"] {
            if let Some(w) = f["erwartet"].get(art).and_then(|x| x.as_str()) {
                let vor = nach_name.get(w).unwrap_or_else(|| panic!("{name}: {w} gibt es nicht"));
                let a = anfrage(&vor["anfrage"], &st, &leer);
                let gab = st.app.http(&a);
                frueher.insert(w.to_string(), gab.body.clone());
            }
        }
        let a = anfrage(&f["anfrage"], &st, &leer);
        let gab = st.app.http(&a);
        let n = pruefen(name, &f["erwartet"], &gab, &frueher);
        assert!(n > 0, "{name}: nichts geprueft");
        geprueft += 1;
    }
    assert_eq!(geprueft, faelle.len());
}

#[test]
fn jeder_ablauf_des_zweiten_autors_laeuft_so() {
    let d = datei();
    let ablaeufe = d["ablaeufe"].as_array().unwrap();
    let mut schritte_gesamt = 0;
    for ab in ablaeufe {
        let aname = ab["name"].as_str().unwrap();
        let schritte = ab["schritte"].as_array().expect("schritte");
        let anw = schritte[0]["anwendung"].as_str().unwrap();
        assert!(schritte.iter().all(|s| s["anwendung"].as_str() == Some(anw)),
                "{aname}: ein Ablauf laeuft auf EINEM Stand");
        let mut st = aufbauen(anw);
        let mut gemerkt = BTreeMap::new();
        let frueher = BTreeMap::new();
        for s in schritte {
            let sname = format!("{aname} / {}", s["name"].as_str().unwrap());
            let mal = s.get("wiederhole").and_then(|x| x.as_u64()).unwrap_or(1);
            for i in 0..mal {
                let a = anfrage(&s["anfrage"], &st, &gemerkt);
                let gab = st.app.http(&a);
                pruefen(&format!("{sname} #{}", i + 1), &s["erwartet"], &gab, &frueher);
                if let Some(m) = s["erwartet"].get("merke").and_then(|x| x.as_str()) {
                    let z = gab.als_json().and_then(|j| j["token"].as_str().map(|t| t.to_string()))
                        .unwrap_or_else(|| panic!("{sname}: kein Zeichen zum Merken"));
                    gemerkt.insert(m.to_string(), z);
                }
            }
            schritte_gesamt += 1;
        }
    }
    assert!(schritte_gesamt >= ablaeufe.len());
}
