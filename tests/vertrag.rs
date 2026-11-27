// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Vertrags-Zaun von der Rust-Seite.
//!
//! Der wichtigste Fall hier misst nicht den Vertrag, sondern die ZWEI SEITEN
//! gegeneinander. Eine Seite, die nur ihre Datei gegen ihre eigene Konstante
//! haelt, bleibt gruen, waehrend die andere wandert. Das ist in diesem Repo
//! am 18.09.2026 vorgekommen, und dieser Zeuge macht es unmoeglich.

use maer_sync::contract::{self, declared, digest_of, DIGEST, VERSION};
use maer_sync::{paths, writes};

/// A line of the block together with its addition line (`additions-<key>`):
/// what the service answers is version 6 plus its additions.
fn samt_ergaenzungen(block: &std::collections::BTreeMap<String, Vec<String>>, key: &str) -> Vec<String> {
    let mut v = block.get(key).cloned().unwrap_or_default();
    v.extend(block.get(&format!("additions-{key}")).cloned().unwrap_or_default());
    v
}

fn vertrag() -> Vec<u8> {
    std::fs::read("CONTRACT.md").expect("CONTRACT.md liegt neben dem Zweig")
}

#[test]
fn der_vertrag_hat_den_digest_den_diese_seite_erwartet() {
    assert_eq!(digest_of(&vertrag()), DIGEST,
               "der Vertrag ist nicht der, den dieser Bau bedient");
}

#[test]
fn der_vertrag_nennt_seine_fassung_und_es_ist_unsere() {
    let text = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&text).expect("der Vertrag hat einen maschinenlesbaren Block");
    assert_eq!(block.get("version").map(|v| v.as_slice()), Some(&[VERSION.to_string()][..]),
               "der Vertrag nennt eine andere Fassung als dieser Bau bedient");
}

#[test]
fn was_dieser_bau_beantwortet_steht_im_vertrag() {
    // Die andere Richtung: nicht "der Vertrag ist unveraendert", sondern
    // "was der Code tut, ist darin genannt". Ohne das kann der Vertrag
    // unveraendert und trotzdem unerfuellt sein.
    let text = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&text).unwrap();
    let status = block.get("status").expect("der Vertrag nennt Statuswerte");
    for s in [writes::WRITTEN, writes::STALE, writes::REFUSED] {
        assert!(status.contains(&s.to_string()),
                "dieser Bau antwortet mit {s}, der Vertrag nennt es nicht");
    }
    let pfade = block.get("paths").expect("der Vertrag nennt Pfade");
    for p in ["/v1/doc", "/v1/docs"] {
        assert!(pfade.contains(&p.to_string()), "Pfad {p} fehlt im Vertrag");
    }
    // und die Pfadform dieses Baus passt zu den genannten Pfaden
    assert_eq!(paths::parse_document_path("/v1/doc/stands").as_deref(), Some("stands"));
    assert_eq!(paths::parse_listing_path("/v1/docs/stands").as_deref(), Some("stands"));
}

#[test]
fn ein_vertrag_ohne_block_faellt_auf() {
    // Der leere Fall darf keine leere Karte sein: eine leere Karte machte
    // jede Pruefung darueber gruen, ohne etwas gemessen zu haben.
    assert!(declared("nur Prosa, kein Block").is_none());
    assert!(declared("<!-- maer-sync-contract\n-->").is_none());
    assert!(contract::declared("<!-- maer-sync-contract\nversion: 5\n-->").is_some());
}

// --------------------------------------------------------------------------
// Scheibe 6: dieselben Fragen, die der Python-Zaun seinem Dienst stellt.
//
// Bis hierher fragte dieser Zeuge nur, ob der Vertrag unveraendert ist, und in
// der Gegenrichtung prueften **drei Statuswerte und zwei Pfade von Hand**. Das
// ist genau die Gestalt, vor der die eigene Doktrin warnt: eine Handliste, wo
// man die Quelle messen kann. Eine Liste von Hand vergisst; eine Schleife ueber
// die Quelle kann es nicht.
//
// **Der Loch-Melder ist der Teil, der die Gegenrichtung ueberhaupt
// aussagekraeftig macht.** Findet der Scan weniger, als der Vertrag nennt, dann
// sieht er nicht alles, und sein Schweigen ueber Ungenanntes bedeutet nichts.
// Deshalb faellt er dann um, statt still zu bleiben.

