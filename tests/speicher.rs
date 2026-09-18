// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 2, zweite Haelfte: der Speicher.
//!
//! Der wichtigste Zeuge hier liest eine Datei, die die PYTHON-Seite angelegt
//! hat. Er wird UEBERSPRUNGEN MIT GRUND, wenn sie nicht da ist (Regel 7:
//! Nichtwissen gilt nicht als Wissen), und er ist rot, wenn sie da ist und
//! nicht passt. Eine leere Liste waere die dritte Moeglichkeit, und die gibt
//! es hier nicht.

use maer_sync::store::{Store, StoreError, SCHEMA};
use maer_sync::writes;

fn wegwerf(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("maer-sync-zeuge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&d);
    d.to_string_lossy().into_owned()
}

#[test]
fn ein_frischer_speicher_traegt_seine_gestalt() {
    let pfad = wegwerf("frisch");
    let s = Store::open(&pfad, 10, None).expect("anlegen");
    let gefunden: String = s.db
        .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| r.get(0))
        .expect("Gestalt steht drin");
    assert_eq!(gefunden, SCHEMA.to_string());
    // Und die drei Tabellen sind wirklich da, nicht nur behauptet.
    for t in ["documents", "accounts", "tokens", "meta"] {
        let n: i64 = s.db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [t], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "Tabelle {t} fehlt");
    }
    let _ = std::fs::remove_file(&pfad);
}

#[test]
fn schreiben_lesen_und_das_etikett_zaehlt_hoch() {
    let pfad = wegwerf("schreiben");
    let mut s = Store::open(&pfad, 10, None).expect("anlegen");
    let (u1, e1) = s.write("lzs", "stands", b"erster", 1, "", None, false).expect("Schrieb");
    assert_eq!(u1.status, writes::WRITTEN);
    assert_eq!(e1.as_deref(), Some("1"));

    let held = s.current("lzs", "stands", "").expect("lesen").expect("da");
    assert_eq!(held.body, b"erster");
    assert_eq!(held.format, 1);
    assert_eq!(held.etag, "1");

    // Mit dem gelesenen Etikett darf geschrieben werden.
    let (u2, e2) = s.write("lzs", "stands", b"zweiter Stand", 1, "", Some("1"), false).unwrap();
    assert_eq!(u2.status, writes::WRITTEN);
    assert_eq!(e2.as_deref(), Some("2"));
    assert_eq!(s.versions("lzs", "stands", "").unwrap(), vec![1, 2]);
    let _ = std::fs::remove_file(&pfad);
}

#[test]
fn ein_falsches_etikett_schreibt_nicht_und_laesst_keine_spur() {
    let pfad = wegwerf("cas");
    let mut s = Store::open(&pfad, 10, None).expect("anlegen");
    s.write("lzs", "stands", b"erster", 1, "", None, false).unwrap();

    // Falsche Fassung: 412, und der Bestand bleibt.
    let (u, e) = s.write("lzs", "stands", b"ueberschreiben", 1, "", Some("9"), false).unwrap();
    assert_eq!(u.status, writes::STALE);
    assert!(e.is_none());
    // Schweigen ist kein Anspruch: auch ohne Etikett 412.
    let (u2, _) = s.write("lzs", "stands", b"blind", 1, "", None, false).unwrap();
    assert_eq!(u2.status, writes::STALE);

    // Die Spur: genau eine Fassung, und der Koerper ist der alte.
    assert_eq!(s.versions("lzs", "stands", "").unwrap(), vec![1]);
    assert_eq!(s.current("lzs", "stands", "").unwrap().unwrap().body, b"erster");
    let _ = std::fs::remove_file(&pfad);
}

#[test]
fn die_rotation_laesst_genau_so_viele_stehen_wie_abgesprochen() {
    let pfad = wegwerf("rotation");
    let mut s = Store::open(&pfad, 3, None).expect("anlegen");
    let mut etikett: Option<String> = None;
    for i in 1..=5 {
        let koerper = format!("stand {i} mit genug Bytes, damit kein Einbruch entsteht");
        let (u, e) = s.write("lzs", "stands", koerper.as_bytes(), 1, "",
                             etikett.as_deref(), false).unwrap();
        assert_eq!(u.status, writes::WRITTEN, "Schrieb {i}: {}", u.reason);
        etikett = e;
    }
    assert_eq!(s.versions("lzs", "stands", "").unwrap(), vec![3, 4, 5],
               "keep=3 laesst die drei neuesten stehen");
    let _ = std::fs::remove_file(&pfad);
}

