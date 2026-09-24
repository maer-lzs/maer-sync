// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die Versuchsgrenze an den Anmeldewegen.
//!
//! Bis Fassung 6 gab es keine: ein falsches Geheimnis kostete eine Zeile im
//! Protokoll und sonst nichts. Seit das Passwort beides traegt, die Anmeldung
//! und den Schluessel der Feed-Zugaenge, haengt die Sicherheit gegen Raten
//! allein an der Staerke des Passworts, und Raten ueber die Leitung darf dann
//! nicht beliebig schnell gehen.
//!
//! Gezaehlt wird je PAAR aus Konto und Adresse, und zwar **Fehlversuche**:
//! zehn in einem Fenster von fuenfzehn Minuten, danach ist das Paar bis zum
//! Ende des Fensters gesperrt. Drei Dinge daran sind Absicht:
//!
//! - **Ein unbekanntes Konto zaehlt genauso.** Sonst waere die Grenze eine
//!   Auskunft darueber, wen es gibt.
//! - **Nur das Paar, nicht das Konto allein.** Eine Grenze je Konto ueber alle
//!   Adressen waere ein Hebel, mit dem ein Fremder den Besitzer aussperrt.
//! - **Geprueft wird VOR jeder Ableitung.** Eine Sperre, die erst nach dem
//!   teuren Vergleich greift, spart dem Wirt nichts.
//!
//! Die Buchfuehrung lebt im Speicher und nicht im Laden: nach einem Neustart
//! ist sie leer. Das ist hingenommen, weil ein Neustart selten ist und das
//! Fenster kurz; ein Zaehler im Laden hiesse einen Schrieb je Fehlversuch, und
//! damit waere ein Ratender derjenige, der die Datei waechst.
//!
//! Die Uhr kommt von aussen, als Zahl. So ist jede Entscheidung hier ohne
//! Warten bezeugbar.

use std::collections::HashMap;

/// Wie viele Fehlversuche ein Paar im Fenster hat.
pub const GRENZE: u32 = 10;

/// Wie lang ein Fenster ist, in Sekunden.
pub const FENSTER_S: f64 = 15.0 * 60.0;

/// Wie viele Paare hoechstens gefuehrt werden. Die Menge waechst mit jedem
/// neuen Namen, den ein Ratender versucht; ohne Deckel waere die Grenze
/// selbst der Weg, den Speicher des Wirts zu fuellen.
pub const DECKEL: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct Stand {
    beginn: f64,
    fehler: u32,
}

#[derive(Debug, Default)]
pub struct Versuche {
    paare: HashMap<(String, String), Stand>,
}

impl Versuche {
    pub fn neu() -> Self { Self::default() }

    /// Wie lange das Paar noch gesperrt ist, in ganzen Sekunden, oder nichts.
    ///
    /// Aufgerundet: eine Antwort `Retry-After: 0` hiesse "sofort", und ein
    /// Geraet, das sofort wiederholt, traefe noch die Sperre.
    pub fn gesperrt(&self, konto: &str, adresse: &str, jetzt: f64) -> Option<u64> {
        let s = self.paare.get(&(konto.to_string(), adresse.to_string()))?;
        let ende = s.beginn + FENSTER_S;
        if s.fehler >= GRENZE && jetzt < ende {
            Some((ende - jetzt).ceil().max(1.0) as u64)
        } else {
            None
        }
    }

    /// Ein Fehlversuch. Ein abgelaufenes Fenster faengt neu an.
    pub fn fehlschlag(&mut self, konto: &str, adresse: &str, jetzt: f64) {
        if self.paare.len() >= DECKEL {
            self.aufraeumen(jetzt);
        }
        let s = self.paare.entry((konto.to_string(), adresse.to_string()))
            .or_insert(Stand { beginn: jetzt, fehler: 0 });
        if jetzt >= s.beginn + FENSTER_S {
            *s = Stand { beginn: jetzt, fehler: 0 };
        }
        s.fehler = s.fehler.saturating_add(1);
    }

    /// Eine gelungene Anmeldung setzt das Paar zurueck.
    pub fn erfolg(&mut self, konto: &str, adresse: &str) {
        self.paare.remove(&(konto.to_string(), adresse.to_string()));
    }

    /// Wie viele Paare gefuehrt werden. Fuer Zeugen.
    pub fn anzahl(&self) -> usize { self.paare.len() }

