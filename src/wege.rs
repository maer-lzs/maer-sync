// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 4: die Wege, und der Tunnel durch sie hindurch.
//!
//! **Der ganze Dienst ist hier eine Funktion von einer Anfrage auf eine
//! Antwort.** Kein Socket, keine Laufzeit, keine Uhr außer der, die der
//! Sitzungsladen hält. Die Python-Fassung begründet dasselbe über ASGI: eine
//! Anwendung, die ein aufrufbares Ding mit drei Argumenten ist, lässt sich
//! ohne alles fahren. Hier ist es ein Methodenaufruf, und damit fällt auch
//! die Attrappe weg, mit der Python eine innere Anfrage als ASGI-Umgebung
//! nachbaut: eine Anfrage INNEN ist dieselbe `Anfrage` wie eine von außen,
//! nur mit `innen: true`.
//!
//! Zwei Dinge liegen mit Absicht NICHT hier, weil sie zum Transport gehören:
//!
//! - **Einen Körper von der Leitung lesen.** Ein Abbruch mitten im Körper und
//!   ein Körper, der kürzer ist als angekündigt, sind dort zu erkennen, wo die
//!   Bytes ankommen. Was hier ankommt, ist ein ganzer Körper. Die eine
//!   Ausnahme ist die ANGEKÜNDIGTE Länge: steht `content-length` in den
//!   Kopfzeilen und passt nicht, ist das auch hier noch zu sehen, und ein
//!   Schrieb wird dafür abgewiesen.
//! - **Die Ansage abschicken.** Eine angenommene Schreibung legt ihre Ansage
//!   in die Antwort, und wer die Antwort wegträgt, trägt auch sie weg. Das ist
//!   der Unterschied zur Python-Fassung, wo der Weg selbst sendet: dieselben
//!   Empfänger, dieselbe Nachricht, aber die Entscheidung bleibt prüfbar,
//!   ohne dass ein Socket existiert.

use std::collections::BTreeMap;

use crate::authorization::caller_for;
use crate::frames::{pack, request_of, unpack};
use crate::identity::public_of;
use crate::listeners::Listeners;
use crate::paths::{parse_document_path, parse_listing_path, valid_scope};
use crate::sessions::Limits;
use crate::protokoll::{sitzung_kurz, woher, Zeile};
use crate::sitzungsladen::{SessionStore, SitzungsFehler};
use crate::store::Store;

use serde_json::{json, Value};

/// Schutz des Wirts, KEINE Decke für Daten: ein Körper darüber wird mit 413
/// abgewiesen, bevor er ganz im Speicher liegt. Das Schrumpf-Tor schützt die
/// Daten; das hier schützt die Maschine vor einem kaputten Träger. Mit Absicht
/// großzügig, und je Aufstellung zu setzen.
pub const DEFAULT_MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Ob die fünf älteren Wege AUSSERHALB einer Sitzung noch antworten. An in der
/// Vorgabe, damit Geräte, die nur die einfachen Wege sprechen, weiter
/// abgleichen, bis sie erneuert sind.
pub const PLAIN_WAYS: &str = "MAER_SYNC_PLAIN_WAYS";

/// Wie der Wirt seine Koerpergrenze nennt.
pub const GRENZE: &str = "MAER_SYNC_MAX_BODY_BYTES";

/// Ob die alten Wege offen sind, aus dem Wert der Umgebungsvariablen.
///
/// Als eigene Funktion, weil die ENTSCHEIDUNG bezeugt gehoert und nicht die
/// Tatsache, dass irgendwo `std::env::var` steht. Ein unlesbarer Wert faellt
/// auf die Vorgabe zurueck, und die Vorgabe ist AN: Geraete, die nur die
/// einfachen Wege sprechen, gleichen weiter ab, bis sie erneuert sind.
pub fn alte_wege_aus(wert: Option<String>) -> bool {
    match wert {
        None => true,
        Some(v) => !matches!(v.to_ascii_lowercase().as_str(), "off" | "0" | "no" | "false"),
    }
}

/// Die Koerpergrenze aus dem Wert der Umgebungsvariablen.
///
/// Ein Wert, der keine Zahl ist, faellt auf die Vorgabe zurueck statt auf
/// null: eine Grenze von null wiese jeden Schrieb ab, und ein Tippfehler in
/// einer Umgebungsvariablen darf keinen Dienst stilllegen.
pub fn grenze_aus(wert: Option<String>) -> usize {
    wert.and_then(|v| v.trim().parse().ok()).unwrap_or(DEFAULT_MAX_BODY_BYTES)
}

/// Eine Anfrage, von außen oder von innen. `innen` ist die einzige Stelle, an
/// der die beiden sich unterscheiden, und sie wird nie von der Leitung
/// gelesen: sie wird gesetzt, wenn eine Sitzung sie entsiegelt hat.
#[derive(Debug, Clone, Default)]
pub struct Anfrage {
    pub method: String,
    pub path: String,
    pub query: Vec<u8>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub innen: bool,
    pub client: Option<String>,
    /// Der Name der Sitzung, aus der diese Anfrage entsiegelt wurde. Nur
    /// fuer die Zugriffszeile; die Entscheidung liest ihn nicht.
    pub sitzung: Option<String>,
    /// Die Adresse, fuer die die Versuchsgrenze zaehlt. Aussen leer: dann
    /// kommt sie aus der Gegenstelle und `X-Real-IP` (`versuche::adresse_aus`).
    /// Innen traegt sie die Adresse des UMSCHLAGS, und nie eine Kopfzeile aus
    /// dem versiegelten Rahmen: die schreibt der Rufer selbst.
    pub adresse: Option<String>,
}

