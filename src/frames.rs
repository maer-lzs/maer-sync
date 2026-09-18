// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die Gestalt dessen, was INNEN in einer Sitzung reist.
//!
//! Zwei Byte Kopflaenge big endian, ein kleiner JSON-Kopf, dann der rohe
//! Koerper. Der Koerper geht ausdruecklich nicht durch JSON: Dokumente sind
//! die Masse des Verkehrs, und base64 machte jedes um ein Drittel groesser.

use serde_json::Value;

pub const LENGTH_BYTES: usize = 2;
pub const LONGEST_HEAD: usize = (1 << (8 * LENGTH_BYTES)) - 1;

/// Die Bytes sind kein Rahmen. EIN Fehler fuer jede Art, falsch zu sein: die
/// Antwort des Rufers ist dieselbe, und sie zu unterscheiden beschriebe den
/// Leser demjenigen, der ihn abtastet.
#[derive(Debug, PartialEq, Eq)]
pub struct NotAFrame(pub String);

impl std::fmt::Display for NotAFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Der Kopf, wie ihn beide Seiten schreiben: sortierte Schluessel, keine
/// Leerzeichen. `serde_json` sortiert von sich aus, weil seine Karte eine
/// geordnete ist; das ist hier Vertrag und nicht Zufall.
pub fn head_bytes(head: &Value) -> Result<Vec<u8>, NotAFrame> {
    serde_json::to_vec(head).map_err(|e| NotAFrame(format!("the head is not writable: {e}")))
}

pub fn pack(head: &Value, body: &[u8]) -> Result<Vec<u8>, NotAFrame> {
    let encoded = head_bytes(head)?;
    if encoded.len() > LONGEST_HEAD {
        return Err(NotAFrame(
            "the head is longer than the length field can say".into(),
        ));
    }
    let mut out = Vec::with_capacity(LENGTH_BYTES + encoded.len() + body.len());
    out.extend_from_slice(&(encoded.len() as u16).to_be_bytes());
    out.extend_from_slice(&encoded);
    out.extend_from_slice(body);
    Ok(out)
}

pub fn unpack(frame: &[u8]) -> Result<(Value, Vec<u8>), NotAFrame> {
    if frame.len() < LENGTH_BYTES {
        return Err(NotAFrame("shorter than a length".into()));
    }
    let length = u16::from_be_bytes([frame[0], frame[1]]) as usize;
    let end = LENGTH_BYTES + length;
    if frame.len() < end {
        return Err(NotAFrame("the head is cut short".into()));
    }
    let head: Value = serde_json::from_slice(&frame[LENGTH_BYTES..end])
        .map_err(|_| NotAFrame("the head is not a head".into()))?;
    if !head.is_object() {
        return Err(NotAFrame("the head is not a head".into()));
    }
    Ok((head, frame[end..].to_vec()))
}

/// Die Kopfzeilen eines Kopfs, Namen kleingeschrieben, wie sie vom Draht
/// kommen.
pub fn headers_of(head: &Value) -> Result<std::collections::BTreeMap<String, String>, NotAFrame> {
    let mut aus = std::collections::BTreeMap::new();
    let Some(obj) = head.get("headers") else { return Ok(aus) };
    let Some(map) = obj.as_object() else {
        return Err(NotAFrame("the headers are not headers".into()));
    };
    for (k, v) in map {
        let Some(s) = v.as_str() else {
            return Err(NotAFrame("a header is not a pair of strings".into()));
        };
        aus.insert(k.to_lowercase(), s.to_string());
    }
    Ok(aus)
}

/// Die Methoden, die ein versiegelter Antrag tragen darf.
pub const METHODS: [&str; 3] = ["GET", "PUT", "POST"];

/// Methode, Pfad, Frage und Kopfzeilen aus einem Antrags-Kopf.
///
/// Das Ziel wird am ERSTEN `?` geteilt, so wie ein Server eine Anfragezeile
/// teilt, und die Frage kommt als BYTES zurueck, weil der Rest des Dienstes
/// sie so liest.
pub fn request_of(head: &Value)
    -> Result<(String, String, Vec<u8>, std::collections::BTreeMap<String, String>), NotAFrame>
{
    let method = head.get("method").and_then(|m| m.as_str()).unwrap_or("");
    if !METHODS.contains(&method) {
        return Err(NotAFrame("not a method this service takes".into()));
    }
    let Some(target) = head.get("target").and_then(|t| t.as_str()) else {
        return Err(NotAFrame("not a target".into()));
    };
    if !target.starts_with('/') {
        return Err(NotAFrame("not a target".into()));
    }
    let (pfad, frage) = match target.find('?') {
        Some(i) => (&target[..i], &target[i + 1..]),
        None => (target, ""),
    };
    Ok((method.to_string(), pfad.to_string(), frage.as_bytes().to_vec(), headers_of(head)?))
}

/// Status und Kopfzeilen aus einem Antwort-Kopf.
pub fn reply_of(head: &Value)
    -> Result<(u16, std::collections::BTreeMap<String, String>), NotAFrame>
{
    // Ein Wahrheitswert ist in JSON keine Zahl, in Python aber schon; deshalb
    // steht das hier ausdruecklich, damit beide Seiten `true` gleich abweisen.
    if head.get("status").map(|s| s.is_boolean()).unwrap_or(false) {
        return Err(NotAFrame("not a status".into()));
    }
    let Some(status) = head.get("status").and_then(|s| s.as_u64()) else {
        return Err(NotAFrame("not a status".into()));
    };
    if !(100..=599).contains(&status) {
        return Err(NotAFrame("not a status".into()));
    }
    Ok((status as u16, headers_of(head)?))
}
