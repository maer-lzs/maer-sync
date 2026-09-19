// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Sitzungsladen, mit eigenen Zeugen.
//!
//! Bis zur Zaehlung auf Fall-Ebene war er nur MITTELBAR gedeckt: der Tunnel
//! siegelt und entsiegelt, also tut es der Laden auch. Das deckt die
//! gewoehnlichen Wege und keinen einzigen der Faelle, die ihn eigentlich
//! ausmachen: dass zwei Sitzungen keine Schluessel teilen, dass ein
//! gescheiterter Handschlag nichts kostet, dass ein Beweis eine lebende
//! Sitzung braucht.
//!
//! Die Uhr ist einspeisbar, sonst waere eine Stunde Stille nicht zu bezeugen,
//! ohne eine Stunde zu warten.

use std::sync::{Arc, Mutex};

use maer_sync::authorization::zufall;
use maer_sync::handshake::HandshakeState;
use maer_sync::identity::public_of;
use maer_sync::sessions::Limits;
use maer_sync::sitzungsladen::{SessionStore, SitzungsFehler};

fn schluessel() -> [u8; 32] {
    let mut k = [0u8; 32];
    k.copy_from_slice(&zufall(32).expect("Zufall"));
    k
}

/// Ein Laden mit einer Uhr, die stillsteht, bis jemand sie stellt.
///
/// **`Arc<Mutex>` und nicht `Rc` mit einem `unsafe impl Send`.** Der erste
/// Anlauf tat genau das, und der Uebersetzer hat ihn trotzdem gefangen (er
/// faengt seit Rust 2021 FELDGENAU ein, also die `Rc` selbst). Das war ein
/// Glueck und keine Vorsicht: eine Zusicherung ueber Fadensicherheit, die
/// nicht stimmt, gehoert in keinen Zeugen, auch nicht in einen einfaedigen.
struct Uhr(Arc<Mutex<f64>>);

impl Uhr {
    fn stellen(&self, wann: f64) { *self.0.lock().expect("Uhr") = wann; }
}

fn laden_mit_uhr(limits: Limits) -> (SessionStore, Uhr) {
    let uhr = Arc::new(Mutex::new(1000.0));
    let lesen = uhr.clone();
    (SessionStore::mit(limits, Box::new(move || *lesen.lock().expect("Uhr"))), Uhr(uhr))
}

/// Einen Handschlag von der Client-Seite, so wie ein Geraet ihn schickt.
fn erste_nachricht(fremd: &[u8; 32], prologue: &[u8]) -> (HandshakeState, Vec<u8>) {
    let mut rufer = HandshakeState::new(true, prologue, None, Some(*fremd), None)
        .expect("Rufer");
    let erst = rufer.write_message(b"").expect("erste Nachricht");
    (rufer, erst)
}

#[test]
fn zwei_sitzungen_teilen_keine_schluessel() {
    // Der Fall, der am teuersten waere, wenn er nicht gilt: eine Nachricht
    // der einen Sitzung darf in der anderen nicht aufgehen, sonst ist die
    // Trennung eine Erzaehlung.
    let privat = schluessel();
    let mut laden = SessionStore::new();

    let (mut r1, e1) = erste_nachricht(&public_of(&privat), b"");
    let (n1, a1, _) = laden.open(&e1, &privat, b"").expect("erste Sitzung");
    r1.read_message(&a1).expect("Antwort");
    let (mut hin1, _) = r1.split();

    let (mut r2, e2) = erste_nachricht(&public_of(&privat), b"");
    let (n2, a2, _) = laden.open(&e2, &privat, b"").expect("zweite Sitzung");
    r2.read_message(&a2).expect("Antwort");
    let (mut hin2, _) = r2.split();

    assert_ne!(n1, n2, "zwei Sitzungen, zwei Namen");

    let brief = hin1.encrypt_with_ad(b"", b"fuer die erste").expect("siegeln");
    assert!(laden.unseal(&n1, &brief, b"").is_ok(), "in ihrer eigenen Sitzung geht er auf");

    let brief2 = hin2.encrypt_with_ad(b"", b"fuer die zweite").expect("siegeln");
    assert!(matches!(laden.unseal(&n1, &brief2, b""), Err(SitzungsFehler::Chiffre(_))),
            "ein Brief der zweiten Sitzung darf in der ersten NICHT aufgehen");
}