/// Der ganze Rust-Quelltext als Text. Nicht `tests/`: gefragt ist, was der
/// DIENST tut, nicht was seine Zeugen erwaehnen.
fn quelltext() -> String {
    let mut aus = String::new();
    let mut dateien: Vec<_> = std::fs::read_dir("src").expect("src/")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "rs").unwrap_or(false))
        .collect();
    dateien.sort();
    assert!(dateien.len() >= 10, "src/ ist zu klein, hier wird nichts gemessen");
    for p in dateien {
        aus.push_str(&std::fs::read_to_string(&p).expect("lesbar"));
        aus.push('\n');
    }
    aus
}

/// Alle dreistelligen Zahlen, die direkt hinter einem dieser Namen und einer
/// oeffnenden Klammer stehen.
fn zahlen_hinter(text: &str, namen: &[&str]) -> std::collections::BTreeSet<String> {
    let mut aus = std::collections::BTreeSet::new();
    for name in namen {
        let muster = format!("{name}(");
        let mut i = 0;
        while let Some(pos) = text[i..].find(&muster) {
            let start = i + pos + muster.len();
            let ziffern: String = text[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
            // GENAU drei Ziffern, und danach keine weitere: sonst faengt das
            // Muster `4401` als `440`. Genau das ist beim ersten Anlauf
            // passiert, und es erfand drei Statuswerte, die es nicht gibt.
            if ziffern.len() == 3 {
                aus.insert(ziffern);
            }
            i = start;
        }
    }
    aus
}

/// Statuswerte, die als benannte Konstante entstehen (`pub const STALE: u16 = 412;`).
fn zahlen_als_konstante(text: &str) -> std::collections::BTreeSet<String> {
    let mut aus = std::collections::BTreeSet::new();
    for zeile in text.lines() {
        let z = zeile.trim();
        if !z.starts_with("pub const ") || !z.contains(": u16 = ") {
            continue;
        }
        if let Some((_, rest)) = z.split_once(": u16 = ") {
            let ziffern: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if ziffern.len() == 3 {
                aus.insert(ziffern);
            }
        }
    }
    aus
}

/// Was diese Umsetzung an Statuswerten beantwortet, in BEIDEN Gestalten:
/// an der Antworttuer und als benannte Konstante.
fn beantwortete_status() -> std::collections::BTreeSet<String> {
    let text = quelltext();
    let mut aus = zahlen_hinter(&text, &["grund", "als_json", "als_bytes", "leer", ".status"]);
    aus.extend(zahlen_als_konstante(&text));
    aus
}

/// Statuswerte, die diese Umsetzung beantwortet, ohne dass der Vertrag sie in
/// seiner Liste nennt, **je mit ihrem Grund**. Ein Eintrag ohne Gegenstand ist
/// eine Ausnahme, die niemand mehr nachsieht, also sperrt die Liste in beide
/// Richtungen: zu viele ist ein Befund, zu wenige auch.
const AUSSERHALB_DER_LISTE: &[(&str, &str)] = &[
    ("403", "der Vertrag behandelt 403 in Prosa statt in der Liste, mit zwei \
Saetzen (Abschnitt ueber `/v1/changes`): vor dem Annehmen ist ein 403 die \
Abweisung des Horchers. Die Python-Fassung laesst ihn vom Server erzeugen, \
diese hier schreibt ihn selbst, damit auf der Leitung dasselbe steht"),
    ("500", "der Vertrag nennt 500 auf KEINER Seite. Die Python-Fassung kann \
hier nicht antworten: der Fehler fliegt aus dem Laden, und das Geraet sieht \
die 500 des Servers. Diese Umsetzung schreibt dieselbe Zahl mit demselben \
leeren Koerper, damit der Unterschied keiner ist"),
];

/// Wege, die diese Umsetzung beantwortet, ohne dass der Vertrag sie nennt,
/// je mit Grund. Wie bei den Statuswerten sperrt die Liste in beide
/// Richtungen: ein Eintrag, den der Quelltext nicht mehr traegt, ist ein Befund.
const AUSSERHALB_DER_PFADE: &[(&str, &str)] = &[
    ("/v1/key", "Riegel (c) der Fassung 6: der Vertrag nimmt den alten \
Schluesselweg aus seiner Liste, und genau deshalb MUSS der Dienst ihn kennen. \
Er antwortet 404 fuer jede Methode, vor allem anderen; ohne eigenen Zweig liefe \
er in die Zeichenpruefung und antwortete 401, und nur ein 404 haelt eine App \
der Fassung 5 an, bevor sie ein Passwort schickt"),
];

#[test]
fn jeder_genannte_weg_steht_in_dieser_umsetzung() {
    let text = quelltext();
    let vertragstext = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&vertragstext).expect("der Vertrag hat seinen Block");
    let wege = &samt_ergaenzungen(&block, "paths");
    assert!(wege.len() >= 7, "der Vertrag nennt nur {} Pfade", wege.len());
    for weg in wege {
        // Zwei Gestalten, weil der Dienst seine Pfade auf zwei Arten baut:
        // als ganzes Literal, und zerlegt (`sorte_aus(path, "doc")` mit der
        // Fassung als Konstante). Ein Scan nur auf das erste faende `/v1/doc`
        // nie.
        let ganz = text.contains(&format!("\"{weg}\""));
        let zerlegt = weg.strip_prefix("/v1/")
            .map(|rest| text.contains(&format!("\"{rest}\"")))
            .unwrap_or(false);
        assert!(ganz || zerlegt,
                "der Vertrag verspricht {weg} und diese Umsetzung kennt ihn nicht");
    }
}

