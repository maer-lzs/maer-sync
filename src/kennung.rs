// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der statische Schlüssel als DATEI: lesen, hinlegen, und die eine Tür, die
//! der Dienst benutzt.
//!
//! Die ENTSCHEIDUNG liegt in `identity::decide` und ist rein, an Vektoren
//! bezeugt und ohne Platte. Hier steht nur, was sie nicht sein kann: eine
//! Datei, ihre Rechte und der Vermerk im Speicher. Die Trennung ist der
//! Grund, warum die fünf Fälle überhaupt prüfbar sind.
//!
//! **Ein statischer Schlüssel, der sich still ändert, ist keiner.** Deshalb
//! schreibt die Tür in genau zwei Fällen, die die reine Entscheidung schon
//! erlaubt hat: sie macht den ersten Schlüssel, und sie vermerkt einen, den
//! der Speicher nie gesehen hat. Sie ersetzt nie einen.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::authorization::zufall;
use crate::identity::{decide, fingerprint, public_of};
use crate::store::{Store, StoreError};
use crate::wege::{base64, base64_lesen};

pub const KEY_NAME: &str = "identity.key";
pub const KEY_MODE: u32 = 0o600;

/// Der Dienst weigert sich zu dienen, und sagt warum.
#[derive(Debug)]
pub struct RefusedToServe(pub String);

/// Wo der Schlüssel liegt: `MAER_SYNC_KEY`, sonst neben der Datenbank.
pub fn key_path(db_path: Option<&str>) -> PathBuf {
    if let Ok(genannt) = std::env::var("MAER_SYNC_KEY") {
        if !genannt.is_empty() {
            return PathBuf::from(genannt);
        }
    }
    let db = db_path.map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("MAER_SYNC_DB").unwrap_or_else(|_| "maer-sync.sqlite".into()))
    });
    db.parent().unwrap_or(Path::new(".")).join(KEY_NAME)
}

/// Der private Schlüssel aus seiner Datei, oder nichts, wenn es keine gibt.
///
/// Eine Datei, die DA ist und sich nicht als Schlüssel lesen lässt, gilt
/// **nicht** als Abwesenheit. Abwesend heißt „dieser Dienst ist neu"; eine
/// beschädigte Datei heißt, dass jemand einen Schlüssel hat und es nicht
/// dieser ist, und neu anzufangen würfe den echten weg.
pub fn read_key(pfad: &Path) -> Result<Option<[u8; 32]>, RefusedToServe> {
    if !pfad.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(pfad)
        .map_err(|e| RefusedToServe(format!(
            "the key file {} is there but cannot be read ({e}). It is not being replaced.",
            pfad.display())))?;
    let Some(roh) = base64_lesen(&text) else {
        return Err(RefusedToServe(format!(
            "the key file {} is there but does not read as a key. It is not being replaced, \
because a new key here would lose whatever the damaged one was. Restore the file or move it \
aside deliberately.", pfad.display())));
    };
    if roh.len() != 32 {
        return Err(RefusedToServe(format!(
            "the key file {} holds {} bytes where a key is 32. It is not being replaced; \
restore the file or move it aside deliberately.", pfad.display(), roh.len())));
    }
    warnen_wenn_andere_lesen_koennen(pfad);
    let mut k = [0u8; 32];
    k.copy_from_slice(&roh);
    Ok(Some(k))
}

/// Den Schlüssel hinlegen, nur für den Eigentümer, **ohne ein Fenster, in dem
/// er weiter offen steht.**
///
/// Die Rechte stehen an einem frischen Deskriptor und nicht in einem `chmod`
/// hinterher: zwischen einem Öffnen und einem `chmod` existiert die Datei mit
/// der Maske des Prozesses, und ein Schlüssel, den die Welt eine Millisekunde
/// lesen konnte, war für die Welt lesbar.
pub fn write_key(pfad: &Path, privat: &[u8; 32]) -> Result<(), RefusedToServe> {
    if let Some(ordner) = pfad.parent() {
        std::fs::create_dir_all(ordner).map_err(|e| RefusedToServe(
            format!("the directory for {} cannot be made ({e})", pfad.display())))?;
    }
    let mut datei = std::fs::OpenOptions::new()
        .write(true).create(true).truncate(true).mode(KEY_MODE)
        .open(pfad)
        .map_err(|e| RefusedToServe(format!("{} cannot be written ({e})", pfad.display())))?;
    datei.write_all(base64(privat).as_bytes())
        .and_then(|_| datei.write_all(b"\n"))
        .map_err(|e| RefusedToServe(format!("{} cannot be written ({e})", pfad.display())))?;
    Ok(())
}