#[test]
fn ein_abweichender_prologue_wird_nie_eine_sitzung() {
    // Der Prologue bindet den Handschlag an etwas, das beide schon wissen.
    // Weichen die zwei Seiten darin ab, darf gar nichts entstehen.
    let privat = schluessel();
    let mut laden = SessionStore::new();
    let (_, erst) = erste_nachricht(&public_of(&privat), b"ein anderer anfang");
    assert!(laden.open(&erst, &privat, b"").is_err(),
            "ein Handschlag mit fremdem Prologue wird keine Sitzung");
    assert_eq!(laden.len(), 0, "und er hinterlaesst auch nichts");
}

#[test]
fn ein_gescheiterter_handschlag_kostet_den_laden_nichts() {
    // Das ist der Grund, warum eine gescheiterte Flut kostenlos ist: wer den
    // Austausch nicht zu Ende bringt, hinterlaesst keine Zeile.
    let privat = schluessel();
    let mut laden = SessionStore::new();
    for unsinn in [&b""[..], b"zu kurz", &[0u8; 31][..], &[7u8; 200][..]] {
        assert!(laden.open(unsinn, &privat, b"").is_err(), "das ist kein Handschlag");
    }
    assert_eq!(laden.len(), 0, "und der Laden ist leer geblieben");
}

#[test]
fn eine_echte_nachricht_haelt_die_sitzung_am_leben() {
    // Die Gegenrichtung zu "Rauschen haelt sie nicht am Leben": der Zeitpunkt
    // der Benutzung rueckt bei ERFOLG, und nur dann.
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 100.0, unproven_idle_seconds: 100.0,
                           max_age_seconds: 10_000.0, capacity: 8 };
    let (mut laden, uhr) = laden_mit_uhr(grenzen);
    let (mut rufer, erst) = erste_nachricht(&public_of(&privat), b"");
    let (name, antwort, _) = laden.open(&erst, &privat, b"").expect("Sitzung");
    rufer.read_message(&antwort).expect("Antwort");
    let (mut hin, _) = rufer.split();

    uhr.stellen(1090.0);
    let brief = hin.encrypt_with_ad(b"", b"noch da").expect("siegeln");
    assert!(laden.unseal(&name, &brief, b"").is_ok(), "innerhalb der Frist geht sie auf");

    // Ohne die Benutzung waere sie bei 1101 tot; mit ihr laeuft die Frist neu.
    uhr.stellen(1185.0);
    let brief2 = hin.encrypt_with_ad(b"", b"immer noch").expect("siegeln");
    assert!(laden.unseal(&name, &brief2, b"").is_ok(),
            "die echte Nachricht hat die Frist neu gestellt");

    uhr.stellen(1300.0);
    let brief3 = hin.encrypt_with_ad(b"", b"jetzt nicht mehr").expect("siegeln");
    assert!(matches!(laden.unseal(&name, &brief3, b""), Err(SitzungsFehler::Unbekannt(_))),
            "nach der Frist ist sie zu Ende");
}

#[test]
fn ein_beweis_braucht_eine_lebende_sitzung() {
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 50.0, unproven_idle_seconds: 50.0,
                           max_age_seconds: 10_000.0, capacity: 8 };
    let (mut laden, uhr) = laden_mit_uhr(grenzen);
    let (_, erst) = erste_nachricht(&public_of(&privat), b"");
    let (name, _, _) = laden.open(&erst, &privat, b"").expect("Sitzung");

    assert!(laden.prove(&name).is_ok(), "solange sie lebt, geht es");
    uhr.stellen(1100.0);
    assert!(laden.prove(&name).is_err(), "auf eine tote Sitzung nicht");
    assert!(laden.prove("gibtsnicht").is_err(), "und auf eine erfundene auch nicht");
}

