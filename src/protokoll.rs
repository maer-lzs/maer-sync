// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Was der Betreiber sieht: jede Abweisung, und seit Scheibe 6 Teil 6 jeder
//! Zugriff.
//!
//! **Regel 3: kein Fehler verschwindet spurlos.** Ohne diese Zeilen ist der
//! Fall, für den der Dienst gebaut ist (ein Gerät schreibt leer, wird
//! abgewiesen, der Klient schluckt die 409), für den Betreiber unsichtbar,
//! solange er dauert.
//!
//! **Und seit dem 19.09.2026 die zweite Hälfte: kein Zugriff verschwindet
//! spurlos.** Gemessen an der Python-Fassung: ihr Zugriffsprotokoll sieht nur
//! den Umschlag (`POST /v1/session 200`, hundertfach), nie den inneren Weg,
//! und Erfolge nie. Ein Betreiber, der wissen will, ob ein Gerät seine Wahl
//! geschrieben hat, findet dort nichts. Deshalb nennt hier jede Zugriffszeile
//! den INNEREN Weg: Art, Bereich, Status, bei Abweisung den Grund. Der
//! Umschlag selbst bekommt nur dann eine Zeile, wenn ER scheitert.
//!
//! **Und was eine Zeile NIE nennt: ein Zeichen, ein Geheimnis, einen Körper.**
//! Das Protokoll ist die Stelle, an der ein Wert in Dateien wandert, die
//! jemand anders liest, in Sicherungen und in Fehlerberichte. Ein Zaun über
//! dem Quelltext hält das, nicht die Aufmerksamkeit beim Schreiben.
//!
//! **Die Zeilen reisen, sie werden nicht geschrieben.** Genau wie die Ansage:
//! die Entscheidung, dass etwas zu melden ist, fällt hier und ist ohne Netz
//! prüfbar; wer sie wegträgt und in einen Strom schreibt, ist der Transport.
//! Ein `eprintln!` mitten in der Entscheidung wäre bequem und machte jeden
//! Zeugen davon abhängig, einen Strom abzufangen. Der Transport stempelt beim
//! Wegtragen die Zeit und, bei einem Zugriff, die Dauer: beides kennt nur er.

/// Die zwei Stufen, die dieser Dienst benutzt. Woertlich die Namen, die
/// Pythons `logging` in die Zeile schreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stufe {
    /// Etwas wurde abgewiesen. Der Betreiber soll es sehen.
    Warnung,
    /// Etwas ist geschehen, das erwartbar ist und trotzdem zählt.
    Hinweis,
}

impl Stufe {
    /// Wie Pythons `logging` sie nennt.
    pub fn name(&self) -> &'static str {
        match self {
            Stufe::Warnung => "WARNING",
            Stufe::Hinweis => "INFO",
        }
    }
}

/// Wovon eine Zeile handelt. Die Zeugen der Meldungen zählen gegen die an
/// Python gemessene Tafel, und Python kennt keine Zugriffszeilen; ohne diese
/// Trennung würde jede Zugriffszeile dort als Abweichung zählen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// Eine Abweisung oder ein Ereignis, wie Python sie schreibt.
    Meldung,
    /// Ein Zugriff, mit seinem inneren Weg. Nur diese Sorte bekommt vom
    /// Transport eine Dauer.
    Zugriff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zeile {
    pub stufe: Stufe,
    pub art: Art,
    pub satz: String,
}

impl Zeile {
    pub fn warnung(satz: String) -> Self { Self { stufe: Stufe::Warnung, art: Art::Meldung, satz } }
    pub fn hinweis(satz: String) -> Self { Self { stufe: Stufe::Hinweis, art: Art::Meldung, satz } }
    /// Ein Zugriff. Der Satz beginnt mit `access ` und trägt den inneren Weg;
    /// gebaut wird er nur in `wege.rs`, damit der Zaun über dem Quelltext ihn
    /// sieht.
    pub fn zugriff(satz: String) -> Self { Self { stufe: Stufe::Hinweis, art: Art::Zugriff, satz } }
}

/// Die Quelle, wie sie in der Zeile steht, oder ein Strich.
///
/// Ein Strich und nicht das Weglassen des Feldes: eine Zeile ohne `from=`
/// liesse den Leser raten, ob die Quelle unbekannt war oder das Feld vergessen
/// wurde.
pub fn woher(client: Option<&str>) -> String {
    client.unwrap_or("-").to_string()
}

