// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die lebenden Sitzungen, nach Namen.
//!
//! Die REGELN liegen in `sessions`: wann eine Sitzung steht, wer weicht, was
//! gefegt wird. Hier kommen die Schlüssel dazu, also das, was eine Regel
//! nicht mehr rein sein lässt. Die Trennung ist keine Ästhetik: alle Zahlen
//! von `sessions` sind an `vektoren/sitzungen.json` bezeugt, und das ginge
//! nicht mehr, wenn ein Chiffrezustand daneben läge.
//!
//! Die Uhr ist einspeisbar. Anders ist eine Stunde Stille in einer Suite,
//! die schnell bleiben muss, nicht zu bezeugen.

use std::collections::HashMap;

use crate::authorization::{hex, zufall};
use crate::handshake::{CipherState, HandshakeError, HandshakeState};
use crate::sessions::{room_for_one, standing, Limits, Sitzung, LIVE};

/// Wie lang ein Sitzungsname ist. Ein Name ist kein Geheimnis, aber ein
/// RATBARER Name ließe einen Fremden fremde Sitzungen vom Fegen abhalten oder
/// sie aus dem Laden drängen.
pub const NAME_BYTES: usize = 16;

/// Keine solche Sitzung, oder eine, die zu Ende ist. EIN Fehler für beides,
/// mit Absicht: die Antwort des Rufers ist dieselbe (ein neuer Handschlag),
/// und die zwei zu unterscheiden verriete, ob ein Name je existiert hat.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownSession(pub String);

struct Zeile {
    send: CipherState,
    receive: CipherState,
    made: f64,
    used: f64,
    proven: bool,
}

pub struct SessionStore {
    pub limits: Limits,
    zeilen: HashMap<String, Zeile>,
    uhr: Box<dyn Fn() -> f64 + Send + Sync>,
}

impl Default for SessionStore {
    fn default() -> Self { Self::new() }
}

impl SessionStore {
    pub fn new() -> Self {
        Self::mit(Limits::default(), Box::new(wanduhr))
    }

    pub fn mit(limits: Limits, uhr: Box<dyn Fn() -> f64 + Send + Sync>) -> Self {
        Self { limits, zeilen: HashMap::new(), uhr }
    }

    pub fn len(&self) -> usize { self.zeilen.len() }
    pub fn is_empty(&self) -> bool { self.zeilen.is_empty() }

    // ------------------------------------------------------ Eine anlegen

    /// Einen Handschlag beantworten und die Sitzung behalten. Gibt Namen,
    /// Antwort und die Nutzlast der ersten Nachricht zurück.
    ///
    /// Der Platz wird gemacht, BEVOR die Sitzung dazukommt, durch die eine
    /// Tür, die erst fegt und dann verdrängt: eine tote Zeile darf eine
    /// lebende nie ihren Platz kosten.
    ///
    /// Ein Handschlag, der nicht aufgeht, fliegt aus `read_message` heraus,
    /// und nichts wird gespeichert. Ein Rufer, der den Austausch nicht zu
    /// Ende bringt, hinterlässt hier keine Spur, und genau das macht eine
    /// gescheiterte Flut kostenlos.
    pub fn open(&mut self, first_message: &[u8], static_private: &[u8; 32], prologue: &[u8])
        -> Result<(String, Vec<u8>, Vec<u8>), HandshakeError>
    {
        let mut state = HandshakeState::new(false, prologue, Some(*static_private), None, None)?;
        let payload = state.read_message(first_message)?;
        let reply = state.write_message(b"")?;
        let (send, receive) = state.split();

        let now = (self.uhr)();
        for name in room_for_one(&self.als_zeilen(), now, &self.limits) {
            self.zeilen.remove(&name);
        }

        let name = hex(&zufall(NAME_BYTES).map_err(HandshakeError)?);
        self.zeilen.insert(name.clone(), Zeile {
            send, receive, made: now, used: now, proven: false,
        });
        Ok((name, reply, payload))
    }

    // ------------------------------------------------------ Eine benutzen