#[test]
fn zwei_sitzungen_teilen_nie_einen_namen() {
    // Der Name ist kein Geheimnis, aber ein RATBARER Name liesse einen
    // Fremden fremde Sitzungen vom Fegen abhalten oder sie verdraengen.
    let privat = schluessel();
    let mut laden = SessionStore::new();
    let mut namen = std::collections::BTreeSet::new();
    for _ in 0..24 {
        let (_, erst) = erste_nachricht(&public_of(&privat), b"");
        let (name, _, _) = laden.open(&erst, &privat, b"").expect("Sitzung");
        assert_eq!(name.len(), 32, "sechzehn Byte als Hex");
        assert!(name.chars().all(|c| c.is_ascii_hexdigit()), "nur Hex: {name}");
        assert!(namen.insert(name), "ein Name kam zweimal");
    }
    assert_eq!(namen.len(), 24);
}

#[test]
fn ein_name_den_niemand_gemacht_hat_ist_unbekannt() {
    let mut laden = SessionStore::new();
    assert!(matches!(laden.unseal("gibtsnicht", b"egal", b""),
                     Err(SitzungsFehler::Unbekannt(_))));
    assert!(matches!(laden.seal("gibtsnicht", b"egal", b""),
                     Err(SitzungsFehler::Unbekannt(_))));
    assert!(!laden.is_live("gibtsnicht"));
}

#[test]
fn eine_beendete_sitzung_wird_auf_der_stelle_fallen_gelassen() {
    // Nicht erst beim naechsten Fegen: sie kann nie wieder benutzt werden,
    // und sie zu halten liesse einen Fremden einen Namen am Leben halten,
    // indem er nach ihm fragt.
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 50.0, unproven_idle_seconds: 50.0,
                           max_age_seconds: 10_000.0, capacity: 8 };
    let (mut laden, uhr) = laden_mit_uhr(grenzen);
    let (_, erst) = erste_nachricht(&public_of(&privat), b"");
    let (name, _, _) = laden.open(&erst, &privat, b"").expect("Sitzung");
    assert_eq!(laden.len(), 1);

    uhr.stellen(1100.0);
    assert!(!laden.is_live(&name), "sie ist zu Ende");
    assert_eq!(laden.len(), 0, "und schon weg, ohne dass jemand gefegt hat");
}

#[test]
fn eine_flut_laesst_den_laden_nicht_wachsen() {
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 10_000.0, unproven_idle_seconds: 10_000.0,
                           max_age_seconds: 100_000.0, capacity: 4 };
    let (mut laden, _uhr) = laden_mit_uhr(grenzen);
    for _ in 0..40 {
        let (_, erst) = erste_nachricht(&public_of(&privat), b"");
        laden.open(&erst, &privat, b"").expect("Sitzung");
        assert!(laden.len() <= 4, "der Laden waechst ueber seine Decke: {}", laden.len());
    }
    assert_eq!(laden.len(), 4, "und er bleibt voll, nicht leer");
}

#[test]
fn eine_flut_drueckt_keine_bewiesene_sitzung_hinaus() {
    // Der Fall, fuer den `give_way` gebaut ist: ein Handschlag weicht vor
    // einer Sitzung, die schon bewiesen hat, wer fragt.
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 10_000.0, unproven_idle_seconds: 10_000.0,
                           max_age_seconds: 100_000.0, capacity: 3 };
    let (mut laden, _uhr) = laden_mit_uhr(grenzen);
    let (_, erst) = erste_nachricht(&public_of(&privat), b"");
    let (bewiesen, _, _) = laden.open(&erst, &privat, b"").expect("Sitzung");
    laden.prove(&bewiesen).expect("beweisen");

    for _ in 0..20 {
        let (_, e) = erste_nachricht(&public_of(&privat), b"");
        laden.open(&e, &privat, b"").expect("Sitzung");
    }
    assert!(laden.is_live(&bewiesen),
            "die bewiesene Sitzung hat die Flut ueberlebt");
}

