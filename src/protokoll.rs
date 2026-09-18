// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Was der Betreiber sieht, wenn etwas abgewiesen wird.
//!
//! **Regel 3: kein Fehler verschwindet spurlos.** Ohne diese Zeilen ist der
//! Fall, für den der Dienst gebaut ist (ein Gerät schreibt leer, wird
//! abgewiesen, der Klient schluckt die 409), für den Betreiber unsichtbar,
//! solange er dauert.
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
//! Zeugen davon abhängig, einen Strom abzufangen.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zeile {
    pub stufe: Stufe,
    pub satz: String,
}

impl Zeile {
    pub fn warnung(satz: String) -> Self { Self { stufe: Stufe::Warnung, satz } }
    pub fn hinweis(satz: String) -> Self { Self { stufe: Stufe::Hinweis, satz } }
}

/// Die Quelle, wie sie in der Zeile steht, oder ein Strich.
///
/// Ein Strich und nicht das Weglassen des Feldes: eine Zeile ohne `from=`
/// liesse den Leser raten, ob die Quelle unbekannt war oder das Feld vergessen
/// wurde.
pub fn woher(client: Option<&str>) -> String {
    client.unwrap_or("-").to_string()
}
