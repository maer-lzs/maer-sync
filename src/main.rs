// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Draht: hier trifft die Entscheidung auf die Leitung.
//!
//! Alles, was entschieden wird, steht in der Bibliothek und ist ohne Netz
//! bezeugt. Hier steht nur, was ohne Netz nicht zu haben ist, und das ist
//! genau die Stelle, an der Integrationsfehler leben:
//!
//! - **Einen Körper von der Leitung lesen.** Ein Abbruch mitten im Körper und
//!   ein Körper, der kürzer ist als angekündigt, sind nur hier zu erkennen.
//!   `wege.rs` bekommt einen GANZEN Körper oder gar keinen.
//! - **Den Aufstieg zur Leitung.** Die Kopfzeilen, der Schlüssel, das 101.
//! - **Die Ansage zustellen.** `wege.rs` entscheidet, dass angesagt wird, und
//!   legt die Ansage in die Antwort; wer sie wegträgt, ist hier.
//!
//! **Kein `#[tokio::main]`:** das Merkmal `macros` ist nicht im Baum, weil es
//! eine Crate kostet und genau eine Zeile spart. Die Laufzeit entsteht von
//! Hand, und das ist ohnehin die ehrlichere Form, weil man sieht, was sie ist.

use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;

use maer_sync::horcher::Tat;
use maer_sync::kennung::{key_path, open_identity};
use maer_sync::store::{Store, DEFAULT_KEEP};
use maer_sync::wege::{Anfrage, Ansage, App};

/// Woran ein Körper scheitern kann, bevor er je eine Entscheidung erreicht.
enum KoerperFehler {
    /// Der Rufer ist mitten im Körper gegangen, oder hat weniger geschickt,
    /// als er angekündigt hat. **Nichts wird gespeichert:** ein Schrieb, der
    /// abbricht, ist kein kleineres Dokument, sondern ein halbes, und ein
    /// halbes nimmt das Ganze jedem Leser weg.
    Unvollstaendig,
    /// Grösser als dieser Wirt trägt. Wird abgewiesen, BEVOR alles im
    /// Speicher liegt.
    ZuGross,
}

/// Ein Körper, der in einem Stück dasteht. Von Hand, weil `http-body-util`
/// eine Aufnahme wäre (Regel 4) und die zwei Fälle hier zwanzig Zeilen sind.
struct Voll(Option<Bytes>);

impl Body for Voll {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(mut self: Pin<&mut Self>, _: &mut Context<'_>)
        -> Poll<Option<Result<Frame<Bytes>, Self::Error>>>
    {
        Poll::Ready(self.0.take().map(|b| Ok(Frame::data(b))))
    }
}

/// Der laufende Dienst: die Entscheidungen, und die Drähte, an denen jemand
/// zuhört.
struct Dienst {
    app: Mutex<App>,
    /// Je offenem Horcher ein Weg hinaus, unter der Kennung, die der Horcher
    /// selbst trägt. **Getrennt von der Ansageliste in `App`**, weil die dort
    /// eine Entscheidung ist und das hier eine Leitung: wer beides in einen
    /// Topf wirft, kann die Entscheidung nicht mehr ohne Netz prüfen.
    draehte: Mutex<HashMap<String, UnboundedSender<Tat>>>,
}

impl Dienst {
    /// Eine Ansage an alle austeilen, die für ihren Besitzer zuhören.
    ///
    /// Ein Draht, der weg ist, wird fallen gelassen statt wiederholt: eine
    /// Ansage ist eine Beschleunigung und nie die einzige Wahrheit.
    fn ansagen_austeilen(&self, ansagen: &[Ansage]) {
        for ansage in ansagen {
            let zustellungen = {
                let mut app = self.app.lock().unwrap();
                app.zustellen(ansage)
            };
            let mut draehte = self.draehte.lock().unwrap();
            for (id, tat) in zustellungen {
                let weg = match draehte.get(&id) {
                    Some(w) => w,
                    None => continue,
                };
                if weg.send(tat).is_err() {
                    draehte.remove(&id);
                }
            }
        }
    }
}

fn main() -> std::process::ExitCode {
    let wo = std::env::var("MAER_SYNC_DB").unwrap_or_else(|_| "maer-sync.sqlite".into());
    let adresse = std::env::var("MAER_SYNC_BIND").unwrap_or_else(|_| "127.0.0.1:8081".into());

    let mut laden = match Store::open(&wo, DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("the store {wo} cannot be opened: {e:?}");
            return std::process::ExitCode::from(2);
        }
    };
    // Die Kennung wird HIER geöffnet und nicht in `App`: eine zu machen ist
    // eine Entscheidung mit Folgen für jedes Gerät, also gehört sie dorthin,
    // wo ein Dienst absichtlich zusammengesetzt wird. Eine Weigerung wird
    // nicht verschluckt: ein Dienst, der nicht sagen kann, wer er ist, darf
    // nicht hochkommen.
    let pfad = key_path(Some(&wo));
    let (privat, abdruck) = match open_identity(&mut laden, &pfad) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("{}", e.0);
            return std::process::ExitCode::from(3);
        }
    };

    let app = App::neu(laden).mit_kennung(privat, &abdruck);
    let dienst = Arc::new(Dienst {
        app: Mutex::new(app),
        draehte: Mutex::new(HashMap::new()),
    });

    let laufzeit = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("the runtime cannot be built: {e}");
            return std::process::ExitCode::from(4);
        }
    };

    laufzeit.block_on(async move {
        let horcht = match TcpListener::bind(&adresse).await {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{adresse} cannot be listened on: {e}");
                return std::process::ExitCode::from(5);
            }
        };
        eprintln!("serving on {adresse}");
        loop {
            let (strom, wer) = match horcht.accept().await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("a connection could not be accepted: {e}");
                    continue;
                }
            };
            let dienst = Arc::clone(&dienst);
            tokio::spawn(async move {
                let von = format!("{}:{}", wer.ip(), wer.port());
                let dienen = service_fn(move |anfrage| {
                    bedienen(Arc::clone(&dienst), anfrage, von.clone())
                });
                // `with_upgrades`, sonst endet die Verbindung mit der
                // 101-Antwort und der Aufstieg kommt nie an.
                if let Err(e) = http1::Builder::new()
                    .serve_connection(TokioIo::new(strom), dienen)
                    .with_upgrades()
                    .await
                {
                    // Ein Rufer, der einfach geht, ist kein Fehler des Wirts.
                    let text = e.to_string();
                    if !text.contains("closed") && !text.contains("reset") {
                        eprintln!("connection from {}: {text}", wer.ip());
                    }
                }
            });
        }
    })
}