#[test]
fn ein_speicher_aus_der_zukunft_wird_nicht_geoeffnet() {
    // Der Rueckweg, der kein Rueckweg waere: ein zurueckgerollter Dienst
    // wandert nicht rueckwaerts und raet nicht.
    let pfad = wegwerf("zukunft");
    {
        let s = Store::open(&pfad, 10, None).expect("anlegen");
        s.db.execute("UPDATE meta SET value='99' WHERE key='schema'", []).unwrap();
    }
    match Store::open(&pfad, 10, None) {
        Err(StoreError::AheadOfService(grund)) => {
            assert!(grund.contains("does not migrate backwards"), "Grund: {grund}");
        }
        Err(e) => panic!("falscher Fehler: {e:?}"),
        Ok(_) => panic!("ein Speicher aus der Zukunft wurde geoeffnet"),
    }
    let _ = std::fs::remove_file(&pfad);
}

#[test]
fn bereiche_geben_namen_und_fassungen_ohne_koerper() {
    let pfad = wegwerf("bereiche");
    let mut s = Store::open(&pfad, 10, None).expect("anlegen");
    s.write("lzs", "stands", b"eins", 1, "geraet-a", None, false).unwrap();
    s.write("lzs", "stands", b"zwei", 1, "geraet-b", None, false).unwrap();
    s.write("lzs", "stands", b"zwei aber laenger", 1, "geraet-b", Some("1"), false).unwrap();
    let b = s.scopes("lzs", "stands").unwrap();
    assert_eq!(b, vec![("geraet-a".to_string(), "1".to_string(), 1),
                       ("geraet-b".to_string(), "2".to_string(), 1)]);
    let _ = std::fs::remove_file(&pfad);
}

// ------------------------------------------------- Invariante 4

#[test]
#[ignore = "braucht MAER_SYNC_PYTHON_DB, eine von der Python-Seite angelegte Datei"]
fn eine_von_python_angelegte_datei_ist_lesbar() {
    // Regel 7, und der erste Versuch hier war selbst der Fehler: ein `return`
    // im Nichtwissen macht den Fall GRUEN, weil Rusts Zeugen-Werk kein
    // "uebersprungen" kennt. Das ist der gruene Lauf ohne Messung, gegen den
    // die Regel gebaut ist.
    //
    // Deshalb `#[ignore]`: der Fall erscheint als UEBERSPRUNGEN und nicht als
    // bestanden, und wer ihn mit `--ignored` ruft, bekommt einen Fehler, wenn
    // die Datei fehlt. Beide Richtungen sind damit ehrlich.
    let pfad = std::env::var("MAER_SYNC_PYTHON_DB").expect(
        "dieser Fall wurde ausdruecklich gerufen, aber MAER_SYNC_PYTHON_DB ist nicht gesetzt: \
ohne die Datei der Python-Seite ist Invariante 4 nicht gemessen");
    assert!(std::path::Path::new(&pfad).exists(),
            "MAER_SYNC_PYTHON_DB zeigt auf nichts: {pfad}");

    let s = Store::open(&pfad, 10, None).expect("die Python-Datei muss sich oeffnen lassen");
    let gestalt: String = s.db
        .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| r.get(0))
        .expect("Gestalt");
    assert_eq!(gestalt, SCHEMA.to_string(), "dieselbe Gestalt");

    let held = s.current("lzs", "stands", "").expect("lesen")
        .expect("die Python-Seite hat ein Dokument geschrieben");
    assert_eq!(held.body, b"von der Python-Seite geschrieben", "derselbe Koerper");
    assert_eq!(held.format, 1);
    assert_eq!(held.etag, "2", "zwei Fassungen, das Etikett ist die zweite");
    assert_eq!(s.versions("lzs", "stands", "").unwrap(), vec![1, 2]);
    let b = s.scopes("lzs", "stands").unwrap();
    assert_eq!(b, vec![("".to_string(), "2".to_string(), 1)]);
}