#[test]
fn jeder_weg_den_diese_umsetzung_beantwortet_steht_im_vertrag() {
    let text = quelltext();
    let vertragstext = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&vertragstext).expect("Block");
    let alle_wege = samt_ergaenzungen(&block, "paths");
    let genannt: std::collections::BTreeSet<&String> = alle_wege.iter().collect();

    // Alle ganzen `/v1/…`-Literale aus dem Quelltext.
    let mut gefunden = std::collections::BTreeSet::new();
    let mut i = 0;
    while let Some(pos) = text[i..].find("\"/v1/") {
        let start = i + pos + 1;
        let ende = start + text[start..].find('"').expect("ein Literal endet");
        gefunden.insert(text[start..ende].to_string());
        i = ende;
    }
    // Der Loch-Melder: sieht der Scan die genannten Pfade nicht, sagt sein
    // Schweigen ueber ungenannte nichts.
    let ganz_genannte: Vec<&&String> = genannt.iter()
        .filter(|w| text.contains(&format!("\"{w}\"")))
        .collect();
    assert!(ganz_genannte.len() >= 5,
            "dieser Scan findet nur {} der genannten Pfade als ganzes Literal, \
er sieht also zu wenig", ganz_genannte.len());

    let mit_grund: std::collections::BTreeSet<&str> =
        AUSSERHALB_DER_PFADE.iter().map(|(w, _)| *w).collect();
    let ungenannt: Vec<&String> = gefunden.iter()
        .filter(|w| !genannt.contains(w) && !mit_grund.contains(w.as_str()))
        .collect();
    assert!(ungenannt.is_empty(),
            "Wege, die diese Umsetzung beantwortet und der Vertrag nicht nennt: {ungenannt:?}");

    // Die andere Richtung der Ausnahmeliste.
    let ohne_gegenstand: Vec<&str> = AUSSERHALB_DER_PFADE.iter()
        .map(|(w, _)| *w)
        .filter(|w| !gefunden.contains(*w))
        .collect();
    assert!(ohne_gegenstand.is_empty(),
            "die Ausnahmeliste nennt {ohne_gegenstand:?}, und diese Umsetzung kennt den Weg gar nicht");
    for (w, grund) in AUSSERHALB_DER_PFADE {
        assert!(!genannt.iter().any(|g| g.as_str() == *w),
                "{w} steht in der Ausnahmeliste UND im Vertrag; eines von beiden ist alt");
        assert!(grund.len() > 40, "jede Ausnahme traegt ihren Grund, nicht nur ihren Weg");
    }
}