/// Eine Anfrage von der Leitung. Erst der Aufstieg, dann die gewöhnlichen
/// Wege.
async fn bedienen(dienst: Arc<Dienst>, anfrage: Request<Incoming>, von: String)
    -> Result<Response<Voll>, Infallible>
{
    let kopfzeilen: std::collections::BTreeMap<String, String> = anfrage.headers().iter()
        .map(|(k, v)| (k.as_str().to_ascii_lowercase(),
                       v.to_str().unwrap_or("").to_string()))
        .collect();

    if ist_aufstieg(&kopfzeilen) {
        return Ok(aufsteigen(dienst, anfrage, kopfzeilen, von));
    }

    let method = anfrage.method().as_str().to_string();
    let pfad = anfrage.uri().path().to_string();
    let frage = anfrage.uri().query().unwrap_or("").as_bytes().to_vec();
    let grenze = { dienst.app.lock().unwrap().max_body_bytes };
    let angekuendigt = kopfzeilen.get("content-length").and_then(|v| v.parse::<usize>().ok());

    let koerper = match koerper_lesen(anfrage.into_body(), grenze, angekuendigt).await {
        Ok(k) => k,
        Err(KoerperFehler::Unvollstaendig) =>
            return Ok(einfach(400, satz_unvollstaendig(&pfad))),
        Err(KoerperFehler::ZuGross) =>
            return Ok(einfach(413, satz_zu_gross(&pfad))),
    };

    let gab = {
        let mut app = dienst.app.lock().unwrap();
        app.http(&Anfrage {
            method, path: pfad, query: frage, headers: kopfzeilen,
            body: koerper, innen: false, client: Some(von),
        })
    };
    dienst.ansagen_austeilen(&gab.ansagen);

    let mut bau = Response::builder().status(gab.status);
    for (k, v) in &gab.headers {
        bau = bau.header(k.as_str(), v.as_str());
    }
    Ok(bau.body(Voll(Some(Bytes::from(gab.body)))).expect("die Antwort ist wohlgeformt"))
}

/// Die Sätze der abgebrochenen und der zu grossen Körper, je Weg.
///
/// Sie stehen HIER und nicht in `wege.rs`, weil nur hier auffallen kann, dass
/// ein Körper unvollständig ist. Sie sind wörtlich die der Python-Fassung, und
/// der Weg entscheidet, welcher es ist.
fn satz_unvollstaendig(pfad: &str) -> (&'static str, &'static str) {
    match pfad {
        "/v1/session" => ("reason", "the handshake broke off mid request"),
        "/v1/token" => ("reason", "the request ended before it was whole"),
        _ => ("reason", "the write ended before the body did"),
    }
}

fn satz_zu_gross(pfad: &str) -> (&'static str, &'static str) {
    match pfad {
        "/v1/session" => ("reason", "a handshake is not this large"),
        "/v1/token" => ("reason", "the request is larger than this host takes"),
        _ => ("reason", "the document is larger than this host takes"),
    }
}

fn einfach(status: u16, (feld, satz): (&str, &str)) -> Response<Voll> {
    let koerper = serde_json::json!({ feld: satz }).to_string();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("content-length", koerper.len().to_string())
        .body(Voll(Some(Bytes::from(koerper))))
        .expect("die Antwort ist wohlgeformt")
}

