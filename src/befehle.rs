// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Die Befehle des Betreibers.
//!
//! **Ohne sie ist das Erzeugnis nicht aufstellbar**: man kann kein Konto
//! anlegen und keine Sicherung ziehen. Das Lieferstück heißt „installieren
//! heißt kopieren", und dazu gehört, dass der Betreiber danach etwas tun kann.
//!
//! Namen, Argumente und **Ausgänge** sind die der Python-Fassung, gemessen in
//! `vektoren/befehle.json` und nicht nachgebaut. Ein Ausgang, der sich um eins
//! verschiebt, macht aus einer geglückten Sicherung eine gescheiterte, und der
//! Zeitplan des Betreibers alarmiert dann auf die falsche.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use crate::kennung::{adopt, key_path, make_new, open_identity, read_key, zeitmarke};
use crate::identity::{decide, fingerprint, public_of};
use crate::restore::{restore_verdict, RestoreVerdict};
use crate::store::Store;

/// Was ein Ladenfehler dem Betreiber SAGT. Die Debug-Form eines
/// SQLite-Fehlers ist fuer ihn unlesbar und traegt ausserdem Rusts
/// Typennamen; gemeint ist der Satz, den SQLite selbst schreibt.
pub fn satz(e: &crate::store::StoreError) -> String {
    match e {
        crate::store::StoreError::Sql(s) => s.to_string(),
        crate::store::StoreError::Refused(s) => s.clone(),
        crate::store::StoreError::Gate(s) => s.clone(),
        crate::store::StoreError::AheadOfService(s) => s.clone(),
    }
}

/// Wohin der Speicher zeigt, aus der Umgebung, mit derselben Vorgabe wie
/// Python.
pub fn db_pfad() -> String {
    std::env::var("MAER_SYNC_DB").unwrap_or_else(|_| "maer-sync.sqlite".into())
}

/// Den Laden kopieren und die Rückkehr **im selben Lauf** proben.
///
/// *Eine Sicherung, die nie geöffnet wurde, ist eine Datei, keine Sicherung.*
/// Deshalb sind es nicht zwei Schritte: wer den zweiten vergisst, hat eine
/// Datei und glaubt, er habe eine Sicherung.
///
/// Die Größen des lebenden Ladens werden **vor** der Kopie gelesen. Ein
/// Schrieb, der dazwischen landet, erscheint als `differs` mit dem Namen des
/// Dokuments. Das ist eine Probe zum Wiederholen, kein Fehlalarm zum
/// Abschalten.
pub fn backup_and_rehearse(lebend: &Store, kopie: &str) -> RestoreVerdict {
    let gesichert: BTreeMap<String, i64> = match lebend.sizes() {
        Ok(g) => g,
        Err(e) => return restore_verdict(None, None,
            Some(&format!("the copy did not get written: {}", satz(&e)))),
    };
    if let Err(e) = lebend.backup(kopie) {
        // Der SATZ des Fehlers, nicht seine Debug-Form. `{e:?}` gab
        // `Sql(SqliteFailure(Error { code: ... }))` und damit eine Zeile, die
        // ein Betreiber nicht lesen kann und die mit der Python-Fassung
        // nichts gemein hat.
        return restore_verdict(None, None,
            Some(&format!("the copy did not get written: {}", satz(&e))));
    }

    // Die Kopie wird in einem Wegwerf-Verzeichnis geöffnet, genau wie eine
    // Rückkehr sie öffnen würde, und sie muss die Gestaltsprüfung bestehen
    // wie jeder Laden.
    let wegwerf = std::env::temp_dir().join(format!("maer-probe-{}-{}",
        std::process::id(), zeitmarke()));
    if let Err(e) = std::fs::create_dir_all(&wegwerf) {
        return restore_verdict(Some(&gesichert), None,
            Some(&format!("the copy did not open as a store: {e}")));
    }
    let zurueck = wegwerf.join("restored.sqlite");
    let ergebnis = (|| -> Result<BTreeMap<String, i64>, String> {
        std::fs::copy(kopie, &zurueck).map_err(|e| e.to_string())?;
        let laden = Store::open(zurueck.to_str().unwrap_or(""), crate::store::DEFAULT_KEEP, None)
            .map_err(|e| format!("{e:?}"))?;
        laden.sizes().map_err(|e| format!("{e:?}"))
    })();
    let _ = std::fs::remove_dir_all(&wegwerf);

    match ergebnis {
        Err(e) => restore_verdict(Some(&gesichert), None,
            Some(&format!("the copy did not open as a store: {e}"))),
        Ok(gelesen) => restore_verdict(Some(&gesichert), Some(&gelesen), None),
    }
}