    fn lebende(&mut self, name: &str) -> Result<&mut Zeile, UnknownSession> {
        let now = (self.uhr)();
        let limits = self.limits;
        let tot = match self.zeilen.get(name) {
            None => return Err(UnknownSession("no such session".into())),
            Some(z) => standing(z.made, z.used, now, z.proven, &limits) != LIVE,
        };
        if tot {
            // Auf der Stelle fallen gelassen statt dem nächsten Fegen
            // überlassen: sie kann nie wieder benutzt werden, und sie zu
            // halten ließe einen Fremden einen Namen am Leben halten, indem
            // er nach ihm fragt.
            self.zeilen.remove(name);
            return Err(UnknownSession("this session has ended".into()));
        }
        Ok(self.zeilen.get_mut(name).unwrap())
    }

    /// Ob eine Sitzung dieses Namens noch Nachrichten tragen kann. Der Socket
    /// fragt, BEVOR er annimmt, damit ein Fremder mit erfundenem Namen an der
    /// Tür abgewiesen wird statt auf einer Liste zu landen.
    pub fn is_live(&mut self, name: &str) -> bool {
        self.lebende(name).is_ok()
    }

    /// Eine Nachricht öffnen.
    ///
    /// **Der Zeitpunkt der Benutzung wird nur bei ERFOLG vermerkt.** Ein
    /// Fremder, der einen Namen kennt und Rauschen schickt, darf diese Sitzung
    /// nicht am Leben halten können.
    pub fn unseal(&mut self, name: &str, ciphertext: &[u8], ad: &[u8])
        -> Result<Vec<u8>, SitzungsFehler>
    {
        let now = (self.uhr)();
        let zeile = self.lebende(name).map_err(SitzungsFehler::Unbekannt)?;
        let klar = zeile.receive.decrypt_with_ad(ad, ciphertext)
            .map_err(SitzungsFehler::Chiffre)?;
        zeile.used = now;
        Ok(klar)
    }

    pub fn seal(&mut self, name: &str, plaintext: &[u8], ad: &[u8])
        -> Result<Vec<u8>, SitzungsFehler>
    {
        let zeile = self.lebende(name).map_err(SitzungsFehler::Unbekannt)?;
        zeile.send.encrypt_with_ad(ad, plaintext).map_err(SitzungsFehler::Chiffre)
    }

    /// Sagen, dass diese Sitzung eine Anfrage getragen hat, die bewiesen hat,
    /// WER fragt. Von hier an weicht sie erst, wenn jeder Handschlag gewichen
    /// ist.
    ///
    /// Getrennt von `unseal`, weil eine Nachricht zu öffnen beweist, dass
    /// jemand die Schlüssel hält, nicht dass er ein KONTO hält. Das Zeichen
    /// darin beweist das zweite, und nur die Schicht, die es liest, weiß es.
    pub fn prove(&mut self, name: &str) -> Result<(), UnknownSession> {
        self.lebende(name)?.proven = true;
        Ok(())
    }

    // ------------------------------------------------------- Hauswirtschaft

    fn als_zeilen(&self) -> Vec<Sitzung> {
        self.zeilen.iter().map(|(name, z)| Sitzung {
            id: name.clone(), created_at: z.made, last_used_at: z.used, proven: z.proven,
        }).collect()
    }

    /// Fallen lassen, was zu Ende ist, und sagen wie viele. Nicht an einer
    /// Uhr: nichts in diesem Dienst läuft an einer, und die Richtigkeit hängt
    /// nie daran, weil jede Benutzung die Standhaftigkeit ohnehin prüft.
    pub fn sweep(&mut self) -> usize {
        let now = (self.uhr)();
        let weg: Vec<String> = self.als_zeilen().into_iter()
            .filter(|s| standing(s.created_at, s.last_used_at, now, s.proven, &self.limits) != LIVE)
            .map(|s| s.id)
            .collect();
        for name in &weg {
            self.zeilen.remove(name);
        }
        weg.len()
    }
}

/// Was beim Benutzen einer Sitzung schiefgehen kann. Die zwei sind
/// auseinandergehalten, weil der Dienst sie VERSCHIEDEN beantwortet: eine
/// unbekannte Sitzung ist 410 und ein neuer Handschlag, eine unechte
/// Nachricht ist 400 und die Sitzung überlebt sie.
#[derive(Debug)]
pub enum SitzungsFehler {
    Unbekannt(UnknownSession),
    Chiffre(HandshakeError),
}

fn wanduhr() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
