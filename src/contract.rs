// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Vertrag, von DIESER Seite gemessen.
//!
//! Zwei Seiten, die je nur ihre eigene Datei gegen ihre eigene Konstante
//! halten, bleiben beide gruen, waehrend die Konstanten auseinanderlaufen:
//! **genau das ist am 18.09.2026 passiert**, als ein Satz in den Vertrag kam
//! und der Zweig-Digest von `547d9a75` auf `fc35e150` wanderte, waehrend die
//! App-Seite die alte Zahl hielt. Beide Suiten blieben gruen.
//!
//! Bis die Python-Fassung am 24.09.2026 aus diesem Repo ging, hielt der Zeuge
//! daneben drei Dinge gegeneinander: die Datei, diese Konstante und die der
//! Python-Seite. Jetzt sind es zwei. Was er nicht sehen kann, ist die App: sie
//! liegt in einem anderen Repo, und ihr Nachzug bleibt ein Schritt von Hand.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

/// Der Digest des Vertrags, wie er in beiden Repos steht. Eine Aenderung hier
/// ist eine Aenderung am Vertrag und damit ein Schritt auf `main` mit dem
/// App-Nachzug im selben Zug, nie ein Commit auf einem Bau-Zweig.
pub const DIGEST: &str = "283761495f6fd0abc669ca30ab97fc02370addf3a7353e0fc770acfe32143b55";

/// Die Fassung, die dieser Bau bedient.
pub const VERSION: &str = "6";

pub fn digest_of(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Der maschinenlesbare Block des Vertrags, Name auf Werte.
///
/// Gibt nichts zurueck, wenn der Block fehlt: ein Vertrag ohne ihn ist kein
/// Vertrag, den eine Maschine pruefen kann, und das soll auffallen statt als
/// leere Karte durchzugehen.
pub fn declared(text: &str) -> Option<BTreeMap<String, Vec<String>>> {
    let start = text.find("<!-- maer-sync-contract\n")? + "<!-- maer-sync-contract\n".len();
    let rest = &text[start..];
    let end = rest.find("-->")?;
    let mut aus = BTreeMap::new();
    for zeile in rest[..end].trim().lines() {
        let Some((name, werte)) = zeile.split_once(':') else { continue };
        aus.insert(name.trim().to_string(),
                   werte.split_whitespace().map(|s| s.to_string()).collect());
    }
    if aus.is_empty() { None } else { Some(aus) }
}