/// `backup <kopie.sqlite>`
pub fn befehl_backup(pfad: Option<&str>) -> i32 {
    let Some(pfad) = pfad.filter(|p| !p.is_empty()) else {
        eprintln!("usage: maer-sync backup <copy.sqlite>");
        return 2;
    };
    let laden = match Store::open(&db_pfad(), crate::store::DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => { eprintln!("the store cannot be opened: {e:?}"); return 2; }
    };
    let urteil = backup_and_rehearse(&laden, pfad);
    println!("{}: {}", urteil.outcome, urteil.reason);
    if urteil.passed() { 0 } else { 1 }
}

/// `account <owner>`, das Geheimnis von der Eingabe.
///
/// **Nie von der Befehlszeile**: ein Argument steht in der Prozessliste, die
/// jeder auf der Maschine lesen kann, und landet in der Geschichte der Shell.
pub fn befehl_account(owner: Option<&str>) -> i32 {
    let Some(owner) = owner.filter(|o| !o.is_empty()) else {
        eprintln!("usage: maer-sync account <owner>   (secret on stdin)");
        return 2;
    };
    let mut roh = String::new();
    if std::io::stdin().read_to_string(&mut roh).is_err() {
        eprintln!("no secret given");
        return 2;
    }
    let geheimnis = roh.lines().next().unwrap_or("").to_string();
    if geheimnis.is_empty() {
        eprintln!("no secret given");
        return 2;
    }
    let mut laden = match Store::open(&db_pfad(), crate::store::DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => { eprintln!("the store cannot be opened: {e:?}"); return 2; }
    };
    match laden.create_account(owner, &geheimnis) {
        Err(crate::store::StoreError::Refused(satz)) => { eprintln!("refused: {satz}"); 1 }
        Err(e) => { eprintln!("refused: {e:?}"); 1 }
        Ok(()) => { println!("account {owner} laid down"); 0 }
    }
}

/// `account-remove <owner>`, aber erst nach einer datierten Sicherung, die
/// ihre eigene Rückkehr geprobt hat.
///
/// *„Wenn es Daten frisst, geht das auf uns":* ein Tippfehler im Namen darf
/// keine Hörgeschichte kosten, für die es keinen Weg zurück gibt.
///
/// **Das Netz schützt Dokumente, die es gibt.** Ein leerer Laden hat nichts zu
/// verlieren, und seine Probe sagt `not measured` statt `fits`; nur auf
/// `passed` zu schauen wiese also eine harmlose Entfernung ab.
pub fn befehl_account_remove(owner: Option<&str>) -> i32 {
    let Some(owner) = owner.filter(|o| !o.is_empty()) else {
        eprintln!("usage: maer-sync account-remove <owner>");
        return 2;
    };
    let db = db_pfad();
    let kopie = format!("{db}.before-remove-{owner}-{}.sqlite", zeitmarke());
    let mut laden = match Store::open(&db, crate::store::DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => { eprintln!("the store cannot be opened: {e:?}"); return 2; }
    };
    let hatte_dokumente = laden.sizes().map(|s| !s.is_empty()).unwrap_or(false);
    let urteil = backup_and_rehearse(&laden, &kopie);
    if hatte_dokumente && !urteil.passed() {
        eprintln!("refused: the safety backup did not verify ({}); nothing removed",
                  urteil.reason);
        return 1;
    }
    match laden.remove_account(owner) {
        Err(e) => { eprintln!("refused: {e:?}"); 1 }
        Ok((0, _)) => { println!("no account named '{owner}' (backup at {kopie} kept)"); 0 }
        Ok((_, dokumente)) => {
            println!("account {owner} removed, documents dropped: {dokumente} \
(its tokens fell with it); backup at {kopie}");
            0
        }
    }
}

/// Wohin `health` fragt: die Bindung des Dienstes, und eine Bindung an ALLE
/// Adressen heisst fuer eine Frage von innen die Schleife.
///
/// `0.0.0.0:8099` im Container bindet alle Adressen, und dorthin kann man
/// nicht fragen; `127.0.0.1:8099` erreicht denselben Dienst. Eine Bindung, die
/// sich nicht lesen laesst, ist `None` und kein geratener Port: eine Probe, die
/// irgendwohin fragt, prueft den falschen Dienst und meldet ihn gesund.
pub fn ziel_aus_bindung(bindung: &str) -> Option<std::net::SocketAddr> {
    let a: std::net::SocketAddr = bindung.trim().parse().ok()?;
    let ip = match a.ip() {
        std::net::IpAddr::V4(v) if v.is_unspecified() => std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        std::net::IpAddr::V6(v) if v.is_unspecified() => std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        andere => andere,
    };
    Some(std::net::SocketAddr::new(ip, a.port()))
}