/// Sagen, nicht darüber abweisen.
///
/// Ein Schlüssel, den jeder lesen kann, ist ein echter Befund, und er ist
/// zugleich die Gestalt, in der eine Einhängung oder ein zurückgeholtes
/// Archiv ohne jemandes Schuld ankommt.
fn warnen_wenn_andere_lesen_koennen(pfad: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(daten) = std::fs::metadata(pfad) {
        let rechte = daten.permissions().mode() & 0o777;
        if rechte & 0o077 != 0 {
            eprintln!("the key file {} is readable beyond its owner (mode {:o})",
                      pfad.display(), rechte);
        }
    }
}

/// Der Schlüssel, mit dem dieser Dienst antwortet, oder eine Weigerung.
pub fn open_identity(laden: &mut Store, pfad: &Path)
    -> Result<([u8; 32], String), RefusedToServe>
{
    let privat = read_key(pfad)?;
    let gefunden = privat.map(|p| fingerprint(&public_of(&p)))
        .transpose()
        .map_err(RefusedToServe)?;
    let gemerkt = laden.remembered_key()
        .map_err(|e: StoreError| RefusedToServe(format!("the store cannot be read ({e:?})")))?;

    let urteil = decide(gefunden.as_deref(), gemerkt.as_deref());
    if !urteil.may_serve {
        return Err(RefusedToServe(urteil.reason));
    }

    let (privat, gefunden) = if urteil.name == "make_one" {
        let mut neu = [0u8; 32];
        neu.copy_from_slice(&zufall(32).map_err(RefusedToServe)?);
        write_key(pfad, &neu)?;
        let abdruck = fingerprint(&public_of(&neu)).map_err(RefusedToServe)?;
        eprintln!("a static key was made: {abdruck} (every device that connects will pin it)");
        (neu, abdruck)
    } else {
        (privat.expect("jeder andere Fall hat einen Schluessel"),
         gefunden.expect("jeder andere Fall hat einen Abdruck"))
    };

    if urteil.name == "make_one" || urteil.name == "remember" {
        laden.remember_key(&gefunden)
            .map_err(|e| RefusedToServe(format!("the store cannot be written ({e:?})")))?;
        eprintln!("this service now answers as {gefunden}");
    } else {
        eprintln!("serving as {gefunden}");
    }
    Ok((privat, gefunden))
}


/// Sagen, dass der Schluessel, der hier liegt, jetzt der richtige ist.
///
/// Der Weg des Betreibers, eine Aenderung zu WOLLEN, nachdem ein Dienst
/// umgezogen ist oder auf eine Maschine zurueckgeholt wurde, die ihren
/// eigenen Schluessel hat. Es vermerkt; es macht nie einen, denn ein
/// `adopt`, das still erzeugte, waere genau die stille Ersetzung, gegen die
/// diese Datei geschrieben ist.
pub fn adopt(laden: &mut Store, pfad: &Path) -> Result<(String, Option<String>), RefusedToServe> {
    let Some(privat) = read_key(pfad)? else {
        return Err(RefusedToServe(format!(
            "there is no key file at {} to adopt. To make a new identity, which every \
device will have to accept again, run `key --new`.", pfad.display())));
    };
    let gefunden = fingerprint(&public_of(&privat)).map_err(RefusedToServe)?;
    let war = laden.remembered_key()
        .map_err(|e| RefusedToServe(format!("the store cannot be read ({e:?})")))?;
    laden.remember_key(&gefunden)
        .map_err(|e| RefusedToServe(format!("the store cannot be written ({e:?})")))?;
    Ok((gefunden, war))
}