impl Anfrage {
    pub fn neu(method: &str, path: &str) -> Self {
        Self { method: method.into(), path: path.into(), ..Default::default() }
    }
    pub fn mit_frage(mut self, query: &str) -> Self { self.query = query.as_bytes().to_vec(); self }
    pub fn mit_kopf(mut self, k: &str, v: &str) -> Self {
        self.headers.insert(k.to_ascii_lowercase(), v.into()); self
    }
    pub fn mit_koerper(mut self, body: &[u8]) -> Self { self.body = body.to_vec(); self }
    pub fn von_innen(mut self) -> Self { self.innen = true; self }
    pub fn in_sitzung(mut self, name: &str) -> Self { self.sitzung = Some(name.into()); self }
}

/// Was jemandem zu sagen ist, der zuhört. Die Antwort trägt sie; der Transport
/// schickt sie ab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ansage {
    pub owner: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Antwort {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub ansagen: Vec<Ansage>,
}

impl Antwort {
    pub fn kopf(&self, name: &str) -> Option<&str> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    /// Der Körper als JSON, oder nichts. Für Zeugen.
    pub fn als_json(&self) -> Option<Value> { serde_json::from_slice(&self.body).ok() }
    pub fn grund(&self) -> Option<String> {
        self.als_json()?.get("reason")?.as_str().map(|s| s.to_string())
    }
}

fn antwort(status: u16, art: &str, body: Vec<u8>, extra: Vec<(String, String)>) -> Antwort {
    let mut headers = vec![
        ("content-type".to_string(), art.to_string()),
        ("content-length".to_string(), body.len().to_string()),
    ];
    headers.extend(extra);
    Antwort { status, headers, body, ansagen: Vec::new() }
}

fn als_json(status: u16, wert: Value, extra: Vec<(String, String)>) -> Antwort {
    antwort(status, "application/json", wert.to_string().into_bytes(), extra)
}

fn grund(status: u16, satz: &str) -> Antwort {
    als_json(status, json!({ "reason": satz }), vec![])
}

/// Eine Antwort ohne Koerper. Fuer die Faelle, in denen die Python-Fassung
/// gar nicht antwortet und das Geraet die Antwort des SERVERS sieht.
fn leer(status: u16) -> Antwort {
    antwort(status, "application/json", Vec::new(), vec![])
}

fn falsche_methode(method: &str) -> Antwort {
    grund(405, &format!("{method} is not what this path is for"))
}

fn als_bytes(status: u16, body: Vec<u8>, extra: Vec<(String, String)>) -> Antwort {
    antwort(status, "application/octet-stream", body, extra)
}

pub struct App {
    pub store: Store,
    /// Wie jeder eingetragene Horcher seine Ansage bekommen will. Getrennt von
    /// `listeners`, weil dort die reine Buchfuehrung steht (wer hoert fuer
    /// wen) und hier die Gestalt der Zustellung, die einen Sitzungsschluessel
    /// braucht.
    pub zustellarten: BTreeMap<String, crate::horcher::Zustellart>,
    /// Der statische private Schlüssel und sein Abdruck. BEIDES freiwillig:
    /// eine Anwendung ohne Kennung ist ein echtes Ding (jeder Zeuge, der nur
    /// Dokumente will, baut eine), und ein Dienst, der einen Handschlag ohne
    /// Schlüssel beantwortete, antwortete als niemand.
    pub identity: Option<([u8; 32], String)>,
    pub sessions: SessionStore,
    pub listeners: Listeners,
    pub plain_ways: bool,
    pub max_body_bytes: usize,
    /// Was seit dem letzten Abholen zu melden war. Der Transport leert sie
    /// und schreibt sie; hier faellt nur die Entscheidung, dass es eine Zeile
    /// wert ist.
    pub protokoll: Vec<Zeile>,
    /// Die Fehlversuche an den Anmeldewegen, je Konto und Adresse.
    pub versuche: crate::versuche::Versuche,
    /// Die Uhr der Versuchsgrenze, als Sekunden. Von aussen, damit ein Zeuge
    /// ein Fenster ablaufen lassen kann, ohne zu warten.
    pub uhr: Box<dyn Fn() -> f64 + Send>,
}

/// Die Wanduhr als Sekunden seit 1970.
fn wanduhr() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

impl App {
    pub fn neu(store: Store) -> Self {
        Self {
            store,
            zustellarten: BTreeMap::new(),
            identity: None,
            sessions: SessionStore::new(),
            listeners: Listeners::new(),
            plain_ways: alte_wege_aus(std::env::var(PLAIN_WAYS).ok()),
            max_body_bytes: grenze_aus(std::env::var(GRENZE).ok()),
            protokoll: Vec::new(),
            versuche: crate::versuche::Versuche::neu(),
            uhr: Box::new(wanduhr),
        }
    }