#[test]
fn jeder_genannte_status_wird_hier_beantwortet() {
    let vertragstext = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&vertragstext).expect("Block");
    let genannt = &samt_ergaenzungen(&block, "status");
    assert!(genannt.len() >= 10, "der Vertrag nennt nur {} Statuswerte", genannt.len());
    let beantwortet = beantwortete_status();
    let fehlen: Vec<&String> = genannt.iter().filter(|s| !beantwortet.contains(*s)).collect();
    assert!(fehlen.is_empty(),
            "der Vertrag verspricht {fehlen:?}, und diese Umsetzung antwortet nie damit");
}

#[test]
fn jeder_status_den_diese_umsetzung_beantwortet_steht_im_vertrag_oder_hat_seinen_grund() {
    let vertragstext = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&vertragstext).expect("Block");
    let alle_status = samt_ergaenzungen(&block, "status");
    let genannt: std::collections::BTreeSet<&String> = alle_status.iter().collect();
    let beantwortet = beantwortete_status();

    // Loch-Melder zuerst: wer nicht alles sieht, darf ueber Fehlendes nichts sagen.
    let fehlen: Vec<&&String> = genannt.iter().filter(|s| !beantwortet.contains(**s)).collect();
    assert!(fehlen.is_empty(),
            "dieser Scan sieht {fehlen:?} nicht, die der Vertrag nennt; sein Schweigen \
ueber Ungenanntes bedeutet damit nichts");

    let mit_grund: std::collections::BTreeSet<&str> =
        AUSSERHALB_DER_LISTE.iter().map(|(s, _)| *s).collect();
    let ungenannt: Vec<&String> = beantwortet.iter()
        .filter(|s| !genannt.contains(s) && !mit_grund.contains(s.as_str()))
        .collect();
    assert!(ungenannt.is_empty(),
            "Statuswerte, die diese Umsetzung beantwortet und die weder im Vertrag \
noch in der Ausnahmeliste stehen: {ungenannt:?}. Ein Geraet trifft dann eine Zahl \
ohne Bedeutung.");

    // Und die andere Richtung der Ausnahmeliste: ein Eintrag ohne Gegenstand
    // ist eine Ausnahme, die niemand mehr nachsieht.
    let ohne_gegenstand: Vec<&str> = AUSSERHALB_DER_LISTE.iter()
        .map(|(s, _)| *s)
        .filter(|s| !beantwortet.contains(*s))
        .collect();
    assert!(ohne_gegenstand.is_empty(),
            "die Ausnahmeliste nennt {ohne_gegenstand:?}, und diese Umsetzung \
antwortet gar nicht damit");
    for (_, grund) in AUSSERHALB_DER_LISTE {
        assert!(grund.len() > 40, "jede Ausnahme traegt ihren Grund, nicht nur ihre Zahl");
    }
}

#[test]
fn jede_genannte_kopfzeile_wird_hier_gelesen() {
    let text = quelltext().to_lowercase();
    let vertragstext = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&vertragstext).expect("Block");
    let kopfzeilen = &samt_ergaenzungen(&block, "headers");
    assert!(kopfzeilen.len() >= 6, "nur {} Kopfzeilen genannt", kopfzeilen.len());
    for k in kopfzeilen {
        assert!(text.contains(&k.to_lowercase()),
                "der Vertrag verspricht die Kopfzeile {k} und hier liest sie niemand");
    }
}

