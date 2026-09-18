// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 5: der Horcher, als Zustandsmaschine.
//!
//! Die Ansage ist das, was einen eigenen Dienst lohnt: die Uebernahme muss ein
//! gesperrtes, spielendes Geraet erreichen, und ein stiller Push tut das
//! messbar nicht. Deshalb ist dieser Teil kein Beiwerk, und deshalb steht er
//! hier als Maschine und nicht als Schleife um einen Socket: **Woerter herein,
//! Taten hinaus.** Wer ihn pruefen will, braucht kein Netz.
//!
//! Der Dienst antwortet hier im Fehlerfall mit einer ZAHL und sonst nichts.
//! Ein falscher Schliesscode ist auf dem Geraet von keinem anderen zu
//! unterscheiden, also steht die Tabelle in `vektoren/horcher.json`, an der
//! Python-Fassung gemessen.
//!
//! **Die Asymmetrie ist die Sache, nicht ein Detail.** Ein Horcher im Klaren
//! wird VOR dem Annehmen geschlossen, wenn sein Zeichen nicht taugt: erst
//! anzunehmen und dann zu schliessen setzte einen Fremden so lange auf die
//! Ansageliste, wie das dauert. Ein versiegelter Horcher kann das nicht, weil
//! das Zeichen in der Aufstiegsanfrage nicht reisen kann (sie ist das eine
//! Ungesiegelte auf diesem Weg). Er nimmt also erst an, verlangt als ERSTES
//! Wort einen versiegelten Rahmen mit dem Zeichen, und kommt erst dann auf die
//! Liste. Ein Fremder mit erfundenem Sitzungsnamen wird trotzdem vor dem
//! Annehmen abgewiesen.

use std::collections::BTreeMap;

use crate::authorization::caller_for;
use crate::frames::{request_of, unpack};
use crate::protokoll::{woher, Zeile};
use crate::sitzungsladen::SitzungsFehler;
use crate::wege::{Ansage, App};

/// Der Weg, auf dem angesagt wird. Der einzige.
pub const WEG: &str = "/v1/changes";

/// Nicht dieser Weg.
pub const NICHT_DER_WEG: u16 = 4004;
/// Kein Zeichen, oder eines, das niemandem gehoert.
pub const NICHT_BERECHTIGT: u16 = 4401;
/// Dieser Dienst spricht nur innerhalb einer Sitzung.
pub const NUR_INNEN: u16 = 4426;
/// Diese Sitzung gibt es nicht mehr.
pub const SITZUNG_ZU_ENDE: u16 = 4410;
/// Das war kein versiegelter Rahmen.
pub const KEIN_RAHMEN: u16 = 4400;

/// Was von der Leitung hereinkommt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wort {
    Verbunden,
    Text(String),
    Bytes(Vec<u8>),
    Getrennt,
}

/// Was hinausgeht. Wer sie wegtraegt, spricht das Protokoll; diese Maschine
/// nicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tat {
    Annehmen,
    Schliessen(u16),
    SendeText(String),
    SendeBytes(Vec<u8>),
}

impl Tat {
    /// Der ASGI-Name der Tat. Nur damit die gemessenen Vektoren und diese
    /// Maschine dieselbe Sprache sprechen.
    pub fn art(&self) -> &'static str {
        match self {
            Tat::Annehmen => "websocket.accept",
            Tat::Schliessen(_) => "websocket.close",
            Tat::SendeText(_) | Tat::SendeBytes(_) => "websocket.send",
        }
    }
}