/// Eine neue Kennung machen, mit Absicht, und sagen, was sie kostet.
///
/// **Die alte Schluesseldatei wird BEHALTEN, nicht ueberschrieben**, und ihr
/// Name traegt ein Datum. Eine versehentliche Drehung ist sonst nicht
/// rueckholbar, und das einzige, was schlimmer ist als ein Identitaetswechsel,
/// ist einer, den man nicht rueckgaengig machen kann.
///
/// Innerhalb einer Sekunde traefe eine zweite Drehung denselben Namen, und
/// diese Funktion ist dafuer da, keine Schluessel zu verlieren: dann wird
/// hochgezaehlt.
pub fn make_new(laden: &mut Store, pfad: &Path)
    -> Result<(String, Option<String>, Option<String>), RefusedToServe>
{
    let war = laden.remembered_key()
        .map_err(|e| RefusedToServe(format!("the store cannot be read ({e:?})")))?;
    let mut behalten = None;
    if pfad.exists() {
        let marke = zeitmarke();
        let name = pfad.file_name().and_then(|n| n.to_str()).unwrap_or("identity.key");
        let mut ziel = pfad.with_file_name(format!("{name}.was-{marke}"));
        let mut n = 2;
        while ziel.exists() {
            ziel = pfad.with_file_name(format!("{name}.was-{marke}-{n}"));
            n += 1;
        }
        std::fs::rename(pfad, &ziel).map_err(|e| RefusedToServe(format!(
            "the previous key at {} could not be kept ({e}); nothing was replaced",
            pfad.display())))?;
        behalten = Some(ziel.display().to_string());
    }
    let mut neu = [0u8; 32];
    neu.copy_from_slice(&zufall(32).map_err(RefusedToServe)?);
    write_key(pfad, &neu)?;
    let gefunden = fingerprint(&public_of(&neu)).map_err(RefusedToServe)?;
    laden.remember_key(&gefunden)
        .map_err(|e| RefusedToServe(format!("the store cannot be written ({e:?})")))?;
    Ok((gefunden, war, behalten))
}

/// `JJJJMMTT-HHMMSS` in Ortszeit, dieselbe Gestalt, die `account-remove`
/// benutzt. Von Hand gerechnet, weil eine Crate fuer eine Zeitmarke mehr
/// waere, als die Frage wert ist.
pub fn zeitmarke() -> String {
    let jetzt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Ortszeit ueber die Umgebung zu holen hiesse, eine Zeitzonen-Datenbank
    // zu lesen. Hier zaehlt nur, dass der Name EINDEUTIG und lesbar ist,
    // nicht, dass er die Zone des Wirts trifft; bei Gleichstand zaehlt die
    // Schleife oben hoch.
    let tage = jetzt / 86400;
    let rest = jetzt % 86400;
    let (j, m, t) = aus_tagen(tage);
    format!("{j:04}{m:02}{t:02}-{:02}{:02}{:02}", rest / 3600, (rest % 3600) / 60, rest % 60)
}

/// Tage seit 1970 in Jahr, Monat, Tag. Die buergerliche Rechnung, ohne Crate.
fn aus_tagen(mut tage: i64) -> (i64, i64, i64) {
    let mut jahr = 1970;
    loop {
        let laenge = if schaltjahr(jahr) { 366 } else { 365 };
        if tage < laenge { break; }
        tage -= laenge;
        jahr += 1;
    }
    let monate = [31, if schaltjahr(jahr) { 29 } else { 28 }, 31, 30, 31, 30,
                  31, 31, 30, 31, 30, 31];
    let mut monat = 1;
    for l in monate {
        if tage < l { break; }
        tage -= l;
        monat += 1;
    }
    (jahr, monat, tage + 1)
}

fn schaltjahr(j: i64) -> bool { (j % 4 == 0 && j % 100 != 0) || j % 400 == 0 }
