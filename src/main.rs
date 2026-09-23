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
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message, Role};
use tokio_tungstenite::WebSocketStream;

use maer_sync::aufstieg::accept_key;
use maer_sync::befehle;
use maer_sync::horcher::{Tat, Wort};
use maer_sync::kennung::{key_path, open_identity};
use maer_sync::protokoll::{jetzt, Art};
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
    /// Die Zeilen wegtragen, die seit dem letzten Mal entschieden wurden:
    /// stempeln, und bei einem Zugriff die Dauer anhaengen, dann auf stdout.
    ///
    /// **Hier, und nur hier, wird geschrieben.** `wege.rs` entscheidet, dass
    /// eine Zeile zu schreiben ist, und kennt weder Uhr noch Strom; dieser
    /// Transport kennt beides. Bis Scheibe 6 Teil 6 fehlte er ganz: die
    /// Abweisungen entstanden und blieben im Speicher, und `protokoll.txt`
    /// hatte nach acht Stunden Betrieb zwei Zeilen (gemessen am 19.09.2026).
    fn protokoll_austeilen(&self, beginn: std::time::Instant) {
        let zeilen: Vec<maer_sync::protokoll::Zeile> = {
            let mut app = self.app.lock().unwrap();
            app.protokoll.drain(..).collect()
        };
        for z in zeilen {
            let dauer = if z.art == Art::Zugriff {
                format!(" ms={}", beginn.elapsed().as_millis())
            } else { String::new() };
            zeile_schreiben(&format!("{} {} maer.sync {}{}", jetzt(), z.stufe.name(), z.satz, dauer));
        }
    }

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

/// Eine Zeile auf stdout, sofort. Der Strom ist zeilenweise gepuffert, aber
/// ein Betreiber, der `tail -f` liest, soll nicht darauf wetten muessen:
/// das Leeren steht ausdruecklich da.
fn zeile_schreiben(zeile: &str) {
    use std::io::Write;
    let aus = std::io::stdout();
    let mut l = aus.lock();
    let _ = writeln!(l, "{zeile}");
    let _ = l.flush();
}

/// Eine Zugriffszeile, die der TRANSPORT selbst schreibt, weil die Anfrage
/// die Entscheidung nie erreicht hat (Koerper abgebrochen oder zu gross) oder
/// weil sie ein Aufstieg ist, den `wege.rs` gar nicht sieht. Dieselbe
/// Gestalt wie die Zeilen aus `wege.rs`, damit ein Leser nicht zwei Formen
/// lernen muss.
fn zugriff_schreiben(von: &str, method: &str, pfad: &str, status: u16, grund: &str,
                     beginn: std::time::Instant)
{
    let reason = if grund.is_empty() { String::new() } else { format!(" reason={grund}") };
    zeile_schreiben(&format!(
        "{} INFO maer.sync access from={von} session=- inner=false method={method} path={pfad} \
scope=- status={status}{reason} ms={}", jetzt(), beginn.elapsed().as_millis()));
}

