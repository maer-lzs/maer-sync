// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Das eine Tor, durch das jeder Schrieb geht: darf dieser Stand den
//! gespeicherten ersetzen?
//!
//! Der Verlustfall, den wir wirklich kennen, ist nicht ein kaputtes Update,
//! sondern ein Geraet, das beim Start einen LEEREN Stand schreibt, bevor es
//! geladen hat. Das Tor sieht deshalb auf BYTES und sonst nichts: jedes
//! Stueck Bedeutung im Dienst macht eine spaetere Aenderung teurer, und die
//! Byte-Laenge sieht beide verlustreichen Formen schon, den leeren Schrieb
//! und den ploetzlichen Einbruch.
//!
//! Eine Abweisung ist nie stumm. Jedes Urteil traegt den Satz, der es
//! erklaert; ein Tor, das einen Schrieb wortlos fallen liesse, waere genau
//! das Versagen, gegen das es gebaut ist.

/// Angenommen oder nicht, und immer WARUM, in einem Satz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub accepted: bool,
    pub reason: String,
}

impl Verdict {
    pub(crate) fn ja(reason: String) -> Self { Self { accepted: true, reason } }
    pub(crate) fn nein(reason: String) -> Self { Self { accepted: false, reason } }
}

/// Wie viel eines Standes in einem Schrieb verschwinden darf, bevor das Tor
/// den Klienten fragen will, ob er es gemeint hat. Eine GEWAEHLTE Vorgabe,
/// keine Messung.
pub const DEFAULT_SHRINK_FLOOR: f64 = 0.5;

/// Prozent in der Form, die die Python-Seite schreibt (dort ein Format mit
/// Prozentzeichen und null Nachkommastellen).
fn prozent(anteil: f64) -> String {
    format!("{:.0}%", anteil * 100.0)
}

pub fn admit(
    stored_bytes: Option<i64>,
    incoming_bytes: i64,
    declared_shrink: bool,
    shrink_floor: f64,
) -> Result<Verdict, String> {
    if incoming_bytes < 0 {
        return Err(format!("incoming size is not a size: {incoming_bytes}"));
    }
    if let Some(s) = stored_bytes {
        if s < 0 { return Err(format!("stored size is not a size: {s}")); }
    }
    if !(shrink_floor > 0.0 && shrink_floor <= 1.0) {
        return Err(format!("shrink floor outside (0, 1]: {shrink_floor}"));
    }

    // Nichts gespeichert: es gibt keinen Bestand zu verlieren, und eine
    // Abweisung hier hiesse, dass eine frische Installation nie ihren ersten
    // Stand schreiben koennte.
    let Some(stored) = stored_bytes else {
        return Ok(Verdict::ja(format!("first state, {incoming_bytes} bytes, nothing to lose")));
    };
    if stored == 0 {
        return Ok(Verdict::ja(format!("stored state was empty, {incoming_bytes} bytes accepted")));
    }
    // DER Fall: leer ueber voll. Das ist der Start-Schrieb, und er wird auch
    // dann abgewiesen, wenn er ehrlich gemeint ist, solange der Klient es
    // nicht sagt.
    if incoming_bytes == 0 {
        return Ok(if declared_shrink {
            Verdict::ja(format!("empty state accepted, the client declared it (was {stored} bytes)"))
        } else {
            Verdict::nein(format!("an empty state never replaces a full one ({stored} bytes stored)"))
        });
    }
    let share = incoming_bytes as f64 / stored as f64;
    if share < shrink_floor {
        return Ok(if declared_shrink {
            Verdict::ja(format!(
                "shrink to {} accepted, the client declared it ({stored} to {incoming_bytes} bytes)",
                prozent(share)))
        } else {
            Verdict::nein(format!(
                "state shrank to {}, below the {} floor ({stored} to {incoming_bytes} bytes)",
                prozent(share), prozent(shrink_floor)))
        });
    }
    Ok(Verdict::ja(format!("{incoming_bytes} bytes, {} of the stored state", prozent(share))))
}