/// Den ganzen Körper lesen, oder sagen warum nicht.
///
/// **Die Grenze greift, WÄHREND gelesen wird**, nicht danach: ein Körper über
/// der Grenze darf nie ganz im Speicher liegen. Und eine angekündigte Länge,
/// die nicht eintrifft, ist ein Abbruch, kein kürzerer Körper.
async fn koerper_lesen(mut herein: Incoming, grenze: usize, angekuendigt: Option<usize>)
    -> Result<Vec<u8>, KoerperFehler>
{
    let mut aus: Vec<u8> = Vec::new();
    loop {
        let stueck = std::future::poll_fn(|cx| Pin::new(&mut herein).poll_frame(cx)).await;
        match stueck {
            None => break,
            // Ein Fehler auf der Leitung mitten im Körper ist ein Abbruch.
            Some(Err(_)) => return Err(KoerperFehler::Unvollstaendig),
            Some(Ok(rahmen)) => {
                if let Ok(daten) = rahmen.into_data() {
                    if aus.len() + daten.len() > grenze {
                        return Err(KoerperFehler::ZuGross);
                    }
                    aus.extend_from_slice(&daten);
                }
            }
        }
    }
    if let Some(n) = angekuendigt {
        if aus.len() != n {
            return Err(KoerperFehler::Unvollstaendig);
        }
    }
    Ok(aus)
}

// ------------------------------------------------------------ Der Aufstieg

fn ist_aufstieg(kopfzeilen: &std::collections::BTreeMap<String, String>) -> bool {
    let sagt = |name: &str, wort: &str| kopfzeilen.get(name)
        .map(|v| v.to_ascii_lowercase().split(',').any(|t| t.trim() == wort))
        .unwrap_or(false);
    sagt("upgrade", "websocket") && sagt("connection", "upgrade")
}

/// Die 101 bauen und den Horcher hinter der Verbindung starten.
///
/// **Der Horcher wird eröffnet, BEVOR die 101 hinausgeht**, und das ist keine
/// Feinheit: die Entscheidung, ob dieser Rufer überhaupt zuhören darf, fällt
/// im Klaren vor dem Annehmen. Fällt sie gegen ihn, steigt hier gar nichts
/// auf, und er bekommt seine Zahl statt einer offenen Leitung.
fn aufsteigen(dienst: Arc<Dienst>, anfrage: Request<Incoming>,
              kopfzeilen: std::collections::BTreeMap<String, String>, _von: String)
    -> Response<Voll>
{
    let Some(_schluessel) = kopfzeilen.get("sec-websocket-key").cloned() else {
        return einfach(400, ("reason", "this upgrade names no key"));
    };
    let pfad = anfrage.uri().path().to_string();

    let (mut horcher, taten) = {
        let mut app = dienst.app.lock().unwrap();
        app.horcher_oeffnen(&pfad, &kopfzeilen)
    };
    // Eine Abweisung reist als Schliesscode, und ohne Aufstieg gibt es keine
    // Leitung, auf der er reisen könnte. Also sagt es der Status: dieselbe
    // Entscheidung, nur in der Sprache, die vor dem Aufstieg gilt.
    if let Some(Tat::Schliessen(code)) = taten.first() {
        return einfach(status_fuer(*code), ("reason", grund_fuer(*code)));
    }

    // **Hier endet dieser Bau, und zwar benannt.** Um die Leitung zu
    // bedienen, braucht es `SinkExt`/`StreamExt`, und die stehen in
    // `futures-util`. Die Crate haengt laengst im Baum (ueber
    // tokio-tungstenite), aber sie DIREKT zu nennen ist eine Aufnahme und
    // damit eine Aufnahme, die vorgelegt werden muss: dass sie null zusaetzliche
    // Crates kostet, ist das Argument fuer ein Ja, nicht sein Ersatz.
    //
    // Was schon richtig ist und bleibt: jede ABWEISUNG oben. Sie braucht
    // keine Leitung, und ein Rufer, der nicht zuhoeren darf, bekommt seine
    // Zahl heute genauso wie spaeter. Nur der Fall, in dem er zuhoeren
    // DUERFTE, hat noch keinen Draht.
    {
        let mut app = dienst.app.lock().unwrap();
        app.horcher_austragen(&mut horcher);
    }
    einfach(503, ("reason", "this build serves no changes socket yet"))
}



/// Ein Schliesscode, übersetzt in die Sprache, die VOR dem Aufstieg gilt.
/// Dieselbe Entscheidung, zwei Sprachen: die Maschine kennt nur die eine.
fn status_fuer(code: u16) -> u16 {
    match code {
        maer_sync::horcher::NICHT_DER_WEG => 404,
        maer_sync::horcher::NICHT_BERECHTIGT => 401,
        maer_sync::horcher::NUR_INNEN => 426,
        maer_sync::horcher::SITZUNG_ZU_ENDE => 410,
        _ => 400,
    }
}

fn grund_fuer(code: u16) -> &'static str {
    match code {
        maer_sync::horcher::NICHT_DER_WEG => "no such path",
        maer_sync::horcher::NICHT_BERECHTIGT => "not authorized",
        maer_sync::horcher::NUR_INNEN => "this service speaks only inside a session",
        maer_sync::horcher::SITZUNG_ZU_ENDE => "this session has ended",
        _ => "this is not a sealed request",
    }
}