    /// Erst die abgelaufenen Fenster, und reicht das nicht, die aeltesten.
    ///
    /// Die zweite Stufe laesst einen Ratenden die Sperre eines anderen Paares
    /// verdraengen, wenn er sehr viele Namen durchprobiert. Das ist der Preis
    /// des Deckels, und er ist kleiner als ein Wirt ohne Speicher.
    fn aufraeumen(&mut self, jetzt: f64) {
        self.paare.retain(|_, s| jetzt < s.beginn + FENSTER_S);
        if self.paare.len() >= DECKEL {
            // Ungesperrte vor gesperrten, und je die aeltesten zuerst. Ein
            // gesperrtes Paar ist fast immer das aelteste; ginge es zuerst,
            // kaeme ein Gesperrter frei, indem er den Deckel mit neuen Namen
            // fuellt.
            let mut nach_alter: Vec<((String, String), bool, f64)> = self.paare.iter()
                .map(|(k, s)| (k.clone(), s.fehler >= GRENZE, s.beginn)).collect();
            nach_alter.sort_by(|a, b| a.1.cmp(&b.1)
                .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)));
            let weg = self.paare.len() - DECKEL / 2;
            for (k, _, _) in nach_alter.into_iter().take(weg) {
                self.paare.remove(&k);
            }
        }
    }
}

/// Welchen Gegenstellen `X-Real-IP` geglaubt wird, wenn niemand etwas sagt:
/// der Schleife, also genau dem Proxy auf demselben Wirt.
pub const VERTRAUT_VORGABE: &str = "127.0.0.1,::1";

/// Wie der Betreiber die Liste nennt.
pub const VERTRAUT_VAR: &str = "MAER_SYNC_TRUSTED_PROXIES";

/// Die Liste der vertrauten Proxys aus dem Wert der Umgebungsvariablen.
///
/// **Ein Eintrag, der keine Adresse ist, ist ein Fehler und kein Weglassen.**
/// Eine Liste, die still kuerzer wird, glaubt einem Proxy nicht mehr, und die
/// Grenze zaehlt dann wieder alle Geraete unter einer Adresse; das sieht von
/// innen gesund aus. Deshalb bricht der Start ab und nennt den Eintrag. Ein
/// leerer Eintrag zaehlt ebenso als unlesbar, und Netze (`10.0.0.0/8`) gibt es
/// mit Absicht nicht: vertraut wird einer Maschine, keinem Bereich.
pub fn vertraute_aus(wert: Option<&str>) -> Result<Vec<std::net::IpAddr>, String> {
    let text = wert.unwrap_or(VERTRAUT_VORGABE);
    text.split(',')
        .map(|teil| {
            let t = teil.trim();
            t.parse::<std::net::IpAddr>()
                .map_err(|_| format!("{VERTRAUT_VAR}: '{t}' is not an address"))
        })
        .collect()
}

/// Die Adresse, fuer die gezaehlt wird.
///
/// Hinter dem Proxy ist die Gegenstelle der Proxy selbst: auf demselben Wirt
/// die Schleife, vor einem Container das Tor seiner Bruecke. Eine Grenze auf
/// dieser Adresse griffe fuer alle zugleich. Der Proxy setzt `X-Real-IP`.
/// **Geglaubt wird die Kopfzeile nur, wenn die Gegenstelle in `vertraut`
/// steht**: kaeme die Anfrage von woanders, koennte sie sich mit der
/// Kopfzeile jede Adresse geben und die Grenze nach Belieben zuruecksetzen.
///
/// Die Gegenstelle kommt als `ip:port`; gezaehlt wird ohne Port, sonst waere
/// jede neue Verbindung ein neues Paar.
pub fn adresse_aus(gegenstelle: Option<&str>, real_ip: Option<&str>,
                   vertraut: &[std::net::IpAddr]) -> String {
    let ohne_port = gegenstelle.map(ohne_port).unwrap_or_default();
    let geglaubt = ohne_port.parse::<std::net::IpAddr>()
        .map(|ip| vertraut.contains(&ip))
        .unwrap_or(false);
    match real_ip.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(ip) if geglaubt => ip.to_string(),
        _ => if ohne_port.is_empty() { "-".to_string() } else { ohne_port },
    }
}

fn ohne_port(s: &str) -> String {
    // `[::1]:4711` und `1.2.3.4:4711`. Eine nackte IPv6-Adresse ohne Klammern
    // hat mehrere Doppelpunkte und keinen Port.
    if let Some(rest) = s.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("").to_string();
    }
    if s.matches(':').count() == 1 {
        return s.split(':').next().unwrap_or("").to_string();
    }
    s.to_string()
}