    /// Eine andere Uhr fuer die Versuchsgrenze. Fuer Zeugen.
    pub fn mit_uhr(mut self, uhr: Box<dyn Fn() -> f64 + Send>) -> Self { self.uhr = uhr; self }

    /// Die Adresse, fuer die gezaehlt wird. Siehe `Anfrage::adresse`.
    fn adresse_von(&self, a: &Anfrage) -> String {
        a.adresse.clone().unwrap_or_else(|| crate::versuche::adresse_aus(
            a.client.as_deref(), a.headers.get("x-real-ip").map(|s| s.as_str())))
    }

    pub fn mit_kennung(mut self, private: [u8; 32], abdruck: &str) -> Self {
        self.identity = Some((private, abdruck.to_string()));
        self
    }
    pub fn ohne_alte_wege(mut self) -> Self { self.plain_ways = false; self }
    pub fn mit_grenzen(mut self, limits: Limits) -> Self {
        self.sessions = SessionStore::mit(limits, Box::new(|| 0.0));
        self
    }

    // ----------------------------------------------------------------- HTTP

    /// Eine Anfrage, eine Antwort, und eine Zugriffszeile.
    ///
    /// **Die Zeile entsteht HIER und fuer jede Anfrage**, innen wie aussen,
    /// mit dem Status, den die Entscheidung gegeben hat. Die eine Ausnahme ist
    /// der Umschlag: eine versiegelte Anfrage tritt hier ZWEIMAL ein, einmal
    /// als `POST /v1/session` von aussen und einmal entsiegelt von innen. Die
    /// innere Zeile nennt den Weg, der zaehlt; die aeussere wuerde nur
    /// `POST /v1/session 200` sagen, und genau das ist die Blindheit, an der
    /// die Python-Fassung gemessen wurde (178-mal dieselbe Zeile, kein Weg
    /// darin). Ein Umschlag, der SELBST scheitert (400, 410, 413), bekommt
    /// seine Zeile, weil dann keine innere entsteht.
    pub fn http(&mut self, a: &Anfrage) -> Antwort {
        let gab = self.entscheiden(a);
        if !(ist_getragener_umschlag(a) && gab.status == 200) {
            let bereich = frage_lesen(&a.query).get("scope").cloned()
                .filter(|b| !b.is_empty()).unwrap_or_else(|| "-".to_string());
            let grund = if gab.status >= 400 {
                gab.grund().map(|g| format!(" reason={g}")).unwrap_or_default()
            } else { String::new() };
            self.protokoll.push(Zeile::zugriff(format!(
                "access from={} session={} inner={} method={} path={} scope={} status={}{}",
                woher(a.client.as_deref()), sitzung_kurz(a.sitzung.as_deref()), a.innen,
                a.method, a.path, bereich, gab.status, grund)));
        }
        gab
    }

    /// Die REIHENFOLGE der Tore hier ist die Entscheidung, nicht die einzelne
    /// Zeile, und sie steht Fall für Fall in `vektoren/wege.json`.
    fn entscheiden(&mut self, a: &Anfrage) -> Antwort {
        // Die Gesundheit antwortet vor allem anderen, auch vor der Methode und
        // auch bei geschlossenen Wegen. Sie sagt nichts über den Laden.
        if a.path == "/v1/health" {
            return als_json(200, json!({ "ok": true }), vec![]);
        }
        // RIEGEL (c) der Fassung 6: der alte Schluesselweg antwortet 404, fuer
        // jede Methode und vor allem anderen. Eine App der Fassung 5 fragt ihn
        // vor JEDER Anmeldung und jedem Sitzungsaufbau, und nur ein 404 haelt
        // sie an, BEVOR sie ein Passwort schickt; eine 401 oder 426 an dieser
        // Stelle wuerde sie weiterlaufen lassen. Der Weg steht deshalb im Code,
        // obwohl er nicht mehr im Vertrag steht (Ausnahme mit Grund im
        // Vertragszeugen).
        if a.path == "/v1/key" {
            return grund(404, "no such path");
        }
        if a.path == "/v2/key" {
            if a.method != "GET" { return falsche_methode(&a.method); }
            let mut aus = self.schluessel();
            if aus.status == 200 {
                aus.headers.push(("x-maer-contract".into(), crate::contract::VERSION.into()));
            }
            return aus;
        }
        if a.path == "/v1/session" {
            if a.method != "POST" { return falsche_methode(&a.method); }
            return self.sitzung(a);
        }
        // Die drei Anmeldewege sprechen NUR in einer Sitzung, was auch immer
        // der Schalter der alten Wege sagt: sie tragen die einzigen Werte, die
        // fuer einen Menschen stehen.
        if matches!(a.path.as_str(), "/v1/token" | "/v1/salt" | "/v1/password") {
            if !a.innen {
                return grund(426, "the sign-in paths speak only inside a session");
            }
            return match (a.path.as_str(), a.method.as_str()) {
                ("/v1/token", "POST") => self.zeichen(a),
                ("/v1/salt", "GET") => self.salz(a),
                ("/v1/password", "POST") => self.wechsel(a),
                _ => falsche_methode(&a.method),
            };
        }
        // Das Tor steht VOR dem Zeichen: ohne die alten Wege ist auch eine
        // Anfrage MIT gültigem Zeichen 426, und ein unbekannter Weg ebenso.
        // Gesagt, nicht stumm abgewiesen: ein Gerät, das noch im Klaren
        // spricht, muss erfahren, dass dieser Dienst nur innen zuhört.
        if !self.plain_ways && !a.innen {
            return grund(426, "this service speaks only inside a session");
        }

        let caller = {
            let store = &self.store;
            caller_for(a.headers.get("authorization").map(|s| s.as_str()),
                       |d| store.owner_of_token(d).ok().flatten())
        };
        if !caller.authorized() {
            self.protokoll.push(Zeile::warnung(format!(
                "unauthorized from={} method={} path={}",
                woher(a.client.as_deref()), a.method, a.path)));
            let mut kopf = vec![("www-authenticate".to_string(), "Bearer".to_string())];
            // Ein ZURUECKGEZOGENES Zeichen sagt, warum: sonst fragt das Geraet
            // nach dem Passwort, als waere nichts geschehen.
            if let Some(g) = self.rueckzug_von(a.headers.get("authorization").map(|s| s.as_str())) {
                kopf.push(("x-maer-refusal".into(), g));
            }
            return als_json(401, json!({ "reason": caller.reason }), kopf);
        }
        let owner = caller.owner.unwrap();

        if let Some(sorte) = parse_listing_path(&a.path) {
            if a.method != "GET" { return falsche_methode(&a.method); }
            return self.liste(&owner, &sorte);
        }
        let Some(sorte) = parse_document_path(&a.path) else {
            return grund(404, "no such path");
        };
        let frage = frage_lesen(&a.query);
        let bereich = frage.get("scope").cloned().unwrap_or_default();
        if !valid_scope(Some(&bereich)) {
            return grund(400, "the scope is not a scope");
        }
        match a.method.as_str() {
            "GET" => self.holen(&owner, &sorte, &bereich),
            "PUT" => self.legen(a, &owner, &sorte, &bereich),
            _ => falsche_methode(&a.method),
        }
    }