/// Wie ein Horcher seine Ansage bekommen will.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zustellart {
    /// Als Text, wie die aelteren Geraete es kennen.
    Klar,
    /// Versiegelt in DIESER Sitzung.
    Versiegelt(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zustand {
    /// Abgewiesen, bevor irgendetwas geschah.
    Zu,
    /// Wartet auf das Verbinden, danach wird angenommen und eingetragen.
    WartetImKlaren,
    /// Wartet auf das Verbinden, danach wird angenommen, aber NICHT
    /// eingetragen: es fehlt noch der erste versiegelte Rahmen.
    WartetVersiegelt,
    /// Angenommen, wartet auf den ersten versiegelten Rahmen.
    WartetAufDenErstenRahmen,
    /// Auf der Liste.
    Offen,
}

pub struct Horcher {
    zustand: Zustand,
    /// Woher der Rufer kommt, fuer die Protokollzeilen nach dem Annehmen.
    von: String,
    /// Der Name, unter dem dieser Horcher auf der Liste steht. Aus derselben
    /// Quelle wie ein Sitzungsname, damit zwei Horcher desselben Besitzers
    /// sich nicht gegenseitig austragen.
    pub id: String,
    besitzer: Option<String>,
    sitzung: Option<String>,
}

impl Horcher {
    pub fn ist_offen(&self) -> bool { self.zustand == Zustand::Offen }
    pub fn besitzer(&self) -> Option<&str> { self.besitzer.as_deref() }
}

impl App {
    /// Einen Horcher eroeffnen. Die Abweisungen fallen HIER, vor dem ersten
    /// Wort: in der Python-Fassung steht das Schliessen ebenso vor dem
    /// `receive`, und das ist kein Zufall, sondern der Punkt.
    pub fn horcher_oeffnen(&mut self, pfad: &str, kopfzeilen: &BTreeMap<String, String>,
                           client: Option<&str>)
        -> (Horcher, Vec<Tat>)
    {
        // Die Quelle kommt als Argument, nicht aus einer Kopfzeile. Eine
        // Kopfzeile schreibt der RUFER, und eine Protokollzeile, deren
        // Herkunftsfeld der Rufer selbst setzen kann, ist keine Auskunft
        // ueber ihn, sondern eine von ihm.
        let von = woher(client);
        let id = crate::authorization::hex(
            &crate::authorization::zufall(16).unwrap_or_else(|_| vec![0; 16]));
        let zu = |grund: u16| (
            Horcher { zustand: Zustand::Zu, id: id.clone(), von: von.clone(),
                      besitzer: None, sitzung: None },
            vec![Tat::Schliessen(grund)],
        );

        if pfad != WEG {
            return zu(NICHT_DER_WEG);
        }
        if let Some(name) = kopfzeilen.get("x-maer-session") {
            if self.identity.is_none() || !self.sessions.is_live(name) {
                return zu(SITZUNG_ZU_ENDE);
            }
            return (Horcher {
                zustand: Zustand::WartetVersiegelt, id, von,
                besitzer: None, sitzung: Some(name.clone()),
            }, vec![]);
        }
        if !self.plain_ways {
            return zu(NUR_INNEN);
        }
        // Ein unberechtigter Socket wird geschlossen, BEVOR er angenommen ist.
        let caller = {
            let laden = &self.store;
            caller_for(kopfzeilen.get("authorization").map(|s| s.as_str()),
                       |d| laden.owner_of_token(d).ok().flatten())
        };
        if !caller.authorized() {
            self.protokoll.push(Zeile::warnung(format!("unauthorized socket from={von}")));
            return zu(NICHT_BERECHTIGT);
        }
        (Horcher {
            zustand: Zustand::WartetImKlaren, id, von,
            besitzer: caller.owner, sitzung: None,
        }, vec![])
    }

    /// Ein Wort von der Leitung, und was daraus folgt.
    pub fn horcher_wort(&mut self, h: &mut Horcher, wort: Wort) -> Vec<Tat> {
        match (h.zustand, &wort) {
            (Zustand::Zu, _) => vec![],

            (Zustand::WartetImKlaren, Wort::Verbunden) => {
                h.zustand = Zustand::Offen;
                let besitzer = h.besitzer.clone().expect("im Klaren steht der Besitzer fest");
                self.listeners.add(&besitzer, &h.id);
                self.zustellarten.insert(h.id.clone(), Zustellart::Klar);
                vec![Tat::Annehmen]
            }
            (Zustand::WartetVersiegelt, Wort::Verbunden) => {
                h.zustand = Zustand::WartetAufDenErstenRahmen;
                vec![Tat::Annehmen]
            }
            // Irgendetwas anderes als ein Verbinden ist kein Anfang. Die
            // Python-Fassung kehrt dann wortlos zurueck, ohne zu schliessen.
            (Zustand::WartetImKlaren, _) | (Zustand::WartetVersiegelt, _) => {
                h.zustand = Zustand::Zu;
                vec![]
            }

            (Zustand::WartetAufDenErstenRahmen, Wort::Bytes(roh)) => {
                let name = h.sitzung.clone().expect("versiegelt heisst: mit Sitzung");
                let klar = match self.sessions.unseal(&name, roh, b"") {
                    Ok(k) => k,
                    Err(SitzungsFehler::Unbekannt(_)) => return Self::schliessen(h, SITZUNG_ZU_ENDE),
                    Err(SitzungsFehler::Chiffre(_)) => return Self::schliessen(h, KEIN_RAHMEN),
                };
                let Ok((kopf, _)) = unpack(&klar) else {
                    self.protokoll.push(Zeile::warnung(format!(
                        "sealed socket refused from={}", h.von)));
                    return Self::schliessen(h, KEIN_RAHMEN);
                };
                let Ok((_, _, _, innere)) = request_of(&kopf) else {
                    return Self::schliessen(h, KEIN_RAHMEN);
                };
                let caller = {
                    let laden = &self.store;
                    caller_for(innere.get("authorization").map(|s| s.as_str()),
                               |d| laden.owner_of_token(d).ok().flatten())
                };
                if !caller.authorized() {
                    self.protokoll.push(Zeile::warnung(format!(
                        "unauthorized sealed socket from={}", h.von)));
                    return Self::schliessen(h, NICHT_BERECHTIGT);
                }
                if self.sessions.prove(&name).is_err() {
                    return Self::schliessen(h, SITZUNG_ZU_ENDE);
                }
                let besitzer = caller.owner.expect("berechtigt heisst: es gibt einen Besitzer");
                h.besitzer = Some(besitzer.clone());
                h.zustand = Zustand::Offen;
                self.listeners.add(&besitzer, &h.id);
                self.zustellarten.insert(h.id.clone(), Zustellart::Versiegelt(name));
                vec![]
            }
            // Ein erstes Wort, das keine Bytes ist, ist kein versiegelter
            // Rahmen. Text faellt hier hinein, und das ist Absicht: ein
            // Horcher innerhalb einer Sitzung spricht nur versiegelt.
            (Zustand::WartetAufDenErstenRahmen, Wort::Text(_)) =>
                Self::schliessen(h, KEIN_RAHMEN),
            (Zustand::WartetAufDenErstenRahmen, Wort::Getrennt) => {
                h.zustand = Zustand::Zu;
                vec![]
            }
            (Zustand::WartetAufDenErstenRahmen, Wort::Verbunden) => vec![],

            (Zustand::Offen, Wort::Getrennt) => {
                self.horcher_austragen(h);
                vec![]
            }
            // Der Klient hat auf diesem Weg nichts zu sagen. Was er schickt,
            // wird NICHT gelesen statt gelesen und verworfen: ein Kanal, der
            // nur ansagt, laesst sich nicht zu etwas anderem ueberreden.
            (Zustand::Offen, _) => vec![],
        }
    }

    fn schliessen(h: &mut Horcher, grund: u16) -> Vec<Tat> {
        h.zustand = Zustand::Zu;
        vec![Tat::Schliessen(grund)]
    }

    /// Einen Horcher von der Liste nehmen. Auch aufzurufen, wenn die Leitung
    /// von sich aus weg ist.
    pub fn horcher_austragen(&mut self, h: &mut Horcher) {
        if let Some(besitzer) = &h.besitzer {
            self.listeners.remove(besitzer, &h.id);
        }
        self.zustellarten.remove(&h.id);
        h.zustand = Zustand::Zu;
    }

    /// Eine Ansage an alle, die fuer diesen Besitzer zuhoeren, jede in ihrer
    /// Gestalt.
    ///
    /// Ein Horcher, der weg ist, wird fallen gelassen statt wiederholt: eine
    /// Ansage ist eine BESCHLEUNIGUNG und nie die einzige Wahrheit, und ein
    /// Geraet, das eine verpasst hat, holt bei der naechsten Gelegenheit auf.
    /// Eine Schlange fuer ein abwesendes Geraet machte aus einer
    /// Annehmlichkeit eine Verbindlichkeit.
    pub fn zustellen(&mut self, ansage: &Ansage) -> Vec<(String, Tat)> {
        let mut aus = Vec::new();
        for id in self.listeners.audience(&ansage.owner) {
            match self.zustellarten.get(&id).cloned() {
                Some(Zustellart::Klar) => aus.push((id, Tat::SendeText(ansage.text.clone()))),
                Some(Zustellart::Versiegelt(name)) => {
                    match self.sessions.seal(&name, ansage.text.as_bytes(), b"") {
                        Ok(bytes) => aus.push((id, Tat::SendeBytes(bytes))),
                        // Die Sitzung ist unter dem Horcher zu Ende gegangen.
                        // Er bekommt nichts mehr, und das ist kein Fehler des
                        // Schreibers.
                        Err(_) => aus.push((id, Tat::Schliessen(SITZUNG_ZU_ENDE))),
                    }
                }
                None => {}
            }
        }
        aus
    }
}