/// **Rauschen haelt eine Sitzung NICHT am Leben.**
///
/// Dieser Zeuge steht hier, weil eine Rot-Probe gruen blieb: ich hatte den
/// Zeitpunkt der Benutzung VOR das Entsiegeln geschoben, und kein Fall hat es
/// gemerkt. Die uebrigen schicken nur echte Nachrichten, und fuer die ist es
/// gleichgueltig, ob der Zeitpunkt vorher oder nachher rueckt.
///
/// Der Unterschied ist keine Feinheit: ein Fremder, der einen Namen kennt,
/// koennte sonst die Sitzung ihres Besitzers beliebig lange offenhalten,
/// indem er Unsinn dagegen schickt. Die Frist ist dann keine Frist mehr.
#[test]
fn rauschen_haelt_die_sitzung_nicht_am_leben() {
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 100.0, unproven_idle_seconds: 100.0,
                           max_age_seconds: 10_000.0, capacity: 8 };
    let (mut laden, uhr) = laden_mit_uhr(grenzen);
    let (mut rufer, erst) = erste_nachricht(&public_of(&privat), b"");
    let (name, antwort, _) = laden.open(&erst, &privat, b"").expect("Sitzung");
    rufer.read_message(&antwort).expect("Antwort");
    let (mut hin, _) = rufer.split();

    // Kurz vor der Frist: Unsinn unter ihrem Namen. Er wird abgewiesen.
    uhr.stellen(1090.0);
    assert!(matches!(laden.unseal(&name, b"kein siegel", b""),
                     Err(SitzungsFehler::Chiffre(_))),
            "Unsinn geht nicht auf");
    assert!(laden.is_live(&name), "und die Sitzung ueberlebt ihn");

    // GUTFALL ZUERST, sonst misst der Rest nichts: haette der Unsinn die
    // Frist gestellt, waere sie jetzt noch am Leben.
    uhr.stellen(1150.0);
    let brief = hin.encrypt_with_ad(b"", b"echt").expect("siegeln");
    assert!(matches!(laden.unseal(&name, &brief, b""), Err(SitzungsFehler::Unbekannt(_))),
            "der Unsinn darf die Frist NICHT gestellt haben");
}

/// **Eine bewiesene Sitzung ueberlebt die kurze Frist.**
///
/// Die kurze Frist gilt einem Handschlag, der noch nichts bewiesen hat: er
/// soll billig sein und schnell wieder weg. Sobald eine Anmeldung oder ein
/// Zeichen gezeigt hat, WER fragt, gilt die lange. Ohne diesen Unterschied
/// waere entweder jeder Handschlag teuer oder jede echte Sitzung kurzlebig.
#[test]
fn eine_bewiesene_sitzung_ueberlebt_die_kurze_frist() {
    let privat = schluessel();
    let grenzen = Limits { idle_seconds: 500.0, unproven_idle_seconds: 50.0,
                           max_age_seconds: 10_000.0, capacity: 8 };
    let (mut laden, uhr) = laden_mit_uhr(grenzen);

    let (_, e1) = erste_nachricht(&public_of(&privat), b"");
    let (ohne_beweis, _, _) = laden.open(&e1, &privat, b"").expect("Sitzung");
    let (_, e2) = erste_nachricht(&public_of(&privat), b"");
    let (mit_beweis, _, _) = laden.open(&e2, &privat, b"").expect("Sitzung");
    laden.prove(&mit_beweis).expect("beweisen");

    // GUTFALL ZUERST: innerhalb der kurzen Frist leben beide.
    uhr.stellen(1040.0);
    assert!(laden.is_live(&ohne_beweis), "vor der kurzen Frist lebt auch der Handschlag");
    assert!(laden.is_live(&mit_beweis));

    // Danach trennt sich das.
    uhr.stellen(1100.0);
    assert!(!laden.is_live(&ohne_beweis), "der Handschlag faellt nach der kurzen Frist");
    assert!(laden.is_live(&mit_beweis), "die bewiesene Sitzung nicht");

    // Und auch sie ist nicht ewig.
    uhr.stellen(1600.0);
    assert!(!laden.is_live(&mit_beweis), "nach der langen Frist faellt auch sie");
}
