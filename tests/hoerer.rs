// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Das Register der Hoerer, herausgeloest aus dem Behandler.

use maer_sync::listeners::Listeners;

#[test]
fn eine_ansage_erreicht_nur_die_verbindungen_ihres_besitzers() {
    let mut l = Listeners::new();
    l.add("lzs", "a");
    l.add("lzs", "b");
    l.add("fremd", "c");
    assert_eq!(l.audience("lzs"), vec!["a", "b"]);
    assert_eq!(l.audience("fremd"), vec!["c"]);
    assert_eq!(l.audience("niemand"), Vec::<String>::new());
    assert_eq!(l.count("lzs"), 2);
}

#[test]
fn aufraeumen_raeumt_wirklich_auf() {
    // Ein Register, das leere Listen behaelt, waechst mit jedem Geraet, das je
    // da war. Das ist keine Theorie: es ist ein Speicher, der nur waechst.
    let mut l = Listeners::new();
    l.add("lzs", "a");
    l.add("lzs", "b");
    assert_eq!(l.owners(), 1);
    l.remove("lzs", "a");
    assert_eq!(l.count("lzs"), 1);
    assert_eq!(l.owners(), 1, "einer ist noch da");
    l.remove("lzs", "b");
    assert_eq!(l.count("lzs"), 0);
    assert_eq!(l.owners(), 0, "der Besitzer faellt weg, wenn keine Verbindung bleibt");
}

#[test]
fn ein_entfernen_das_ins_leere_geht_aendert_nichts() {
    let mut l = Listeners::new();
    l.add("lzs", "a");
    l.remove("lzs", "gibtsnicht");
    l.remove("niemand", "a");
    assert_eq!(l.count("lzs"), 1);
    assert_eq!(l.owners(), 1);
}

#[test]
fn dieselbe_verbindung_zweimal_ist_zweimal() {
    // Die Python-Fassung haengt sie an, ohne zu pruefen, und entfernt genau
    // eine. Das ist hier dasselbe, damit die Zaehlung beider Seiten stimmt.
    let mut l = Listeners::new();
    l.add("lzs", "a");
    l.add("lzs", "a");
    assert_eq!(l.count("lzs"), 2);
    l.remove("lzs", "a");
    assert_eq!(l.count("lzs"), 1, "ein Entfernen nimmt genau eine");
}