/// Die Befehle des Betreibers, und `serve` als Vorgabe ohne Befehl.
///
/// Von Hand zerlegt statt mit einer Crate: es sind fuenf Befehle und zwei
/// Schalter, und eine Abhaengigkeit dafuer waere mehr, als die Frage wert
/// ist. Namen, Argumente und **Ausgaenge** sind die der Python-Fassung,
/// gemessen in `vektoren/befehle.json`.
///
/// **Was NICHT nachgebaut wird und warum:** bei einem fehlenden Argument
/// antwortet dort `argparse` mit einem Wortlaut, der den Python-Aufruf nennt.
/// Diese Fassung nennt ihren eigenen Aufruf und trifft denselben Ausgang; die
/// Tafel haelt diese zwei Faelle als `satz_von: argparse` fest, damit der
/// Unterschied benannt ist und nicht stillschweigend uebergangen wird.
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wort = |i: usize| args.get(i).map(|s| s.as_str());
    match wort(0) {
        Some("backup") =>
            return std::process::ExitCode::from(befehle::befehl_backup(wort(1)) as u8),
        Some("account") =>
            return std::process::ExitCode::from(befehle::befehl_account(wort(1)) as u8),
        Some("account-remove") =>
            return std::process::ExitCode::from(befehle::befehl_account_remove(wort(1)) as u8),
        Some("health") =>
            return std::process::ExitCode::from(befehle::befehl_health() as u8),
        Some("key") => {
            let hat = |f: &str| args.iter().any(|a| a == f);
            return std::process::ExitCode::from(
                befehle::befehl_key(hat("--adopt"), hat("--new")) as u8);
        }
        Some("--help") | Some("-h") | Some("help") => {
            // Wie drueben: die Form nennen und mit 0 gehen, nicht dienen.
            // Der gefaehrliche Fall waere, dass `--help` versehentlich den
            // Dienst startet.
            println!("usage: maer-sync [serve|backup <copy.sqlite>|account <owner>|\
account-remove <owner>|health|key [--adopt|--new]]");
            println!();
            println!("The Mar sync service and its operator commands.");
            println!("  serve            run the service (the default with no command)");
            println!("  backup PATH      copy the store and rehearse the restore");
            println!("  account OWNER    lay down an account (secret on stdin)");
            println!("  account-remove OWNER");
            println!("  health           ask the running service, exit 0 when it answers well");
            println!("  key [--adopt|--new]");
            return std::process::ExitCode::SUCCESS;
        }
        Some("serve") | None => {}
        Some(anderes) => {
            eprintln!("no such command: {anderes}");
            eprintln!("usage: maer-sync [serve|backup <copy.sqlite>|account <owner>|\
account-remove <owner>|health|key [--adopt|--new]]");
            return std::process::ExitCode::from(2);
        }
    }

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
    let beginn = std::time::Instant::now();
    let kopfzeilen: std::collections::BTreeMap<String, String> = anfrage.headers().iter()
        .map(|(k, v)| (k.as_str().to_ascii_lowercase(),
                       v.to_str().unwrap_or("").to_string()))
        .collect();

    if ist_aufstieg(&kopfzeilen) {
        return Ok(aufsteigen(dienst, anfrage, kopfzeilen, von, beginn));
    }

    let method = anfrage.method().as_str().to_string();
    let pfad = anfrage.uri().path().to_string();
    let frage = anfrage.uri().query().unwrap_or("").as_bytes().to_vec();
    let grenze = { dienst.app.lock().unwrap().max_body_bytes };
    let angekuendigt = kopfzeilen.get("content-length").and_then(|v| v.parse::<usize>().ok());

    let koerper = match koerper_lesen(anfrage.into_body(), grenze, angekuendigt).await {
        Ok(k) => k,
        Err(KoerperFehler::Unvollstaendig) => {
            let satz = satz_unvollstaendig(&pfad);
            zugriff_schreiben(&von, &method, &pfad, 400, satz.1, beginn);
            return Ok(einfach(400, satz));
        }
        Err(KoerperFehler::ZuGross) => {
            let satz = satz_zu_gross(&pfad);
            zugriff_schreiben(&von, &method, &pfad, 413, satz.1, beginn);
            return Ok(einfach(413, satz));
        }
    };

    let gab = {
        let mut app = dienst.app.lock().unwrap();
        app.http(&Anfrage {
            method, path: pfad, query: frage, headers: kopfzeilen,
            body: koerper, innen: false, client: Some(von), sitzung: None,
        })
    };
    dienst.protokoll_austeilen(beginn);
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
              kopfzeilen: std::collections::BTreeMap<String, String>, _von: String,
              beginn: std::time::Instant)
    -> Response<Voll>
{
    let pfad = anfrage.uri().path().to_string();
    let Some(schluessel) = kopfzeilen.get("sec-websocket-key").cloned() else {
        zugriff_schreiben(&_von, "GET", &pfad, 400, "this upgrade names no key", beginn);
        return einfach(400, ("reason", "this upgrade names no key"));
    };

    let (mut horcher, taten) = {
        let mut app = dienst.app.lock().unwrap();
        app.horcher_oeffnen(&pfad, &kopfzeilen, Some(&_von))
    };
    // Eine Abweisung reist als Schliesscode, und ohne Aufstieg gibt es keine
    // Leitung, auf der er reisen könnte. Also sagt es der Status: dieselbe
    // Entscheidung, nur in der Sprache, die vor dem Aufstieg gilt.
    dienst.protokoll_austeilen(beginn);
    if let Some(Tat::Schliessen(code)) = taten.first() {
        // Der Status ist die 403, die vor dem Aufstieg gilt; der Grund ist der
        // Schliesscode, den der Rufer NICHT sieht (gemessen: uvicorn
        // verschluckt ihn). Hier steht er, damit der Betreiber ihn hat.
        zugriff_schreiben(&_von, "GET", &pfad, 403, &format!("{} code={code}", grund_fuer(*code)), beginn);
        return abgewiesener_aufstieg();
    }
    zugriff_schreiben(&_von, "GET", &pfad, 101, "", beginn);

    let (an_den_draht, mut vom_dienst) = unbounded_channel::<Tat>();
    {
        let mut draehte = dienst.draehte.lock().unwrap();
        draehte.insert(horcher.id.clone(), an_den_draht.clone());
    }

    let id = horcher.id.clone();
    let mitschrift = Arc::clone(&dienst);
    let von_h = _von.clone();
    tokio::spawn(async move {
        let aufgestiegen = match hyper::upgrade::on(anfrage).await {
            Ok(a) => a,
            Err(_) => return aufraeumen(&mitschrift, &mut horcher, &id),
        };
        let leitung = WebSocketStream::from_raw_socket(
            TokioIo::new(aufgestiegen), Role::Server, None).await;

        // **Zwei Aufgaben statt `select!`.** Die eine haelt die Schreibseite
        // und leert den Kanal, die andere liest und legt hinein, was daraus
        // folgt. Damit gibt es genau einen Schreiber, die Reihenfolge der
        // Worte auf der Leitung ist die des Kanals, und das Merkmal `macros`
        // (eine Crate fuer eine Zeile) bleibt draussen.
        use futures_util::StreamExt;
        let (mut schreibt, mut liest) = leitung.split();
        let von_s = von_h.clone();
        let schreiber = tokio::spawn(async move {
            use futures_util::SinkExt;
            while let Some(tat) = vom_dienst.recv().await {
                let nachricht = match tat {
                    // Das Annehmen geschah eine Schicht tiefer: die 101 ging
                    // hinaus, bevor der Aufstieg ueberhaupt begann.
                    Tat::Annehmen => continue,
                    Tat::SendeText(t) => Message::Text(t.into()),
                    Tat::SendeBytes(b) => Message::Binary(b.into()),
                    Tat::Schliessen(code) => {
                        zeile_schreiben(&format!(
                            "{} INFO maer.sync socket close from={von_s} code={code} reason={}",
                            jetzt(), grund_fuer(code)));
                        let _ = schreibt.send(Message::Close(Some(CloseFrame {
                            code: code.into(), reason: grund_fuer(code).into(),
                        }))).await;
                        break;
                    }
                };
                if schreibt.send(nachricht).await.is_err() {
                    break;
                }
            }
            let _ = schreibt.close().await;
        });

        // Das Verbinden ist fuer die Maschine ein eigenes Wort: erst danach
        // steht der Horcher auf der Ansageliste.
        let taten = {
            let mut app = mitschrift.app.lock().unwrap();
            app.horcher_wort(&mut horcher, Wort::Verbunden)
        };
        mitschrift.protokoll_austeilen(beginn);
        for tat in taten {
            let _ = an_den_draht.send(tat);
        }

        while let Some(herein) = liest.next().await {
            let wort = match herein {
                Err(_) => Wort::Getrennt,
                Ok(Message::Binary(b)) => Wort::Bytes(b.to_vec()),
                Ok(Message::Text(t)) => Wort::Text(t.to_string()),
                Ok(Message::Close(_)) => Wort::Getrennt,
                // Steuerworte beantwortet die Leitung selbst.
                Ok(_) => continue,
            };
            let fertig = wort == Wort::Getrennt;
            let taten = {
                let mut app = mitschrift.app.lock().unwrap();
                app.horcher_wort(&mut horcher, wort)
            };
            mitschrift.protokoll_austeilen(beginn);
            let schliesst = taten.iter().any(|t| matches!(t, Tat::Schliessen(_)));
            for tat in taten {
                let _ = an_den_draht.send(tat);
            }
            if fertig || schliesst {
                break;
            }
        }
        // Beide Haelften des Kanals fallen lassen, sonst wartet der Schreiber
        // ewig auf ein Wort, das nicht mehr kommt.
        drop(an_den_draht);
        aufraeumen(&mitschrift, &mut horcher, &id);
        zeile_schreiben(&format!("{} INFO maer.sync socket closed from={von_h}", jetzt()));
        let _ = schreiber.await;
    });

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("upgrade", "websocket")
        .header("connection", "Upgrade")
        .header("sec-websocket-accept", accept_key(&schluessel))
        .body(Voll(None))
        .expect("die Antwort ist wohlgeformt")
}

