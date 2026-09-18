// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 5: die Schliesscodes und die Zustellung, gegen die gemessene Tafel.
//!
//! `vektoren/horcher.json` ist an der laufenden Python-Anwendung gezogen. Der
//! Socket antwortet im Fehlerfall mit einer ZAHL und sonst nichts, und ein
//! falscher Code ist auf dem Geraet von keinem anderen zu unterscheiden; die
//! Tafel ist die einzige Stelle, an der das auffaellt.

use std::collections::BTreeMap;

use maer_sync::authorization::zufall;
use maer_sync::frames::pack;
use maer_sync::handshake::HandshakeState;
use maer_sync::horcher::{Tat, Wort};
use maer_sync::identity::{fingerprint, public_of};
use maer_sync::store::Store;
use maer_sync::wege::{Anfrage, Ansage, App};
use serde_json::Value;

const BESITZER: &str = "anna";
const GEHEIMNIS: &str = "ein hinreichend langes geheimnis";
const FREMDER: &str = "bodo";
const FREMD_GEHEIMNIS: &str = "ein ganz anderes langes geheimnis";

struct Stand {
    app: App,
    zeichen: String,
    fremdes: String,
    privat: [u8; 32],
}

fn aufbauen() -> Stand {
    #[allow(dead_code)]
    static ZAEHLER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nr = ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let wo = std::env::temp_dir().join(format!("horcher-{}-{nr}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None)
        .expect("Laden");
    laden.create_account(BESITZER, GEHEIMNIS).expect("Konto");
    laden.create_account(FREMDER, FREMD_GEHEIMNIS).expect("fremdes Konto");
    let zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1").unwrap().unwrap();
    let fremdes = laden.issue_token(FREMDER, FREMD_GEHEIMNIS, "geraet-x").unwrap().unwrap();
    let mut privat = [0u8; 32];
    privat.copy_from_slice(&zufall(32).unwrap());
    let abdruck = fingerprint(&public_of(&privat)).unwrap();
    let app = App::neu(laden).mit_kennung(privat, &abdruck);
    Stand { app, zeichen, fremdes, privat }
}

fn tafel() -> Value {
    let t = std::fs::read_to_string("vektoren/horcher.json").expect("vektoren/horcher.json");
    serde_json::from_str(&t).expect("JSON")
}

fn kopfzeilen(v: &Value, zeichen: &str, fremdes: &str, sitzung: &str) -> BTreeMap<String, String> {
    v.as_object().map(|o| o.iter().map(|(k, val)| {
        let val = match val.as_str().unwrap() {
            "<zeichen>" => format!("Bearer {zeichen}"),
            "<fremdes zeichen>" => format!("Bearer {fremdes}"),
            "<sitzung>" => sitzung.to_string(),
            anderes => anderes.to_string(),
        };
        (k.clone(), val)
    }).collect()).unwrap_or_default()
}

/// Eine Sitzung aufmachen, von der Client-Seite.
fn sitzung_auf(st: &mut Stand) -> (String, maer_sync::handshake::CipherState,
                                   maer_sync::handshake::CipherState) {
    let mut rufer = HandshakeState::new(true, b"", None, Some(public_of(&st.privat)), None).unwrap();
    let erst = rufer.write_message(b"").unwrap();
    let auf = st.app.http(&Anfrage::neu("POST", "/v1/session").mit_koerper(&erst));
    assert_eq!(auf.status, 200, "der Handschlag muss angenommen werden");
    let name = auf.kopf("x-maer-session").unwrap().to_string();
    rufer.read_message(&auf.body).unwrap();
    let (hin, her) = rufer.split();
    (name, hin, her)
}

#[test]
fn jeder_gemessene_fall_des_horchers_antwortet_genauso() {
    let t = tafel();
    let faelle = t["faelle"].as_array().expect("faelle");
    assert!(faelle.len() >= 12, "die Tafel ist geschrumpft: {}", faelle.len());

    // EINMAL bauen. Ein Konto je Fall waeren vier scrypt-Laeufe mal zwoelf, und
    // eine Suite, die anderthalb Minuten braucht, wird nicht mehr gefahren.
    // Dafuer wird jeder Horcher am Ende seines Falls ausgetragen, sonst hoert
    // der offene aus Fall sechs noch in Fall sieben mit.
    let mut st = aufbauen();

    for fall in faelle {
        let name = fall["name"].as_str().unwrap();
        st.app.plain_ways = fall["anwendung"].as_str().unwrap() != "geschlossen";

        let an = &fall["anfrage"];
        let pfad = an["path"].as_str().unwrap();
        let gestalt = an.get("erste_gestalt").and_then(|x| x.as_str());

        // Eine echte Sitzung nur dort, wo der Fall eine braucht.
        let sitzung = if gestalt.is_some() { Some(sitzung_auf(&mut st)) } else { None };
        let vorher = st.app.listeners.count(BESITZER);
        let sname = sitzung.as_ref().map(|(n, _, _)| n.clone()).unwrap_or_default();
        let zeilen = kopfzeilen(&an["headers"], &st.zeichen, &st.fremdes, &sname);

        let (mut h, mut taten) = st.app.horcher_oeffnen(pfad, &zeilen);
        if taten.is_empty() {
            taten.extend(st.app.horcher_wort(&mut h, Wort::Verbunden));
        }

        // Das erste Wort des versiegelten Horchers.
        if let (Some(g), Some((_, mut hin, mut her))) = (gestalt, sitzung) {
            let erstes = match g {
                "rahmen" => {
                    let mut kopf = serde_json::json!({ "method": "GET", "target": "/v1/changes" });
                    if let Some(hh) = an["erster_rahmen"].get("headers") {
                        kopf["headers"] = serde_json::json!({
                            "authorization": format!("Bearer {}", st.zeichen) });
                        let _ = hh;
                    }
                    Wort::Bytes(hin.encrypt_with_ad(b"", &pack(&kopf, b"").unwrap()).unwrap())
                }
                "versiegelter_unsinn" =>
                    Wort::Bytes(hin.encrypt_with_ad(b"", b"kein rahmen").unwrap()),
                "text_statt_bytes" => Wort::Text("kein siegel".into()),
                "kein_siegel" => Wort::Bytes(b"kein siegel".to_vec()),
                andere => panic!("{name}: unbekannte Gestalt {andere}"),
            };
            taten.extend(st.app.horcher_wort(&mut h, erstes));

            // Die Ansage, wenn der Fall eine erwartet.
            if let Some(ansagen) = fall["erwartet"].get("ansagen").and_then(|x| x.as_array()) {
                assert!(h.ist_offen(), "{name}: der Horcher ist nicht auf der Liste");
                for will in ansagen {
                    let ansage = Ansage { owner: BESITZER.into(), text: will.to_string() };
                    for (_, tat) in st.app.zustellen(&ansage) {
                        match &tat {
                            Tat::SendeBytes(b) => {
                                let klar = her.decrypt_with_ad(b"", b)
                                    .expect("die Ansage muss in DIESER Sitzung aufgehen");
                                let gelesen: Value = serde_json::from_slice(&klar).unwrap();
                                assert_eq!(&gelesen, will, "{name}: Ansage");
                            }
                            andere => panic!("{name}: versiegelt erwartet, bekam {andere:?}"),
                        }
                        taten.push(tat);
                    }
                }
            }
        } else if let Some(ansagen) = fall["erwartet"].get("ansagen").and_then(|x| x.as_array()) {
            // Der offene Horcher. Angesagt wird immer fuer BESITZER; ob DIESER
            // Horcher es hoert, ist genau die Frage.
            assert!(h.ist_offen(), "{name}: der Horcher ist nicht auf der Liste");
            let text = serde_json::json!({
                "kind": "playback", "scope": "geraet-1", "version": "1" }).to_string();
            let ansage = Ansage { owner: BESITZER.into(), text: text.clone() };
            let zugestellt = st.app.zustellen(&ansage);
            assert_eq!(zugestellt.len(), ansagen.len(),
                       "{name}: Zahl der Zustellungen");
            for (_, tat) in zugestellt {
                assert_eq!(tat, Tat::SendeText(text.clone()), "{name}: als Text, nicht versiegelt");
                taten.push(tat);
            }
        }

        let gesendet: Vec<&str> = taten.iter().map(|t| t.art()).collect();
        let gewollt: Vec<String> = fall["erwartet"]["gesendet"].as_array().unwrap()
            .iter().map(|x| x.as_str().unwrap().to_string()).collect();
        assert_eq!(gesendet, gewollt, "{name}: die Folge der Taten");

        let angenommen = taten.iter().any(|t| *t == Tat::Annehmen);
        assert_eq!(angenommen, fall["erwartet"]["angenommen"].as_bool().unwrap(),
                   "{name}: angenommen");

        let code = taten.iter().find_map(|t| match t {
            Tat::Schliessen(c) => Some(*c), _ => None });
        assert_eq!(code.map(|c| c as u64),
                   fall["erwartet"].get("schliesscode").and_then(|x| x.as_u64()),
                   "{name}: Schliesscode");

        st.app.horcher_austragen(&mut h);
        assert_eq!(st.app.listeners.count(BESITZER), vorher,
                   "{name}: laesst einen Horcher auf der Liste zurueck");
    }
}

/// Ein Horcher, der weg ist, hoert nichts mehr. Ohne diesen Zeugen bliebe eine
/// tote Verbindung auf der Liste, und jede Ansage liefe in sie hinein.
#[test]
fn ein_getrennter_horcher_steht_nicht_mehr_auf_der_liste() {
    let mut st = aufbauen();
    let zeilen = BTreeMap::from([
        ("authorization".to_string(), format!("Bearer {}", st.zeichen))]);
    let (mut h, taten) = st.app.horcher_oeffnen("/v1/changes", &zeilen);
    assert!(taten.is_empty());
    st.app.horcher_wort(&mut h, Wort::Verbunden);
    assert_eq!(st.app.listeners.count(BESITZER), 1);

    let ansage = Ansage { owner: BESITZER.into(), text: "{}".into() };
    assert_eq!(st.app.zustellen(&ansage).len(), 1, "vorher hoert er");

    st.app.horcher_wort(&mut h, Wort::Getrennt);
    assert_eq!(st.app.listeners.count(BESITZER), 0);
    assert_eq!(st.app.zustellen(&ansage).len(), 0, "nachher nicht mehr");
}

/// Zwei Horcher desselben Besitzers hoeren beide, und der eine traegt den
/// anderen nicht aus. Ohne eigene Kennung je Verbindung waere genau das der
/// Fehler: das zweite Geraet nimmt dem ersten die Ansagen weg.
#[test]
fn zwei_horcher_desselben_besitzers_hoeren_beide() {
    let mut st = aufbauen();
    let zeilen = BTreeMap::from([
        ("authorization".to_string(), format!("Bearer {}", st.zeichen))]);
    let (mut a, _) = st.app.horcher_oeffnen("/v1/changes", &zeilen);
    let (mut b, _) = st.app.horcher_oeffnen("/v1/changes", &zeilen);
    assert_ne!(a.id, b.id, "zwei Verbindungen sind zwei Kennungen");
    st.app.horcher_wort(&mut a, Wort::Verbunden);
    st.app.horcher_wort(&mut b, Wort::Verbunden);

    let ansage = Ansage { owner: BESITZER.into(), text: "{}".into() };
    assert_eq!(st.app.zustellen(&ansage).len(), 2, "beide hoeren");

    st.app.horcher_wort(&mut a, Wort::Getrennt);
    assert_eq!(st.app.zustellen(&ansage).len(), 1, "der andere hoert weiter");
}

/// Der Klient hat auf diesem Weg nichts zu sagen. Was er schickt, bewegt
/// nichts: ein Kanal, der nur ansagt, laesst sich nicht zu etwas anderem
/// ueberreden.
#[test]
fn was_der_klient_auf_dem_horchweg_sagt_bewegt_nichts() {
    let mut st = aufbauen();
    let zeilen = BTreeMap::from([
        ("authorization".to_string(), format!("Bearer {}", st.zeichen))]);
    let (mut h, _) = st.app.horcher_oeffnen("/v1/changes", &zeilen);
    st.app.horcher_wort(&mut h, Wort::Verbunden);

    for wort in [Wort::Text("gib mir alles".into()), Wort::Bytes(vec![1, 2, 3]),
                 Wort::Verbunden] {
        assert!(st.app.horcher_wort(&mut h, wort).is_empty(), "nichts folgt daraus");
    }
    assert!(h.ist_offen(), "und er bleibt offen");
    assert_eq!(st.app.listeners.count(BESITZER), 1);
}
