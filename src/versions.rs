// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Welche gespeicherten Fassungen einen Schrieb ueberleben, und welche gehen.
//!
//! Die Forderung sagt "versionierte Staende statt einer Sicherung", und sie
//! sagt warum: EINE Konstruktion deckt beide Verluste, die wir benennen
//! koennen, ein kaputtes Update und einen Klienten, der Unsinn geschrieben
//! hat. Eine Sicherung, die einmal nachts laeuft, deckt innerhalb der Nacht
//! keinen von beiden.

/// Teilt die Fassungen in das, was bleibt, und das, was geht.
///
/// Die Kennungen kommen aeltestes zuerst, so wie der Speicher sie
/// zurueckgibt. Beide Rueckgaben behalten diese Ordnung.
pub fn rotate(version_ids: &[String], keep: i64) -> Result<(Vec<String>, Vec<String>), String> {
    if keep < 1 {
        // Null Fassungen zu behalten hiesse, den gerade geschriebenen Stand
        // zu loeschen. Ein Speicher, der nichts haelt, ist keiner, und der
        // Rufer hat einen Fehler und keine Vorliebe.
        return Err(format!("a store keeps at least one version, asked for {keep}"));
    }
    let ids: Vec<String> = version_ids.to_vec();
    if ids.len() as i64 <= keep {
        return Ok((ids, Vec::new()));
    }
    let cut = ids.len() - keep as usize;
    Ok((ids[cut..].to_vec(), ids[..cut].to_vec()))
}