/// Einen Horcher von beiden Listen nehmen: von der Ansageliste der
/// Entscheidung und von den Draehten des Transports.
fn aufraeumen(dienst: &Dienst, horcher: &mut maer_sync::horcher::Horcher, id: &str) {
    {
        let mut app = dienst.app.lock().unwrap();
        app.horcher_austragen(horcher);
    }
    dienst.draehte.lock().unwrap().remove(id);
}

/// Ein abgewiesener Aufstieg, GENAU wie die Python-Aufstellung ihn abweist.
///
/// **Gemessen, nicht angenommen** (18.09.2026, Fassung 5 aus `main` unter
/// uvicorn 0.53.0 in eigener venv): ein `websocket.close` VOR dem `accept`
/// wird zu `403` mit LEEREM Koerper, bei jedem Grund gleich, ob falscher Weg,
/// fehlendes Zeichen, unbekanntes Zeichen oder erfundene Sitzung. Der
/// Schliesscode, den unser Python-Code sehr wohl setzt, geht also im SERVER
/// verloren, nicht bei uns.
///
/// **Was NICHT nachgebildet wird, und warum.** Die zwei gemessenen
/// uvicorn-Fassungen antworten verschieden: 0.52.4 schickt `Connection:
/// close`, `Content-Type: text/plain` und eine Grundphrase, 0.53.0 nur
/// `content-length: 0`. Stabil ist allein **Status und leerer Koerper**; der
/// Rest ist Zierrat dessen, der serviert, und ihn nachzubauen hiesse, sich an
/// eine Fassung eines fremden Servers zu binden.
///
/// **Warum ueberhaupt nachgebildet statt verbessert.** Eine feiner
/// unterscheidende Antwort (404, 401, 426, 410) waere nuetzlicher und stand
/// hier auch kurz. Sie macht die zwei Umsetzungen aber von aussen
/// UNTERSCHEIDBAR, und genau das soll dieser Bau nicht; ausserdem liest die
/// App den Status heute gar nicht. Die bessere Auskunft ist nicht verworfen,
/// sondern an die richtige Stelle verschoben: sie gehoert in den VERTRAG,
/// dann machen es beide Seiten (Entscheid 18.09.2026).
fn abgewiesener_aufstieg() -> Response<Voll> {
    // `content-length: 0` ist nicht Zierrat, sondern noetig: ohne sie waehlt
    // hyper `chunked` und schickt die Abschlussmarke, also fuenf Byte auf der
    // Leitung, wo uvicorn null schickt. Gemessen, nicht vermutet.
    Response::builder()
        .status(403)
        .header("content-length", "0")
        .body(Voll(None))
        .expect("die Antwort ist wohlgeformt")
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