/// Die Ansage des Sockets, GEMESSEN statt im Text gesucht.
///
/// Der erste Anlauf fragte, ob die Woerter `kind`, `scope` und `version`
/// irgendwo in `src/` stehen. Sie stehen dort auch in der Liste der
/// Schriftstuecke, also blieb der Zeuge gruen, als ich das Feld der Ansage
/// in `art` umbenannte: **das Suchmuster griff weiter als die Frage.**
///
/// Jetzt wird eine Ansage wirklich erzeugt und ihre Schluesselmenge gegen die
/// Zeile im Vertrag gehalten. Das ist dieselbe Klasse wie die Handliste
/// darueber: wo man messen kann, sucht man nicht im Text.
#[test]
fn die_nachricht_des_sockets_ist_die_des_vertrags() {
    // Was der Vertrag verspricht, aus seiner Zeile GELESEN, nicht abgetippt.
    let vertragstext = String::from_utf8(vertrag()).expect("UTF-8");
    let zeile = vertragstext.lines()
        .map(|z| z.trim())
        .find(|z| z.starts_with("{\"kind\""))
        .expect("der Vertrag nennt die Gestalt der Ansage");
    let muster: serde_json::Value = serde_json::from_str(zeile).expect("die Zeile ist JSON");
    let mut versprochen: Vec<&str> = muster.as_object().expect("Objekt").keys()
        .map(|s| s.as_str()).collect();
    versprochen.sort();
    assert_eq!(versprochen, ["kind", "scope", "version"],
               "der Vertrag nennt eine andere Gestalt als dieser Zeuge kennt");

    // Und was der Bau wirklich ansagt.
    let wo = std::env::temp_dir().join(format!("vertrag-ansage-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = maer_sync::store::Store::open(
        wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None).expect("Laden");
    laden.create_account("anna", "ein hinreichend langes geheimnis").expect("Konto");
    let zeichen = laden.issue_token("anna", "ein hinreichend langes geheimnis", "g1")
        .expect("Zeichen").expect("da");
    let mut app = maer_sync::wege::App::neu(laden);
    let gab = app.http(&maer_sync::wege::Anfrage::neu("PUT", "/v1/doc/playback")
        .mit_frage("scope=g1")
        .mit_kopf("authorization", &format!("Bearer {zeichen}"))
        .mit_kopf("x-maer-format", "1")
        .mit_koerper(b"{\"a\": 1}"));
    assert_eq!(gab.status, 200, "der Schrieb muss durchgehen, sonst misst dieser Fall nichts");
    assert_eq!(gab.ansagen.len(), 1, "genau eine Ansage je angenommenem Schrieb");

    let gesagt: serde_json::Value =
        serde_json::from_str(&gab.ansagen[0].text).expect("die Ansage ist JSON");
    let mut hat: Vec<&str> = gesagt.as_object().expect("Objekt").keys()
        .map(|s| s.as_str()).collect();
    hat.sort();
    assert_eq!(hat, versprochen,
               "die Ansage dieses Baus hat andere Felder als der Vertrag verspricht");
    let _ = std::fs::remove_file(&wo);
}


// ---------------------------------------------------------------- additions to version 6 (25.11.2026)

/// The service speaks version 6 with the catalog: an addition never raises
/// the number, so every app in the field keeps
/// signing in.
#[test]
fn the_service_speaks_version_6_with_its_additions() {
    assert_eq!(VERSION, "6");
    let text = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&text).expect("Block");
    assert_eq!(block.get("version").map(|v| v.as_slice()), Some(&["6".to_string()][..]));
    assert_eq!(block.get("additions").cloned().unwrap_or_default(), contract::ADDITIONS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(contract::ADDITIONS, ["katalog-1"]);
}

/// An addition names only what version 6 does not: a path, header or status
/// of an addition never stands in the lines of version 6, where the app's
/// fence would ask for it in the app's own code.
#[test]
fn additions_do_not_touch_version_6() {
    let text = String::from_utf8(vertrag()).expect("UTF-8");
    let block = declared(&text).expect("Block");
    for key in ["paths", "headers", "status"] {
        let own: std::collections::BTreeSet<&String> = block.get(key).map(|v| v.iter().collect()).unwrap_or_default();
        let added = block.get(&format!("additions-{key}")).cloned().unwrap_or_default();
        for a in &added {
            assert!(!own.contains(a), "{a} stands in {key} and in additions-{key}");
        }
    }
    let paths = block.get("paths").cloned().unwrap_or_default();
    let added = block.get("additions-paths").cloned().unwrap_or_default();
    assert!(added.contains(&"/v1/katalog".to_string()) && !paths.contains(&"/v1/katalog".to_string()));
}

/// No text of this service calls the catalog "version 7" while the spoken
/// version is 6: the catalog is the
/// addition katalog-1, and a comment that says otherwise is the next wrong
/// bump waiting to happen.
#[test]
fn no_text_names_a_version_the_service_does_not_speak() {
    assert_eq!(VERSION, "6");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = vec![];
    let mut read = 0;
    for dir in ["src", "tests", "abruf/src", "abruf/tests"] {
        // A directory that cannot be read fails the fence: skipped, it would
        // leave the fence green without having looked.
        let entries = std::fs::read_dir(root.join(dir)).unwrap_or_else(|e| panic!("{dir}: {e}"));
        for e in entries {
            let p = e.unwrap().path();
            if p.extension().and_then(|x| x.to_str()) != Some("rs") || p.ends_with("tests/vertrag.rs") { continue; }
            read += 1;
            let text = std::fs::read_to_string(&p).unwrap().to_lowercase();
            for (i, line) in text.lines().enumerate() {
                if line.contains("version 7") || line.contains("fassung 7") {
                    found.push(format!("{}:{}", p.strip_prefix(root).unwrap().display(), i + 1));
                }
            }
        }
    }
    // Measured on 26.11.2026: 73 files in the four directories.
    assert!(read >= 70, "only {read} files were read");
    assert!(found.is_empty(), "texts that name version 7: {found:?}");
}

// ---------------------------------------------------------------- always backwards compatible (27.11.2026)

fn text() -> String {
    String::from_utf8(vertrag()).expect("UTF-8")
}

/// The text of one section, from its heading to the next one.
fn abschnitt(text: &str, heading: &str) -> Option<String> {
    let start = text.find(&format!("\n## {heading}\n"))? + 1;
    let rest = &text[start + 3 + heading.len()..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

#[test]
fn the_contract_carries_the_rule_of_backwards_compatibility() {
    let s = abschnitt(&text(), "Always backwards compatible").expect("the section is missing");
    for n in 1..=4 {
        assert!(s.lines().any(|l| l.starts_with(&format!("{n}. "))), "point {n} is missing");
    }
    assert!(!s.lines().any(|l| l.starts_with("5. ")), "more than four points");
    // The text wraps its lines; a sentence is looked for with its spaces made one.
    let flach = s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flach.contains("states which of the four it touches"), "the line of every delivery is missing");
}

#[test]
fn the_rule_names_its_three_exceptions() {
    let s = abschnitt(&text(), "Always backwards compatible").expect("the section is missing");
    let flach = s.split_whitespace().collect::<Vec<_>>().join(" ");
    for name in ["`/v1/key`", "`X-Maer-Format`", "rolled back"] {
        assert!(flach.contains(name), "the exception {name} is not named");
    }
}

/// The fields a catalog row carries, as the paragraph that begins "A row
/// carries" names them in backticks; the words in brackets after a field
/// (its values) are not fields.
fn felder_im_vertrag(text: &str) -> std::collections::BTreeSet<String> {
    let start = text.find("A row carries").expect("the paragraph of the row");
    let end = start + text[start..].find("Rows come in").expect("the end of the paragraph");
    let mut flat = String::new();
    let mut depth = 0;
    for c in text[start..end].chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => flat.push(c),
            _ => {}
        }
    }
    flat.split('`').skip(1).step_by(2).map(str::to_string).collect()
}