    /// Der ÖFFENTLICHE statische Schlüssel, damit ein Gerät ihn festhalten
    /// kann. Kein Zeichen, und er könnte sinnvoll keines verlangen: das ist
    /// das Erste, was ein Gerät holt.
    ///
    /// **Das ist der Kanal des ersten Vertrauens, und er sagt das.** Über eine
    /// offene Leitung kann hier jeder in der Mitte seinen eigenen Schlüssel
    /// ausgeben. Das Festhalten ist die Hälfte des Geräts, und es macht einen
    /// späteren Tausch zu einem Ereignis.
    fn schluessel(&self) -> Antwort {
        let Some((private, abdruck)) = &self.identity else {
            return grund(503, "this service has no static key");
        };
        als_json(200, json!({
            "key": base64(&public_of(private)),
            "fingerprint": abdruck,
        }), vec![])
    }

    // --------------------------------------------------------- Der Tunnel

    /// Ohne Sitzungskopfzeile: einen Handschlag beantworten und den Namen
    /// zurückgeben. Mit einer: eine versiegelte Anfrage in dieser Sitzung
    /// tragen.
    ///
    /// Nichts hier ist JSON: das sind zwei Nachrichten fester Gestalt eines
    /// Protokolls, und sie in ein Dokument zu wickeln lüde nur jemanden ein,
    /// ein Feld hinzuzufügen.
    fn sitzung(&mut self, a: &Anfrage) -> Antwort {
        let Some((private, _)) = self.identity else {
            return grund(503, "this service has no static key");
        };
        if a.body.len() > self.max_body_bytes {
            return grund(413, "a handshake is not this large");
        }
        if let Some(name) = a.headers.get("x-maer-session").cloned() {
            return self.versiegelt(a, &name);
        }
        match self.sessions.open(&a.body, &private, b"") {
            // Jede Art, kein Handschlag zu sein, ist EINE Antwort. Zu sagen,
            // welche, verriete jemandem beim Abtasten, wie weit er kam.
            Err(_) => {
                self.protokoll.push(Zeile::warnung(format!(
                    "handshake refused from={}", woher(a.client.as_deref()))));
                grund(400, "this is not a handshake")
            }
            Ok((name, reply, _)) =>
                als_bytes(200, reply, vec![("x-maer-session".into(), name)]),
        }
    }

