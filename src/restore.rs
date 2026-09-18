// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Das Urteil ueber eine geprobte Rueckkehr: ist die Sicherung wirklich
//! zurueckgekommen?
//!
//! "Eine Sicherung, die nie zurueckgespielt wurde, ist keine." Deshalb oeffnet
//! der Sicherungsbefehl im selben Lauf seine eigene Kopie in einem
//! Wegwerf-Speicher und fragt hier, ob sie passt.
//!
//! **Drei Ausgaenge, und der dritte ist der Punkt.** Eine Probe, die nicht
//! laufen konnte, ist KEINE bestandene Probe. Das ist die Regel-7-Gestalt in
//! ihrer reinsten Form: ein Fehlschlag, der wie eine Messung aussieht. Was
//! zwei Ausgaenge meldet, meldet eines Tages "passt" fuer eine Probe, die nie
//! stattgefunden hat.
//!
//! Verglichen werden GROESSEN je Kennung, nicht Inhalte: der Dienst liest
//! nicht, was er haelt, und ein Dokument, das mit der richtigen Kennung und
//! der richtigen Laenge zurueckkommt, ist so weit, wie ein Byte-Speicher
//! ehrlich aussagen kann.

use std::collections::BTreeMap;

pub const FITS: &str = "fits";
pub const DIFFERS: &str = "differs";
pub const NOT_MEASURED: &str = "not_measured";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreVerdict {
    pub outcome: String,
    pub reason: String,
}

impl RestoreVerdict {
    /// Nur `FITS` besteht. `NOT_MEASURED` besteht NICHT, und diese eine
    /// Stelle koennte versucht sein, es anders zu sehen.
    pub fn passed(&self) -> bool { self.outcome == FITS }
    fn neu(outcome: &str, reason: String) -> Self {
        Self { outcome: outcome.to_string(), reason }
    }
}

/// Urteilt ueber eine Probe.
///
/// Die beiden Karten bilden Dokumentkennung auf Byte-Laenge ab: was die
/// Sicherung zu halten behauptete, und was aus dem Wegwerf-Speicher kam.
pub fn restore_verdict(
    backed_up: Option<&BTreeMap<String, i64>>,
    read_back: Option<&BTreeMap<String, i64>>,
    error: Option<&str>,
) -> RestoreVerdict {
    if let Some(e) = error.filter(|e| !e.is_empty()) {
        return RestoreVerdict::neu(NOT_MEASURED,
            format!("not measured, the rehearsal stopped: {e}"));
    }
    let (Some(aus), Some(zurueck)) = (backed_up, read_back) else {
        return RestoreVerdict::neu(NOT_MEASURED,
            "not measured, the rehearsal handed back nothing to compare".into());
    };
    // Eine Probe auf einer leeren Sicherung vergleicht nichts mit nichts und
    // kaeme gruen heraus. Das ist der Fehlschlag, der wie eine Messung
    // aussieht, also bekommt er seine eigene Antwort.
    if aus.is_empty() {
        return RestoreVerdict::neu(NOT_MEASURED, "not measured, the backup held no documents".into());
    }

    let fehlend: Vec<&String> = aus.keys().filter(|k| !zurueck.contains_key(*k)).collect();
    if !fehlend.is_empty() {
        let erste: Vec<&str> = fehlend.iter().take(5).map(|s| s.as_str()).collect();
        return RestoreVerdict::neu(DIFFERS, format!(
            "{} of {} documents did not come back: {}",
            fehlend.len(), aus.len(), erste.join(", ")));
    }
    let zuviel: Vec<&String> = zurueck.keys().filter(|k| !aus.contains_key(*k)).collect();
    if !zuviel.is_empty() {
        let erste: Vec<&str> = zuviel.iter().take(5).map(|s| s.as_str()).collect();
        return RestoreVerdict::neu(DIFFERS, format!(
            "{} documents came back that were not backed up: {}",
            zuviel.len(), erste.join(", ")));
    }
    let falsch: Vec<&String> = aus.iter()
        .filter(|(k, v)| zurueck.get(*k) != Some(v))
        .map(|(k, _)| k)
        .collect();
    if !falsch.is_empty() {
        let erste = falsch[0];
        return RestoreVerdict::neu(DIFFERS, format!(
            "{} of {} documents came back at a different size, first {}: {} bytes out, {} bytes back",
            falsch.len(), aus.len(), erste, aus[erste], zurueck[erste]));
    }
    let gesamt: i64 = aus.values().sum();
    RestoreVerdict::neu(FITS, format!(
        "{} documents, {} bytes, came back unchanged", aus.len(), gesamt))
}
