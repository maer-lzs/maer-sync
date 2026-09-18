// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die ganze Entscheidung hinter einem Schrieb, an einer Stelle und ohne I/O.
//!
//! Die Tore daneben beantworten je eine Frage. Irgendetwas muss sie in einer
//! ORDNUNG stellen, und die Ordnung ist selbst eine Entscheidung, die Daten
//! verlieren oder einen Klienten verwirren kann. Deshalb liegt sie hier, mit
//! Zeugen, statt in einem Anfrage-Behandler, wo sie wie Installation aussaehe.
//!
//! Warum diese Ordnung:
//!
//! 1. Die Vorbedingung zuerst. Ein Klient mit veralteter Fassung muss hoeren,
//!    dass er neu lesen, mischen und wiederkommen soll, das ist die ganze
//!    Schleife. Kaeme die Schrumpf-Abweisung zuerst, hoerte er "dein Stand ist
//!    zu klein", waehrend die Wahrheit "du siehst einen alten an" ist, und er
//!    wuerde das Falsche reparieren.
//! 2. Das Format als naechstes. Ein Klient, der aelter ist als das Dokument,
//!    darf nicht schreiben, egal wie gross, und er muss diesen Grund hoeren
//!    und keinen Groessen-Grund.
//! 3. Die Groesse zuletzt. Jetzt wissen wir, dass der Schreiber aktuell ist
//!    und die richtige Fassung ansieht, also ist ein Einbruch wirklich einer.

use crate::admission::{admit, DEFAULT_SHRINK_FLOOR};
use crate::formats::write_verdict;

/// Der gespeicherte Stand ist nicht der, den der Klient zuletzt sah.
pub const STALE: u16 = 412;
/// Der Klient darf dieses Dokument in dieser Gestalt gar nicht schreiben.
pub const REFUSED: u16 = 409;
pub const WRITTEN: u16 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDecision {
    pub status: u16,
    pub reason: String,
}

impl WriteDecision {
    pub fn accepted(&self) -> bool { self.status == WRITTEN }
}

/// Alles, was ein Urteil ueber einen Schrieb braucht. Als eigenes Ding, weil
/// acht Stellungsparameter eine Verwechslung sind, die der Uebersetzer nicht
/// findet.
#[derive(Debug, Clone)]
pub struct Write<'a> {
    pub stored_bytes: Option<i64>,
    pub stored_format: Option<i64>,
    pub stored_etag: Option<&'a str>,
    pub incoming_bytes: i64,
    pub writer_format: Option<i64>,
    pub if_match: Option<&'a str>,
    pub declared_shrink: bool,
    pub shrink_floor: f64,
}

impl<'a> Write<'a> {
    pub fn neu(incoming_bytes: i64, writer_format: i64) -> Self {
        Self {
            stored_bytes: None, stored_format: None, stored_etag: None,
            incoming_bytes, writer_format: Some(writer_format), if_match: None,
            declared_shrink: false, shrink_floor: DEFAULT_SHRINK_FLOOR,
        }
    }
}

pub fn decide(w: &Write<'_>) -> Result<WriteDecision, String> {
    let exists = w.stored_etag.is_some();

    if exists {
        let stored_etag = w.stored_etag.expect("gerade geprueft");
        match w.if_match {
            None => {
                // Schweigen ist kein Anspruch, aktuell zu sein. Ein Klient,
                // der ein bestehendes Dokument ueberschreibt, ohne die
                // gelesene Fassung zu nennen, ist genau das blinde
                // Ueberschreiben, gegen das dieser Dienst existiert.
                return Ok(WriteDecision { status: STALE, reason:
                    "this document exists and the write named no version; \
read it, merge, and write again naming what you read".into() });
            }
            Some(im) if im != stored_etag => {
                return Ok(WriteDecision { status: STALE, reason: format!(
                    "version {im} is not the stored one; read it, merge, and write again") });
            }
            Some(_) => {}
        }
    } else if let Some(im) = w.if_match {
        // Ein Klient mit einer Fassung eines Dokuments, das nicht da ist, hat
        // ein Wettrennen gegen ein Loeschen verloren oder spricht mit dem
        // falschen Speicher.
        return Ok(WriteDecision { status: STALE, reason: format!(
            "no such document, but the write named version {im}") });
    }

    let formats = write_verdict(w.stored_format, w.writer_format)?;
    if !formats.accepted {
        return Ok(WriteDecision { status: REFUSED, reason: formats.reason });
    }
    let sizes = admit(w.stored_bytes, w.incoming_bytes, w.declared_shrink, w.shrink_floor)?;
    if !sizes.accepted {
        return Ok(WriteDecision { status: REFUSED, reason: sizes.reason });
    }
    Ok(WriteDecision { status: WRITTEN, reason: sizes.reason })
}