    /// Eine Anfrage innerhalb einer Sitzung: öffnen, durch dieselben Türen
    /// schicken wie jede Anfrage, die Antwort versiegeln.
    ///
    /// **Innen ist die gewöhnliche Leitung.** Der Rahmen trägt Methode, Ziel,
    /// Kopfzeilen und Körper, also reitet das Zeichen in `Authorization`
    /// genau wie der Vertrag es sagt, nur kann niemand dazwischen es lesen.
    ///
    /// Eine Sitzung, die zu Ende ist, ist 410 und nichts sonst. Eine
    /// Nachricht, die nicht aufgeht, ist 400, und die Sitzung überlebt es:
    /// der Zähler der Chiffre rückt nur bei Erfolg vor, also kostet Rauschen
    /// unter einem gestohlenen Namen seinen Besitzer nichts.
    fn versiegelt(&mut self, a: &Anfrage, name: &str) -> Antwort {
        let klartext = match self.sessions.unseal(name, &a.body, b"") {
            Ok(k) => k,
            Err(SitzungsFehler::Unbekannt(_)) => return grund(410, "this session has ended"),
            Err(SitzungsFehler::Chiffre(_)) => {
                self.protokoll.push(Zeile::warnung(format!(
                    "sealed message refused from={}", woher(a.client.as_deref()))));
                return grund(400, "this is not a sealed message");
            }
        };
        let Ok((kopf, koerper)) = unpack(&klartext) else {
            return grund(400, "this is not a request");
        };
        let Ok((method, pfad, frage, kopfzeilen)) = request_of(&kopf) else {
            return grund(400, "this is not a request");
        };
        // Eine Sitzung schachtelt nicht, und der Schlüssel ist das, woraus sie
        // gebaut wurde; nach beiden von innen zu fragen sagt nichts. Die
        // Weigerung geht UNVERSIEGELT zurück, wie jede Weigerung der äußeren
        // Schicht: sie ist keine Antwort auf die innere Anfrage.
        if pfad == "/v1/session" || pfad == "/v2/key" {
            return grund(400, "this path is not answered inside a session");
        }

        // Die WAHRE Länge gewinnt gegen die angekündigte. Der Körper ist
        // genau das, was aus dem Rahmen kam; eine Zahl im Rahmenkopf, die
        // etwas anderes behauptet, ist veraltet oder gelogen, und in beiden
        // Fällen ist sie nichts, woran eine Entscheidung hängen darf. Die
        // angekündigte Länge hat ihren Sinn auf der LEITUNG, wo der Körper
        // erst noch kommt; hier ist er schon da.
        let mut innere_kopfzeilen = kopfzeilen.clone();
        innere_kopfzeilen.insert("content-length".into(), koerper.len().to_string());
        let innen = Anfrage {
            method, path: pfad.clone(), query: frage,
            headers: innere_kopfzeilen, body: koerper,
            innen: true, client: a.client.clone(), sitzung: Some(name.to_string()),
            adresse: Some(self.adresse_von(a)),
        };
        let gab = self.http(&innen);

        // Bewiesen heißt: diese Sitzung hat eine Anfrage getragen, die
        // bewiesen hat, WER fragt. Ein angenommenes Zeichen beweist es, eine
        // Anmeldung, zu der der Laden ja gesagt hat, auch, und ein gelungener
        // Passwortwechsel ebenso. Eine 401 beweist nichts, und die zwei
        // Antworten der Fassung 6 auch nicht: eine 429 hat gar nicht erst
        // geprueft, und eine 428 gibt kein Zeichen.
        if !matches!(gab.status, 401 | 428 | 429)
            && (kopfzeilen.contains_key("authorization") || pfad == "/v1/token"
                || pfad == "/v1/password")
            && self.sessions.prove(name).is_err()
        {
            return grund(410, "this session has ended");
        }

        let kopf_zurueck: BTreeMap<String, String> = gab.headers.iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("content-length"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let Ok(roh) = pack(&json!({ "status": gab.status, "headers": kopf_zurueck }), &gab.body)
        else {
            return grund(400, "this is not a request");
        };
        match self.sessions.seal(name, &roh, b"") {
            Err(SitzungsFehler::Unbekannt(_)) => grund(410, "this session has ended"),
            Err(SitzungsFehler::Chiffre(_)) => grund(400, "this is not a sealed message"),
            Ok(versiegelt) => {
                let mut aus = als_bytes(200, versiegelt, vec![]);
                // Die Ansage der inneren Schreibung reist mit nach draußen.
                aus.ansagen = gab.ansagen;
                aus
            }
        }
    }

    // ------------------------------------------------------- Die fünf Wege

    /// `/v1/token`: ein Beweis (Fassung 6), oder ein Passwort, das der Dienst
    /// kennt (Fassung 5, nur fuer ein noch nicht umgezogenes Konto).
    fn zeichen(&mut self, a: &Anfrage) -> Antwort {
        if a.body.len() > self.max_body_bytes {
            return grund(413, "the request is larger than this host takes");
        }
        let satz = "this path wants an owner and a proof";
        let Ok(gefragt) = serde_json::from_slice::<Value>(&a.body) else {
            return grund(400, satz);
        };
        let Some(owner) = gefragt.get("owner").and_then(|v| v.as_str()) else {
            return grund(400, satz);
        };
        let device = gefragt.get("device").and_then(|v| v.as_str()).unwrap_or("");
        let adresse = self.adresse_von(a);
        // Die Grenze VOR jeder Ableitung: eine Sperre nach dem teuren
        // Vergleich spart dem Wirt nichts.
        if let Some(antwort) = self.gesperrt(owner, &adresse) {
            return antwort;
        }

        let ergebnis = if let Some(b) = gefragt.get("proof").and_then(|v| v.as_str()) {
            let Some(beweis) = base64_lesen(b)
                .filter(|r| r.len() == crate::authorization::PROOF_BYTES)
            else {
                return grund(400, satz);
            };
            self.store.anmelden_mit_beweis(owner, &beweis, device)
        } else if let Some(s) = gefragt.get("secret").and_then(|v| v.as_str()) {
            self.store.anmelden_mit_geheimnis(owner, s, device)
        } else {
            return grund(400, satz);
        };

        match ergebnis {
            Ok(crate::store::Anmeldung::Zeichen(zeichen)) => {
                self.versuche.erfolg(owner, &adresse);
                self.protokoll.push(Zeile::hinweis(format!(
                    "sign-in from={} owner={owner} device={device}",
                    woher(a.client.as_deref()))));
                als_json(200, json!({ "token": zeichen }), vec![])
            }
            // Das Passwort stimmt, aber der Betreiber hat es gesetzt. Kein
            // Fehlversuch, und kein Zeichen.
            Ok(crate::store::Anmeldung::NurWechsel) => {
                self.versuche.erfolg(owner, &adresse);
                self.protokoll.push(Zeile::hinweis(format!(
                    "sign-in with the password the operator set from={} owner={owner}",
                    woher(a.client.as_deref()))));
                grund(428, "the operator set this password; change it on /v1/password")
            }
            // Dieselbe Antwort für ein falsches Geheimnis und für ein Konto,
            // das es nicht gibt. Alles andere macht daraus eine Liste, wer
            // ein Konto hat.
            Ok(crate::store::Anmeldung::Abgewiesen) | Err(_) => {
                self.versuche.fehlschlag(owner, &adresse, (self.uhr)());
                self.protokoll.push(Zeile::warnung(format!(
                    "sign-in refused from={} owner={owner}", woher(a.client.as_deref()))));
                grund(401, crate::authorization::DENIED)
            }
        }
    }

    /// `/v1/salt?owner=`: fuer jeden Namen dieselbe Gestalt.
    fn salz(&mut self, a: &Anfrage) -> Antwort {
        let owner = frage_lesen(&a.query).get("owner").cloned().unwrap_or_default();
        if owner.is_empty() {
            return grund(400, "this path wants an owner");
        }
        match self.store.salz_fuer(&owner) {
            Ok(s) => als_json(200, json!({
                "salt": base64(&s.salt),
                "kdf": crate::authorization::KDF,
                "rounds": s.rounds,
                "form": s.form,
            }), vec![]),
            Err(_) => leer(500),
        }
    }

    /// `/v1/password`: das Passwort wechseln, und damit auch der Umzug.
    fn wechsel(&mut self, a: &Anfrage) -> Antwort {
        if a.body.len() > self.max_body_bytes {
            return grund(413, "the request is larger than this host takes");
        }
        let satz = "this path wants an owner, the current credential and the new one";
        let Ok(gefragt) = serde_json::from_slice::<Value>(&a.body) else {
            return grund(400, satz);
        };
        let Some(owner) = gefragt.get("owner").and_then(|v| v.as_str()) else {
            return grund(400, satz);
        };
        let device = gefragt.get("device").and_then(|v| v.as_str()).unwrap_or("");
        let adresse = self.adresse_von(a);
        if let Some(antwort) = self.gesperrt(owner, &adresse) {
            return antwort;
        }

        // Das Neue zuerst lesen: ein Wechsel, dessen Ziel unlesbar ist, soll
        // gar nicht erst pruefen, ob der Rufer der Richtige ist.
        let neu = gefragt.get("new");
        let feld = |n: &str| neu.and_then(|v| v.get(n));
        let neu_salz = feld("salt").and_then(|v| v.as_str()).and_then(base64_lesen);
        let neu_beweis = feld("proof").and_then(|v| v.as_str()).and_then(base64_lesen);
        let neu_kdf = feld("kdf").and_then(|v| v.as_str());
        let neu_runden = feld("rounds").and_then(|v| v.as_u64());
        let (Some(neu_salz), Some(neu_beweis), Some(crate::authorization::KDF), Some(neu_runden)) =
            (neu_salz, neu_beweis, neu_kdf, neu_runden)
        else {
            return grund(400, "the new password is not readable");
        };
        if neu_salz.len() != crate::authorization::KONTO_SALZ_BYTES
            || neu_beweis.len() != crate::authorization::PROOF_BYTES
            || neu_runden < crate::authorization::MIN_ROUNDS
        {
            return grund(400, "the new password is not readable");
        }

        let jetzt_gilt = gefragt.get("current");
        let beweis_roh = jetzt_gilt.and_then(|v| v.get("proof")).and_then(|v| v.as_str());
        let geheimnis = jetzt_gilt.and_then(|v| v.get("secret")).and_then(|v| v.as_str());
        let beweis;
        let aktuell = match (beweis_roh, geheimnis) {
            (Some(b), None) => {
                match base64_lesen(b).filter(|r| r.len() == crate::authorization::PROOF_BYTES) {
                    Some(r) => { beweis = r; crate::store::Aktuell::Beweis(&beweis) }
                    None => return grund(400, satz),
                }
            }
            (None, Some(s)) => crate::store::Aktuell::Geheimnis(s),
            _ => return grund(400, satz),
        };

        match self.store.passwort_wechseln(owner, device, aktuell, &neu_salz, neu_runden, &neu_beweis) {
            Ok(crate::store::Anmeldung::Zeichen(zeichen)) => {
                self.versuche.erfolg(owner, &adresse);
                self.protokoll.push(Zeile::hinweis(format!(
                    "password changed from={} owner={owner} device={device}; \
every other sign-in of this owner is withdrawn", woher(a.client.as_deref()))));
                als_json(200, json!({ "token": zeichen }), vec![])
            }
            Ok(_) => {
                self.versuche.fehlschlag(owner, &adresse, (self.uhr)());
                self.protokoll.push(Zeile::warnung(format!(
                    "password change refused from={} owner={owner}", woher(a.client.as_deref()))));
                grund(401, crate::authorization::DENIED)
            }
            Err(crate::store::StoreError::Refused(_)) => grund(400, "the new password is not readable"),
            Err(_) => leer(500),
        }
    }

    /// Die 429, wenn das Paar gesperrt ist, sonst nichts.
    fn gesperrt(&mut self, owner: &str, adresse: &str) -> Option<Antwort> {
        let sekunden = self.versuche.gesperrt(owner, adresse, (self.uhr)())?;
        self.protokoll.push(Zeile::warnung(format!(
            "sign-in held back from={adresse} owner={owner} for={sekunden}s")));
        Some(als_json(429, json!({ "reason": "too many failed attempts; try again later" }),
                      vec![("retry-after".into(), sekunden.to_string())]))
    }

    /// Warum das vorgelegte Zeichen zurueckgezogen ist, oder nichts.
    fn rueckzug_von(&self, kopf: Option<&str>) -> Option<String> {
        let rest = kopf?.trim();
        let (art, zeichen) = rest.split_once(' ')?;
        if !art.eq_ignore_ascii_case("bearer") || zeichen.trim().is_empty() { return None; }
        let digest = crate::authorization::token_digest(zeichen.trim());
        self.store.grund_des_rueckzugs(&digest).ok().flatten()
    }

    fn liste(&self, owner: &str, sorte: &str) -> Antwort {
        let Ok(gefunden) = self.store.scopes(owner, sorte) else {
            return grund(404, "no such document");
        };
        let dokumente: Vec<Value> = gefunden.into_iter()
            .map(|(bereich, etikett, format)| json!({
                "scope": bereich, "version": etikett, "format": format }))
            .collect();
        als_json(200, json!({ "kind": sorte, "documents": dokumente }), vec![])
    }

    fn holen(&self, owner: &str, sorte: &str, bereich: &str) -> Antwort {
        match self.store.current(owner, sorte, bereich) {
            Ok(Some(held)) => als_bytes(200, held.body, vec![
                ("etag".into(), held.etag),
                ("x-maer-format".into(), held.format.to_string()),
            ]),
            _ => grund(404, "no such document"),
        }
    }

    fn legen(&mut self, a: &Anfrage, owner: &str, sorte: &str, bereich: &str) -> Antwort {
        // Ein Format zu RATEN ließe jeden Klienten, der die Kopfzeile vergisst,
        // Dokumente überschreiben, die eine neuere Fassung geschrieben hat.
        let Some(format) = a.headers.get("x-maer-format").and_then(|v| v.parse::<i64>().ok())
        else {
            return grund(400, "the write named no format");
        };
        if a.body.len() > self.max_body_bytes {
            self.protokoll.push(Zeile::warnung(format!(
                "write too large from={} owner={owner} kind={sorte} scope={bereich} limit={}",
                woher(a.client.as_deref()), self.max_body_bytes)));
            return grund(413, "the document is larger than this host takes");
        }
        // Die einzige Unvollständigkeit, die auch hier noch zu sehen ist: eine
        // angekündigte Länge, die nicht stimmt. Nichts wird gespeichert, der
        // gehaltene Stand bleibt ganz. Ein Schrieb, der abbricht, ist kein
        // kleineres Dokument, er ist ein halbes, und ein halbes nimmt das
        // Ganze jedem Leser weg.
        if let Some(angekuendigt) = a.headers.get("content-length").and_then(|v| v.parse::<usize>().ok()) {
            if angekuendigt != a.body.len() {
                self.protokoll.push(Zeile::warnung(format!(
                    "incomplete write from={} owner={owner} kind={sorte} scope={bereich} \
announced={angekuendigt}", woher(a.client.as_deref()))));
                return grund(400, "the write ended before the body did");
            }
        }
        // Was der Dienst nicht lesen kann, ersetzt nie, was er hält. Ein
        // LEERER Körper ist kein kaputtes Dokument: das ist der leere Stand,
        // den der Laden mit seinem eigenen Satz beantwortet.
        if !a.body.is_empty() && serde_json::from_slice::<Value>(&a.body).is_err() {
            self.protokoll.push(Zeile::warnung(format!(
                "unreadable write from={} owner={owner} kind={sorte} scope={bereich} bytes={}",
                woher(a.client.as_deref()), a.body.len())));
            return grund(400, "the write is not readable as a document");
        }

        let geschrieben = self.store.write(
            owner, sorte, &a.body, format, bereich,
            a.headers.get("if-match").map(|s| s.as_str()),
            a.headers.get("x-maer-declared-shrink").map(|s| s == "1").unwrap_or(false));
        let Ok((urteil, etikett)) = geschrieben else {
            // **Leer, und das ist die Gestalt, nicht die Bequemlichkeit.** Die
            // Python-Fassung kann hier gar nicht antworten: der Fehler fliegt
            // aus dem Laden heraus, und was das Geraet sieht, ist die 500 des
            // SERVERS, ohne Koerper. Ein Satz an dieser Stelle waere
            // nuetzlicher und machte die zwei Fassungen unterscheidbar; die
            // 500 selbst nennt der Vertrag ohnehin nicht, auf keiner Seite.
            return leer(500);
        };

        if !urteil.accepted() {
            self.protokoll.push(Zeile::warnung(format!(
                "refused from={} owner={owner} kind={sorte} scope={bereich} status={} reason={}",
                woher(a.client.as_deref()), urteil.status, urteil.reason)));
            let gehalten = self.store.current(owner, sorte, bereich).ok().flatten();
            let extra = gehalten.map(|h| vec![("etag".to_string(), h.etag)]).unwrap_or_default();
            return als_json(urteil.status, json!({ "reason": urteil.reason }), extra);
        }
        let etikett = etikett.unwrap_or_default();
        let mut aus = als_json(200,
            json!({ "reason": urteil.reason, "version": etikett }),
            vec![("etag".into(), etikett.clone())]);
        // GENAU EINE Ansage je angenommenem Schrieb, unabhängig davon, wie
        // viele zuhören. Das Auffächern auf die Zuhörer ist Sache dessen, der
        // sie wegträgt; hier steht die Entscheidung, und die ist eine. Sie
        // steht auch dann da, wenn niemand zuhört: dann trägt der Transport
        // sie an niemanden, und das ist kein anderer Fall.
        aus.ansagen = vec![Ansage {
            owner: owner.to_string(),
            text: json!({ "kind": sorte, "scope": bereich, "version": etikett }).to_string(),
        }];
        aus
    }
}

/// Ein `POST /v1/session` MIT Sitzungskopf ist ein Umschlag: er traegt eine
/// innere Anfrage, und die schreibt ihre eigene Zeile. Ohne den Kopf ist
/// es der Handschlag selbst, und der ist ein Vorgang mit eigener Zeile.
fn ist_getragener_umschlag(a: &Anfrage) -> bool {
    !a.innen && a.path == "/v1/session" && a.headers.contains_key("x-maer-session")
}

// ------------------------------------------- Das bisschen HTTP, das ein
// Byte-Laden braucht

/// Die Frage als Abbildung, Werte entschlüsselt: ein Klient, der den
/// Doppelpunkt in einer Gerätekennung kodiert, muss sein eigenes Dokument
/// wiederfinden. Ein Paar ohne `=` fällt weg, wie in der Python-Fassung.
pub fn frage_lesen(roh: &[u8]) -> BTreeMap<String, String> {
    let text = String::from_utf8_lossy(roh);
    let mut aus = BTreeMap::new();
    for paar in text.split('&') {
        if let Some((k, v)) = paar.split_once('=') {
            aus.insert(k.to_string(), prozent_lesen(v));
        }
    }
    aus
}

/// `%XX` zurück in ein Byte. `+` bleibt `+`, wie bei Pythons `unquote`.
fn prozent_lesen(s: &str) -> String {
    let b = s.as_bytes();
    let mut aus: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hoch = (b[i + 1] as char).to_digit(16);
            let tief = (b[i + 2] as char).to_digit(16);
            if let (Some(h), Some(t)) = (hoch, tief) {
                aus.push((h * 16 + t) as u8);
                i += 3;
                continue;
            }
        }
        aus.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&aus).into_owned()
}

