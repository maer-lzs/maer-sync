// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wem der Dienst `X-Real-IP` glaubt, und dass die Grenze einen Gesperrten
//! nicht beim Raeumen freigibt.
//!
//! Anlass, gemessen am 24.09.2026: im Container ist die Gegenstelle nicht die
//! Schleife, sondern das Tor der Bruecke. Die Kopfzeile wurde dort nie
//! geglaubt, und zehn Fehlversuche eines Fremden auf einen Namen sperrten
//! jeden, der sich darunter anmeldet. Die Heilung ist eine ausdrueckliche
//! Liste, `MAER_SYNC_TRUSTED_PROXIES`, mit der Schleife als Vorgabe.

use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::process::{Command, Stdio};

use maer_sync::versuche::{adresse_aus, vertraute_aus, Versuche, DECKEL, FENSTER_S, GRENZE,
                          VERTRAUT_VAR};

fn ip(s: &str) -> IpAddr { s.parse().unwrap() }

#[test]
fn ohne_angabe_ist_es_die_schleife_und_sonst_genau_die_liste() {
    assert_eq!(vertraute_aus(None).unwrap(), vec![ip("127.0.0.1"), ip("::1")]);
    assert_eq!(vertraute_aus(Some("10.89.12.1")).unwrap(), vec![ip("10.89.12.1")]);
    assert_eq!(vertraute_aus(Some(" 127.0.0.1 , ::1,10.89.12.1 ")).unwrap(),
               vec![ip("127.0.0.1"), ip("::1"), ip("10.89.12.1")]);
}

#[test]
fn ein_unlesbarer_eintrag_ist_ein_fehler_und_kein_weglassen() {
    for schlecht in ["", "kein-proxy", "127.0.0.1,", "127.0.0.1,,::1", "10.0.0.0/8", "10.89.12.1:80"] {
        let e = vertraute_aus(Some(schlecht)).expect_err(schlecht);
        assert!(e.starts_with(VERTRAUT_VAR), "nennt die Variable: {e}");
    }
    // Und der Satz nennt den Eintrag, damit der Betreiber weiss, welcher.
    assert!(vertraute_aus(Some("127.0.0.1,kein-proxy")).unwrap_err().contains("'kein-proxy'"));
}

#[test]
fn das_tor_des_containers_wird_nur_geglaubt_wenn_es_in_der_liste_steht() {
    let tor = "10.89.12.1:60870";
    let mit = vertraute_aus(Some("127.0.0.1,::1,10.89.12.1")).unwrap();
    let ohne = vertraute_aus(None).unwrap();
    assert_eq!(adresse_aus(Some(tor), Some("198.51.100.1"), &mit), "198.51.100.1");
    assert_eq!(adresse_aus(Some(tor), Some("198.51.100.1"), &ohne), "10.89.12.1",
               "ohne Eintrag bleibt es das Tor, fuer alle gleich");
    // Ein Fremder, der nicht in der Liste steht, waehlt seine Adresse nicht selbst.
    assert_eq!(adresse_aus(Some("198.51.100.9:4000"), Some("203.0.113.5"), &mit), "198.51.100.9");
}

#[test]
fn beim_raeumen_bleibt_ein_gesperrtes_paar_gesperrt() {
    let mut v = Versuche::neu();
    // Das aelteste Paar, gesperrt.
    for _ in 0..GRENZE {
        v.fehlschlag("opfer", "198.51.100.1", 0.0);
    }
    assert!(v.gesperrt("opfer", "198.51.100.1", 1.0).is_some(), "Hebel: das Paar ist gesperrt");
    // Den Deckel mit jungen, ungesperrten Paaren fuellen, bis geraeumt wird.
    for i in 0..DECKEL {
        v.fehlschlag(&format!("name-{i}"), "198.51.100.2", 1.0 + i as f64 * 1e-6);
    }
    assert!(v.anzahl() < DECKEL, "Hebel: es wurde geraeumt ({})", v.anzahl());
    assert!(v.gesperrt("opfer", "198.51.100.1", 2.0).is_some(),
            "das gesperrte Paar ist beim Raeumen verschwunden");
    // Und es bleibt nur bis zum Ende seines Fensters.
    assert!(v.gesperrt("opfer", "198.51.100.1", FENSTER_S + 0.5).is_none());
}

fn wegwerf() -> std::path::PathBuf {
    static Z: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!("vertraut-{}-{}", std::process::id(),
        Z.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn der_start_haelt_bei_einer_unlesbaren_liste_laut_an() {
    let d = wegwerf();
    let aus = Command::new(env!("CARGO_BIN_EXE_maer-sync"))
        .arg("serve")
        .env("MAER_SYNC_DB", d.join("s.sqlite"))
        .env("MAER_SYNC_BIND", "127.0.0.1:0")
        .env(VERTRAUT_VAR, "127.0.0.1,kein-proxy")
        .output().expect("Start");
    let fehler = String::from_utf8_lossy(&aus.stderr);
    assert_eq!(aus.status.code(), Some(6), "Ausgang: {:?}, {fehler}", aus.status);
    assert!(fehler.contains("'kein-proxy' is not an address"), "Satz: {fehler}");
    assert!(!fehler.contains("serving on"), "er darf nicht dienen: {fehler}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn der_start_nennt_die_geglaubten_proxys_in_einer_zeile() {
    let d = wegwerf();
    let mut kind = Command::new(env!("CARGO_BIN_EXE_maer-sync"))
        .arg("serve")
        .env("MAER_SYNC_DB", d.join("s.sqlite"))
        .env("MAER_SYNC_BIND", "127.0.0.1:0")
        .env(VERTRAUT_VAR, "127.0.0.1, ::1, 10.89.12.1")
        .stderr(Stdio::piped()).stdout(Stdio::null())
        .spawn().expect("Start");
    let leser = BufReader::new(kind.stderr.take().unwrap());
    let mut gesehen = Vec::new();
    for zeile in leser.lines().map_while(Result::ok) {
        let fertig = zeile.starts_with("serving on");
        gesehen.push(zeile);
        if fertig { break; }
    }
    let _ = kind.kill();
    let _ = kind.wait();
    let _ = std::fs::remove_dir_all(&d);
    let treffer: Vec<&String> = gesehen.iter().filter(|z| z.starts_with("trusting X-Real-IP from:")).collect();
    assert_eq!(treffer.len(), 1, "genau eine Zeile: {gesehen:?}");
    assert_eq!(treffer[0], "trusting X-Real-IP from: 127.0.0.1, ::1, 10.89.12.1");
}
