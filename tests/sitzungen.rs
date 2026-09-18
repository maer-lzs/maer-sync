// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 4, reine Haelfte: die Buchfuehrung der Sitzungen.
//!
//! Die Ordnung ist hier die Entscheidung, nicht die einzelne Zahl. Deshalb
//! traegt jeder Fall die ganze Liste und ihre Antwort, und deshalb hat der
//! Fall, in dem zwei tote Sitzungen einer lebenden ihren Platz kosten
//! koennten, einen eigenen Zeugen.

use maer_sync::sessions::{self as se, Limits, Sitzung};
use serde_json::Value;

fn lese() -> Value {
    let t = std::fs::read_to_string("vektoren/sitzungen.json").expect("sitzungen.json");
    serde_json::from_str(&t).expect("JSON")
}

fn kleine(v: &Value) -> Limits {
    let g = &v["kleine_grenzen"];
    Limits {
        idle_seconds: g["idle_seconds"].as_f64().unwrap(),
        unproven_idle_seconds: g["unproven_idle_seconds"].as_f64().unwrap(),
        max_age_seconds: g["max_age_seconds"].as_f64().unwrap(),
        capacity: g["capacity"].as_u64().unwrap() as usize,
    }
}

fn paare(v: &Value) -> Vec<(String, f64, bool)> {
    v.as_array().unwrap().iter().map(|r| {
        let a = r.as_array().unwrap();
        (a[0].as_str().unwrap().to_string(), a[1].as_f64().unwrap(), a[2].as_bool().unwrap())
    }).collect()
}

fn vierer(v: &Value) -> Vec<Sitzung> {
    v.as_array().unwrap().iter().map(|r| {
        let a = r.as_array().unwrap();
        Sitzung {
            id: a[0].as_str().unwrap().to_string(),
            created_at: a[1].as_f64().unwrap(),
            last_used_at: a[2].as_f64().unwrap(),
            proven: a[3].as_bool().unwrap(),
        }
    }).collect()
}

fn liste(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

#[test]
fn die_zahlen_sind_auf_beiden_seiten_dieselben() {
    let v = lese();
    let k = &v["konstanten"];
    assert_eq!(k["IDLE_SECONDS"].as_f64(), Some(se::IDLE_SECONDS));
    assert_eq!(k["UNPROVEN_IDLE_SECONDS"].as_f64(), Some(se::UNPROVEN_IDLE_SECONDS));
    assert_eq!(k["MAX_AGE_SECONDS"].as_f64(), Some(se::MAX_AGE_SECONDS));
    assert_eq!(k["CAPACITY"].as_u64(), Some(se::CAPACITY as u64));
    assert_eq!(k["LIVE"].as_str(), Some(se::LIVE));
    assert_eq!(k["IDLE"].as_str(), Some(se::IDLE));
    assert_eq!(k["OLD"].as_str(), Some(se::OLD));
    assert_eq!(v["standing"].as_array().map(|a| a.len()), Some(16),
               "sechzehn, darunter vier, in denen BEIDES zutrifft");
}

#[test]
fn der_stand_einer_sitzung_ist_der_gemessene() {
    let v = lese();
    let g = Limits::default();
    for f in v["standing"].as_array().unwrap() {
        let ist = se::standing(f["created_at"].as_f64().unwrap(),
                               f["last_used_at"].as_f64().unwrap(),
                               f["now"].as_f64().unwrap(),
                               f["proven"].as_bool().unwrap(), &g);
        assert_eq!(ist, f["standing"].as_str().unwrap(), "Stand bei {f}");
    }
}

#[test]
fn unbewiesene_weichen_zuerst() {
    let v = lese();
    let g = kleine(&v);
    for f in v["give_way"].as_array().unwrap() {
        let s = paare(&f["sessions"]);
        assert_eq!(se::give_way(&s, &g), liste(&f["give_way"]), "Fall {}", f["name"]);
    }
}

#[test]
fn gefegt_wird_was_nicht_lebt() {
    let v = lese();
    let g = kleine(&v);
    for f in v["sweep"].as_array().unwrap() {
        let s = vierer(&f["sessions"]);
        assert_eq!(se::sweep(&s, f["now"].as_f64().unwrap(), &g),
                   liste(&f["sweep"]), "Fall {}", f["name"]);
    }
}

#[test]
fn eine_tote_sitzung_kostet_keine_lebende_ihren_platz() {
    let v = lese();
    let g = kleine(&v);
    for f in v["room_for_one"].as_array().unwrap() {
        let s = vierer(&f["sessions"]);
        assert_eq!(se::room_for_one(&s, f["now"].as_f64().unwrap(), &g),
                   liste(&f["room_for_one"]), "Fall {}", f["name"]);
    }
}

#[test]
fn die_ordnung_von_fegen_und_weichen_ist_die_entscheidung() {
    // Derselbe Bestand, zweimal gerechnet: einmal durch die eine Tuer, einmal
    // in der falschen Reihenfolge von Hand. Die Antworten muessen sich
    // unterscheiden, sonst misst der Zeuge oben nichts.
    let v = lese();
    let g = kleine(&v);
    let s = vierer(&v["room_for_one"][0]["sessions"]);
    let now = v["room_for_one"][0]["now"].as_f64().unwrap();

    let richtig = se::room_for_one(&s, now, &g);
    assert_eq!(richtig, vec!["alt1", "alt2"], "die zwei toten gehen, das Geraet bleibt");

    // Falsch herum: erst weichen lassen, dann fegen.
    let als_paare: Vec<(String, f64, bool)> = s.iter()
        .map(|x| (x.id.clone(), x.last_used_at, x.proven)).collect();
    let zuerst_weichen = se::give_way(&als_paare, &g);
    assert_eq!(zuerst_weichen, vec!["neu"],
               "in der falschen Reihenfolge trifft es das echte Geraet mitten im Handschlag");
    assert_ne!(richtig, zuerst_weichen, "waeren sie gleich, waere die Ordnung egal");
}