/// Ob eine Antwort `/v1/health` gesund ist: Status 200 UND der Koerper, den der
/// Dienst dort gibt. Nur der Status waere zu wenig: ein fremder Dienst auf
/// demselben Port antwortet auch mit 200.
pub fn gesund(antwort: &[u8]) -> bool {
    let text = String::from_utf8_lossy(antwort);
    let mut zeilen = text.lines();
    let status_ok = zeilen.next()
        .map(|z| z.starts_with("HTTP/1.1 200") || z.starts_with("HTTP/1.0 200"))
        .unwrap_or(false);
    let koerper = text.split("\r\n\r\n").nth(1).unwrap_or("");
    let koerper_ok = serde_json::from_str::<serde_json::Value>(koerper.trim())
        .ok()
        .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
        .unwrap_or(false);
    status_ok && koerper_ok
}

/// `health`: fragt den laufenden Dienst nach `/v1/health`, 0 fuer gesund, 1
/// fuer alles andere.
///
/// **Fuer den Container**, der kein curl und kein Python traegt: das Abbild
/// enthaelt nur dieses Binary, also fragt es sich selbst. Ohne HTTP-Klienten
/// aus einer Crate (das waere eine Aufnahme), von Hand ueber TCP: eine Zeile
/// hin, die Antwort zurueck, drei Sekunden Frist.
pub fn befehl_health() -> i32 {
    let bindung = std::env::var("MAER_SYNC_BIND").unwrap_or_else(|_| "127.0.0.1:8081".into());
    let Some(ziel) = ziel_aus_bindung(&bindung) else {
        eprintln!("not healthy: MAER_SYNC_BIND is not an address: {bindung}");
        return 1;
    };
    match gesundheit_fragen(ziel) {
        Ok(satz) => { println!("{satz}"); 0 }
        Err(satz) => { eprintln!("{satz}"); 1 }
    }
}

/// Die eigentliche Frage an `ziel`, ohne Umgebung: `Ok` mit dem Satz fuer
/// gesund, `Err` mit dem Grund fuer alles andere.
pub fn gesundheit_fragen(ziel: std::net::SocketAddr) -> Result<String, String> {
    use std::io::{Read as _, Write as _};
    let frist = std::time::Duration::from_secs(3);
    let mut leitung = std::net::TcpStream::connect_timeout(&ziel, frist)
        .map_err(|e| format!("not healthy: nothing answers on {ziel}: {e}"))?;
    let _ = leitung.set_read_timeout(Some(frist));
    let _ = leitung.set_write_timeout(Some(frist));
    let frage = format!("GET /v1/health HTTP/1.1\r\nHost: {ziel}\r\nConnection: close\r\n\r\n");
    leitung.write_all(frage.as_bytes())
        .map_err(|e| format!("not healthy: the question did not go out: {e}"))?;
    let mut antwort = Vec::new();
    let _ = leitung.take(4096).read_to_end(&mut antwort);
    if gesund(&antwort) {
        Ok(format!("healthy: {ziel}"))
    } else {
        let erste = String::from_utf8_lossy(&antwort).lines().next().unwrap_or("").to_string();
        Err(format!("not healthy: {ziel} answered {erste:?}"))
    }
}

/// `reset <owner>`: ein Einmal-Passwort fuer ein Konto, dessen Passwort
/// verloren ist.
///
/// **Der Dienst erzeugt es, nicht der Betreiber**, und zeigt es genau einmal.
/// Es taugt nur zum Wechsel: eine Anmeldung damit antwortet 428, kein Zeichen
/// entsteht, und das Geraet fragt sofort ein neues Passwort. Jedes Zeichen des
/// Kontos ist danach zurueckgezogen. Der Betreiber kennt das Einmal-Passwort,
/// und deshalb darf es nie der Schluessel der Feed-Zugaenge werden: das
/// verhindert, dass das Konto in die Form `secret` faellt.
pub fn befehl_reset(owner: Option<&str>) -> i32 {
    let Some(owner) = owner.filter(|o| !o.is_empty()) else {
        eprintln!("usage: maer-sync reset <owner>");
        return 2;
    };
    let mut laden = match Store::open(&db_pfad(), crate::store::DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => { eprintln!("the store cannot be opened: {e:?}"); return 2; }
    };
    match laden.zuruecksetzen(owner) {
        Err(e) => { eprintln!("refused: {}", satz(&e)); 1 }
        Ok(None) => { eprintln!("no account named '{owner}'"); 1 }
        Ok(Some(einmal)) => {
            println!("one-time password for {owner}: {einmal}");
            println!("it only changes the password; every token of {owner} is withdrawn");
            0
        }
    }
}