#[test]
fn every_field_of_a_catalog_row_stands_in_the_contract() {
    use maer_sync::katalog::{Katalog, Row};
    let d = std::env::temp_dir().join(format!("vertrag-zeile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let katalog = d.join("katalog.db").to_string_lossy().into_owned();
    let mut k = Katalog::open(&katalog).unwrap();
    // A row with every field set, so the answer shows all of them.
    k.write_run("librivox", &[Row {
        quelle: "librivox".into(), id: "lva:x".into(), art: "hoerbuch".into(), titel: Some("T".into()),
        urheber: Some("U".into()), sprache: Some("eng".into()), jahr: Some(1900), rechte_kennung: "pd".into(),
        rechte_url: Some("http://example.org/pd".into()), laden: true, reihen: vec!["r".into()],
        auch: vec!["a".into()], tief: Some("{\"x\": 1}".into()), beliebt_stufe: Some(1),
    }], false).unwrap();
    k.close_clean().unwrap();
    let mut laden = maer_sync::store::Store::open(
        d.join("s.sqlite").to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None).unwrap();
    laden.create_account("anna", "ein hinreichend langes geheimnis").unwrap();
    let zeichen = laden.issue_token("anna", "ein hinreichend langes geheimnis", "g1").unwrap().unwrap();
    let mut app = maer_sync::wege::App::neu(laden).mit_katalog(&katalog);
    app.plain_ways = true;
    let a = app.http(&maer_sync::wege::Anfrage::neu("GET", "/v1/katalog/librivox")
        .mit_kopf("authorization", &format!("Bearer {zeichen}")));
    assert_eq!(a.status, 200);
    let v = a.als_json().unwrap();
    let geschrieben: std::collections::BTreeSet<String> =
        v["zeilen"][0].as_object().unwrap().keys().cloned().collect();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(felder_im_vertrag(&text()), geschrieben,
               "the fields the contract names for a row and the fields the service writes differ");
}

#[test]
fn a_write_at_an_older_format_is_409_and_the_contract_says_so() {
    let wo = std::env::temp_dir().join(format!("vertrag-form-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&wo);
    let mut laden = maer_sync::store::Store::open(wo.to_str().unwrap(), maer_sync::store::DEFAULT_KEEP, None).unwrap();
    laden.create_account("anna", "ein hinreichend langes geheimnis").unwrap();
    let zeichen = laden.issue_token("anna", "ein hinreichend langes geheimnis", "g1").unwrap().unwrap();
    let mut app = maer_sync::wege::App::neu(laden);
    let schrieb = |form: &str, etag: Option<&str>| {
        let mut a = maer_sync::wege::Anfrage::neu("PUT", "/v1/doc/playback").mit_frage("scope=g1")
            .mit_kopf("authorization", &format!("Bearer {zeichen}"))
            .mit_kopf("x-maer-format", form).mit_koerper(b"{\"a\": 1, \"b\": 2}");
        if let Some(e) = etag {
            a = a.mit_kopf("if-match", e);
        }
        a
    };
    let neu = app.http(&schrieb("2", None));
    assert_eq!(neu.status, 200, "the first write at format 2 must pass, or this case measures nothing");
    let etag = neu.kopf("etag").expect("ETag").to_string();
    let alt = app.http(&schrieb("1", Some(&etag)));
    let _ = std::fs::remove_file(&wo);
    assert_eq!(alt.status, 409, "a write at format 1 over format 2");
    let zeile = text().lines().find(|l| l.starts_with("| 409 |")).expect("the line of 409").to_string();
    assert!(zeile.contains("`X-Maer-Format`"), "the contract names only the shrink for 409: {zeile}");
}
