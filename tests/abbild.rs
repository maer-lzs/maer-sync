// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Das Betriebsabbild und die Bau-Vorrichtung sprechen von DERSELBEN Umgebung.
//!
//! Ein Dockerfile kann kein anderes einbinden, also stehen der Digest des
//! Grundabbilds und die Fassung von `musl-dev` zweimal da: in `Dockerfile` fuer
//! den Container und in `werkzeug/Dockerfile` fuer `bau.sh`. Zwei Kopien laufen
//! auseinander, sobald jemand eine davon anfasst, und dann baut der Container
//! mit einer anderen Kette als die Suite, die ihn freigegeben hat. Dieser Zeuge
//! haelt beide gegeneinander, und gegen `bau.sh`, das den Digest ein drittes
//! Mal nennt.

fn lesen(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// Der Digest hinter `rust@sha256:` in einem Text, genau einmal, oder ein Halt.
fn digest(text: &str, woher: &str) -> String {
    let treffer: Vec<&str> = text.match_indices("rust@sha256:")
        .map(|(i, m)| &text[i + m.len()..])
        .map(|rest| rest.split(|c: char| !c.is_ascii_hexdigit()).next().unwrap_or(""))
        .collect();
    assert_eq!(treffer.len(), 1, "{woher}: der Digest steht {} mal da", treffer.len());
    assert_eq!(treffer[0].len(), 64, "{woher}: kein ganzer Digest");
    treffer[0].to_string()
}

fn musl_pin(text: &str, woher: &str) -> String {
    // Nur die RUN-Zeile: ein Kommentar, der die Fassung zitiert, ist keine
    // Angabe (der erste Lauf las genau so eine Zeile mit einem Backtick dahinter).
    let zeile = text.lines().find(|z| z.trim_start().starts_with("RUN ") && z.contains("musl-dev="))
        .unwrap_or_else(|| panic!("{woher}: keine gepinnte musl-dev-Fassung"));
    zeile.split("musl-dev=").nth(1).unwrap().split_whitespace().next().unwrap().to_string()
}

#[test]
fn container_und_bau_vorrichtung_haben_denselben_grund() {
    let betrieb = lesen("Dockerfile");
    let bau = lesen("werkzeug/Dockerfile");
    let skript = lesen("werkzeug/bau.sh");
    let d = digest(&betrieb, "Dockerfile");
    assert_eq!(d, digest(&bau, "werkzeug/Dockerfile"), "Container und Bau-Vorrichtung laufen auseinander");
    assert_eq!(d, digest(&skript, "werkzeug/bau.sh"), "bau.sh nennt einen anderen Digest");
    assert_eq!(musl_pin(&betrieb, "Dockerfile"), musl_pin(&bau, "werkzeug/Dockerfile"),
               "musl-dev ist in den zwei Dockerfiles verschieden gepinnt");
}

#[test]
fn das_laufabbild_ist_leer_und_nicht_root() {
    let betrieb = lesen("Dockerfile");
    let stufen: Vec<&str> = betrieb.lines().filter(|z| z.starts_with("FROM ")).collect();
    assert_eq!(stufen.len(), 2, "zwei Stufen, gefunden {stufen:?}");
    assert_eq!(stufen[1].trim(), "FROM scratch", "die letzte Stufe traegt nur das Binary");
    assert!(betrieb.lines().any(|z| z.trim() == "USER 10001:10001"),
            "das Volume gehoert der UID 10001; ohne sie steht der Dienst vor seinem eigenen Laden");
    assert!(betrieb.contains("--chown=10001:10001"), "das Datenverzeichnis gehoert derselben UID");
}

#[test]
fn das_leere_abbild_traegt_was_der_dienst_zur_laufzeit_braucht() {
    // Beides gemessen am 24.09.2026 im ersten Lauf des Abbilds, beides still:
    // ohne /tmp sagte jede Sicherung `not measured`, und ohne Init endete jeder
    // Stopp nach zehn Sekunden mit SIGKILL. Der Text hier ist nur der Zaun; der
    // Beweis ist der Lauf, und der steht im README.
    let befehle = lesen("src/befehle.rs");
    let betrieb = lesen("Dockerfile");
    if befehle.contains("temp_dir()") {
        assert!(betrieb.lines().any(|z| z.starts_with("COPY --from=bau --chown=10001:10001")
                                        && z.trim_end().ends_with(" /tmp")),
                "backup probt die Rueckkehr in temp_dir(): das leere Abbild braucht ein /tmp, \
                 in das die UID des Dienstes schreiben darf");
    }
    let compose = lesen("compose.yaml");
    assert!(compose.lines().any(|z| z.trim() == "init: true"),
            "ohne Init ist der Dienst PID 1, und SIGTERM verpufft");
}

#[test]
fn der_container_setzt_die_variable_die_der_dienst_liest() {
    // Die Falle aus dem README: HOST und PORT liest die Rust-Fassung nicht,
    // und wer sie setzt, bekommt keinen Fehler, sondern einen Dienst auf 8081.
    let quelltext: String = std::fs::read_dir("src").expect("src/")
        .filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "rs").unwrap_or(false))
        .map(|p| lesen(p.to_str().unwrap())).collect();
    assert!(quelltext.contains("\"MAER_SYNC_BIND\""), "der Dienst liest MAER_SYNC_BIND nicht mehr");
    for datei in ["Dockerfile", "compose.yaml"] {
        let t = lesen(datei);
        assert!(t.contains("MAER_SYNC_BIND"), "{datei} setzt MAER_SYNC_BIND nicht");
        for tot in ["MAER_SYNC_HOST", "MAER_SYNC_PORT"] {
            let gesetzt = t.lines().any(|z| {
                let z = z.trim();
                !z.starts_with('#') && (z.starts_with(&format!("{tot}:")) || z.starts_with(&format!("{tot}=")))
            });
            assert!(!gesetzt || quelltext.contains(&format!("\"{tot}\"")),
                    "{datei} setzt {tot}, und der Dienst liest es nicht");
        }
    }
    let compose = lesen("compose.yaml");
    assert!(compose.contains("[\"CMD\", \"/maer-sync\", \"health\"]"),
            "die Pruefung des Containers fragt das Binary selbst");
}