/// `devices <owner>`: je Geraet die Form der letzten Anmeldung.
///
/// Die Messung fuer Bedingung (b): bevor ein Konto den Schluessel der
/// Feed-Zugaenge aus dem Passwort nimmt, darf sich seit dem Wechsel kein
/// Geraet mehr mit einem Passwort angemeldet haben, das der Dienst kennt.
/// Formen: `proof` (Fassung 6), `secret` (Fassung 5), `move` (der Umzug),
/// `change` (ein Wechsel), `reset` (ein Wechsel nach dem Einmal-Passwort).
pub fn befehl_devices(owner: Option<&str>) -> i32 {
    let Some(owner) = owner.filter(|o| !o.is_empty()) else {
        eprintln!("usage: maer-sync devices <owner>");
        return 2;
    };
    let laden = match Store::open(&db_pfad(), crate::store::DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => { eprintln!("the store cannot be opened: {e:?}"); return 2; }
    };
    let konto = match laden.konto(owner) {
        Ok(Some(k)) => k,
        Ok(None) => { eprintln!("no account named '{owner}'"); return 1; }
        Err(e) => { eprintln!("refused: {}", satz(&e)); return 1; }
    };
    let umgezogen = konto.umgezogen.map(|t| format!("{t:.0}")).unwrap_or_else(|| "not yet".into());
    println!("account {owner}: form={} set-by-operator={} moved-at={umgezogen}",
             konto.form, if konto.betreiber { "yes" } else { "no" });
    match laden.geraete(owner) {
        Err(e) => { eprintln!("refused: {}", satz(&e)); 1 }
        Ok(g) if g.is_empty() => { println!("no device has signed in since version 6"); 0 }
        Ok(g) => {
            for (geraet, form, zeit) in g {
                let name = if geraet.is_empty() { "-".to_string() } else { geraet };
                println!("device={name} form={form} at={zeit:.0}");
            }
            0
        }
    }
}

/// `key`, `key --adopt`, `key --new`.
///
/// Drei Befehle in einem, weil sie eine Frage beantworten: wer ist dieser
/// Dienst. Zeigen ist der gewöhnliche Fall und ändert nichts, deshalb ist es
/// die Form ohne Schalter.
pub fn befehl_key(uebernehmen: bool, neu: bool) -> i32 {
    if uebernehmen && neu {
        eprintln!("--adopt takes the key that is there, --new makes one. Not both.");
        return 2;
    }
    let db = db_pfad();
    let pfad = key_path(Some(&db));
    let mut laden = match Store::open(&db, crate::store::DEFAULT_KEEP, None) {
        Ok(l) => l,
        Err(e) => { eprintln!("the store cannot be opened: {e:?}"); return 2; }
    };

    if uebernehmen {
        return match adopt(&mut laden, &pfad) {
            Err(e) => { eprintln!("refused: {}", e.0); 1 }
            Ok((gefunden, war)) => {
                println!("this service now answers as {gefunden}");
                if let Some(w) = war.filter(|w| *w != gefunden) {
                    println!("it answered as {w} before; every device that pinned that one \
will ask about the change");
                }
                0
            }
        };
    }
    if neu {
        return match make_new(&mut laden, &pfad) {
            Err(e) => { eprintln!("refused: {}", e.0); 1 }
            Ok((gefunden, war, behalten)) => {
                println!("a new identity was made: {gefunden}");
                if let Some(w) = war {
                    println!("it answered as {w} before. EVERY device that pinned that one \
will refuse this service until somebody accepts the new key.");
                }
                if let Some(b) = behalten {
                    println!("the previous key file was kept at {b}, not overwritten");
                }
                0
            }
        };
    }

    // Zeigen: liest nur. Eine Auskunft darf den Zustand nicht aendern, sonst
    // kann man nicht nachsehen, ohne einzugreifen.
    let privat = match read_key(&pfad) {
        Ok(p) => p,
        Err(e) => { eprintln!("refused: {}", e.0); return 1; }
    };
    let gefunden = match privat.map(|p| fingerprint(&public_of(&p))).transpose() {
        Ok(g) => g,
        Err(e) => { eprintln!("refused: {e}"); return 1; }
    };
    let gemerkt = laden.remembered_key().ok().flatten();
    let urteil = decide(gefunden.as_deref(), gemerkt.as_deref());
    println!("key file: {}", pfad.display());
    println!("fingerprint: {}", gefunden.as_deref().unwrap_or("none here"));
    println!("this store expects: {}", gemerkt.as_deref().unwrap_or("nothing yet"));
    println!("{}", urteil.reason);
    if urteil.may_serve { 0 } else { 1 }
}

/// Der Dienst selbst, damit `open_identity` und `Path` hier gebraucht werden
/// und der Ruf aus `main` eine Stelle hat.
pub fn kennung_oeffnen(laden: &mut Store, db: &str)
    -> Result<([u8; 32], String), crate::kennung::RefusedToServe>
{
    let pfad: std::path::PathBuf = key_path(Some(db));
    open_identity(laden, Path::new(&pfad))
}