/// Ein Sitzungsname, wie er in einer Zeile steht: die ersten acht Zeichen,
/// oder ein Strich. Der Name ist kein Geheimnis (er reist im Klaren im Kopf
/// der Anfrage), aber er ist lang, und acht Zeichen reichen, um zwei
/// Sitzungen in einem Protokoll auseinanderzuhalten.
pub fn sitzung_kurz(name: Option<&str>) -> String {
    match name {
        Some(n) if n.len() >= 8 => n[..8].to_string(),
        Some(n) if !n.is_empty() => n.to_string(),
        _ => "-".to_string(),
    }
}

// ------------------------------------------------------------ Die Uhr

/// Ein Zeitpunkt als ISO-8601-Text in UTC, mit Millisekunden und dem `Z`.
///
/// **Die Zone steht IM Stempel.** Die Python-Fassung schreibt Lokalzeit ohne
/// Zone, und genau daran ist am 16.09.2026 eine Messung um zwei Stunden
/// danebengegangen. Ein Stempel, der seine Zone nennt, ist auf jedem Rechner
/// dieselbe Zahl.
///
/// **Von Hand statt mit `chrono`:** die Umrechnung von Sekunden seit 1970 in
/// ein Datum ist zwanzig Zeilen (Howard Hinnants `civil_from_days`), und eine
/// Crate dafuer waere eine Aufnahme (Regel 4) fuer eine Rechnung, die sich an
/// bekannten Tagen bezeugen laesst.
pub fn zeitstempel(secs: u64, nanos: u32) -> String {
    let tage = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let (jahr, monat, tag) = datum_aus_tagen(tage);
    format!("{jahr:04}-{monat:02}-{tag:02}T{:02}:{:02}:{:02}.{:03}Z",
            rest / 3600, (rest % 3600) / 60, rest % 60, nanos / 1_000_000)
}

/// Jetzt, nach der Uhr des Wirts. Nur der Transport ruft das: die
/// Entscheidung kennt keine Uhr.
pub fn jetzt() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => zeitstempel(d.as_secs(), d.subsec_nanos()),
        // Eine Uhr vor 1970 ist keine Uhr. Der Stempel sagt das, statt zu raten.
        Err(_) => "0000-00-00T00:00:00.000Z".to_string(),
    }
}

/// Tage seit 1970-01-01 in (Jahr, Monat, Tag), proleptisch gregorianisch.
/// Howard Hinnant, `civil_from_days`, unveraendert uebertragen.
fn datum_aus_tagen(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;                                   // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;      // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);                      // [0, 365]
    let mp = (5 * doy + 2) / 153;                                           // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;                          // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;                   // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod zeit_zeugen {
    use super::*;

    /// Bekannte Tage, von Hand nachgerechnet, nicht aus derselben Funktion.
    #[test]
    fn bekannte_zeitpunkte() {
        assert_eq!(zeitstempel(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(zeitstempel(951_782_400, 0), "2000-02-29T00:00:00.000Z");      // Schaltjahr, durch 400 teilbar
        assert_eq!(zeitstempel(1_709_164_800, 0), "2024-02-29T00:00:00.000Z");    // Schaltjahr
        assert_eq!(zeitstempel(1_709_251_200, 0), "2024-03-01T00:00:00.000Z");    // der Tag danach
        assert_eq!(zeitstempel(1_789_831_200, 0), "2026-09-19T15:20:00.000Z");    // heute; `date -u -d @1789831200`
        assert_eq!(zeitstempel(1_789_831_200, 999_999_999), "2026-09-19T15:20:00.999Z"); // Millisekunden gekappt, nicht gerundet
        assert_eq!(zeitstempel(2_147_483_647, 0), "2038-01-19T03:14:07.000Z");    // 32-Bit-Grenze, keine Sonderrolle
    }

    /// Jeder Stempel hat dieselbe Gestalt und dieselbe Laenge, damit ein
    /// Leser ihn mit einem festen Schnitt abtrennen kann.
    #[test]
    fn gestalt_ist_fest() {
        let s = jetzt();
        assert_eq!(s.len(), 24, "{s}");
        assert_eq!(&s[4..5], "-"); assert_eq!(&s[10..11], "T"); assert_eq!(&s[19..20], "."); assert_eq!(&s[23..], "Z");
        assert!(s.starts_with("20"), "die Uhr des Wirts steht in einem anderen Jahrhundert: {s}");
    }

    #[test]
    fn sitzungsname_wird_gekuerzt_und_nie_geraten() {
        assert_eq!(sitzung_kurz(None), "-");
        assert_eq!(sitzung_kurz(Some("")), "-");
        assert_eq!(sitzung_kurz(Some("abc")), "abc");
        assert_eq!(sitzung_kurz(Some("0123456789abcdef0123456789abcdef")), "01234567");
    }
}
