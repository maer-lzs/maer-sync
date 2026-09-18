// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wer darauf wartet zu hoeren, dass sich etwas geaendert hat.
//!
//! **Das ist eine HERAUSLOESUNG, keine Portierung.** In der Python-Fassung
//! steht diese Buchfuehrung nicht in `core/`, sondern als Klasse im
//! ASGI-Behandler (`service/app.py`). Scheibe 5 hat dort keine reine Haelfte:
//! der Socket lebt ganz im Behandler, und was sich ohne Transport ueberhaupt
//! sagen laesst, ist genau dieses Register. Das steht hier, damit die Zaehlung
//! und das Aufraeumen Zeugen bekommen, bevor eine WebSocket-Crate entschieden
//! ist; das Senden selbst ist Transport und fehlt mit Absicht.
//!
//! Die Ansage ist das, was einen eigenen Dienst lohnend macht: die Uebernahme
//! muss ein gesperrtes, spielendes Geraet erreichen, und eine stille Meldung
//! tut das messbar nicht.
//!
//! **Ein Hoerer, der weg ist, wird fallen gelassen und nicht wiederholt.** Eine
//! Ansage ist eine BESCHLEUNIGUNG und nie die einzige Quelle der Wahrheit; ein
//! Geraet, das eine verpasst hat, holt es bei der naechsten Gelegenheit nach.
//! Eine Warteschlange fuer ein abwesendes Geraet machte aus einer
//! Annehmlichkeit eine Last.

use std::collections::BTreeMap;

/// Das Register: je Besitzer die Kennungen der offenen Verbindungen.
///
/// Kennungen statt Sende-Funktionen, weil das Senden Transport ist. Wer eine
/// Verbindung fuehrt, haelt die Kennung und findet seine Funktion selbst.
#[derive(Debug, Default)]
pub struct Listeners {
    nach_besitzer: BTreeMap<String, Vec<String>>,
}

impl Listeners {
    pub fn new() -> Self { Self::default() }

    pub fn add(&mut self, owner: &str, verbindung: &str) {
        self.nach_besitzer.entry(owner.to_string()).or_default().push(verbindung.to_string());
    }

    /// Entfernt eine Verbindung. Ist der Besitzer danach leer, faellt sein
    /// Eintrag ganz weg: ein Register, das leere Listen behaelt, waechst mit
    /// jedem Geraet, das je da war.
    pub fn remove(&mut self, owner: &str, verbindung: &str) {
        let leer = match self.nach_besitzer.get_mut(owner) {
            None => return,
            Some(wartend) => {
                if let Some(i) = wartend.iter().position(|v| v == verbindung) {
                    wartend.remove(i);
                }
                wartend.is_empty()
            }
        };
        if leer { self.nach_besitzer.remove(owner); }
    }

    pub fn count(&self, owner: &str) -> usize {
        self.nach_besitzer.get(owner).map_or(0, |v| v.len())
    }

    /// Wer von einer Aenderung dieses Besitzers erfaehrt.
    pub fn audience(&self, owner: &str) -> Vec<String> {
        self.nach_besitzer.get(owner).cloned().unwrap_or_default()
    }

    /// Wie viele Besitzer ueberhaupt gefuehrt werden. Fuer den Zeugen, der
    /// prueft, dass Aufraeumen wirklich aufraeumt.
    pub fn owners(&self) -> usize { self.nach_besitzer.len() }
}
