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