/// base64 nach RFC 4648, mit Füllung. Zwanzig Zeilen statt einer Crate: das
/// hier ist eine Tabelle und drei Verschiebungen, und die Abhängigkeit wäre
/// mehr, als die Frage wert ist.
pub fn base64_lesen(text: &str) -> Option<Vec<u8>> {
    const TAFEL: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut aus = Vec::new();
    let (mut puffer, mut wie_viel) = (0u32, 0u32);
    for c in text.trim().bytes() {
        if c == b'=' { break; }
        if c.is_ascii_whitespace() { continue; }
        // Streng gelesen: ein Zeichen, das nicht in die Tafel gehoert, macht
        // die ganze Eingabe ungueltig. Es zu ueberspringen hiesse, eine
        // beschaedigte Datei als kuerzeren Schluessel zu lesen.
        let i = TAFEL.iter().position(|&t| t == c)?;
        puffer = (puffer << 6) | i as u32;
        wie_viel += 6;
        if wie_viel >= 8 {
            wie_viel -= 8;
            aus.push((puffer >> wie_viel) as u8);
        }
    }
    Some(aus)
}

pub fn base64(roh: &[u8]) -> String {
    const TAFEL: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut aus = String::with_capacity(roh.len().div_ceil(3) * 4);
    for stueck in roh.chunks(3) {
        let b0 = stueck[0] as u32;
        let b1 = *stueck.get(1).unwrap_or(&0) as u32;
        let b2 = *stueck.get(2).unwrap_or(&0) as u32;
        let drei = (b0 << 16) | (b1 << 8) | b2;
        aus.push(TAFEL[(drei >> 18 & 63) as usize] as char);
        aus.push(TAFEL[(drei >> 12 & 63) as usize] as char);
        aus.push(if stueck.len() > 1 { TAFEL[(drei >> 6 & 63) as usize] as char } else { '=' });
        aus.push(if stueck.len() > 2 { TAFEL[(drei & 63) as usize] as char } else { '=' });
    }
    aus
}
