// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wozu eine Fassungsnummer da ist: einen Rueckweg sicher zu machen.
//!
//! ZWEI Nummern leben hier, und sie duerfen nicht verwechselt werden. Die
//! DOKUMENT-Fassung gehoert dem Klienten: der Dienst liest kein Dokument,
//! kann also selbst kein Feld verlieren, aber er kann ein Geraet anhalten,
//! das es wuerde. Die SPEICHER-Fassung gehoert dem Dienst: ein
//! zurueckgerollter Dienst wandert nicht rueckwaerts und raet nicht, er
//! verweigert den Betrieb und sagt es.

use crate::admission::Verdict;

/// Darf ein Klient mit writer_format ein Dokument in stored_format
/// ueberschreiben?
pub fn write_verdict(stored_format: Option<i64>, writer_format: Option<i64>) -> Result<Verdict, String> {
    let Some(writer) = writer_format else {
        return Err("a writer names its format: None".into());
    };
    if writer < 1 { return Err(format!("a writer names its format: {writer}")); }
    if let Some(s) = stored_format {
        if s < 1 { return Err(format!("a stored document names its format: {s}")); }
    }
    let Some(stored) = stored_format else {
        return Ok(Verdict::ja(format!("first state at format {writer}")));
    };
    if writer >= stored {
        return Ok(Verdict::ja(format!("format {writer} over {stored}")));
    }
    Ok(Verdict::nein(format!(
        "a client at format {writer} must not overwrite a document written at format {stored}: \
the newer form carries fields the older one would drop")))
}

/// Darf ein Dienst mit service_schema einen Speicher mit store_schema oeffnen?
pub fn serve_verdict(store_schema: Option<i64>, service_schema: Option<i64>) -> Result<Verdict, String> {
    let Some(service) = service_schema else {
        return Err("the service names its schema: None".into());
    };
    if service < 1 { return Err(format!("the service names its schema: {service}")); }
    let Some(store) = store_schema else {
        return Ok(Verdict::ja(format!("empty store, laid out at schema {service}")));
    };
    if store < 1 { return Err(format!("the store names its schema: {store}")); }
    if store <= service {
        return Ok(Verdict::ja(format!("store at schema {store}, service at {service}")));
    }
    Ok(Verdict::nein(format!(
        "the store is at schema {store} and this service only knows {service}: \
a rolled-back service does not migrate backwards")))
}
