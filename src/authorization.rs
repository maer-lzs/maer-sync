// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wer fragt, und ob er darf.
//!
//! Die Forderung stellt das mit Absicht beiseite: "getrennt zu behandeln und
//! getrennt abzunehmen. Wenn hier ein Fehler ist, stehen die Hoerstaende
//! anderer Leute offen, und das ist die eine Sorte Fehler, die man nicht mit
//! einem Update repariert."
//!
//! Drei Dinge, um die herum das hier gebaut ist, und jedes davon ist ein
//! Fehler, der leicht zu machen und teuer zu finden ist:
//!
//! - **Eine Antwort fuer jedes Scheitern.** Ein fehlendes Konto und ein
//!   falsches Geheimnis geben denselben Satz. Alles andere macht den Zugang
//!   zu einer Liste, wer hier ein Konto hat.
//! - **Zeitkonstanter Vergleich.** Ein Vergleich, der beim ersten falschen
//!   Byte zurueckkehrt, verraet das Geheimnis Byte fuer Byte.
//! - **Ein Zeichen ist kein Name.** Der Rufer legt ein Zeichen vor; der
//!   BESITZER kommt aus der Antwort des Speichers und aus nichts, was der
//!   Rufer geschrieben hat.
//!
//! The scrypt derivation lives further down (`ableiten`, `secret_matches`);
//! `secret_matches_with` takes the derivation as an argument, so every check
//! before it and every refusal can be measured without the cost of scrypt.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Das Einzige, was ein abgewiesener Rufer je hoert. Es sagt nichts darueber,
/// welche Haelfte falsch war.
pub const DENIED: &str = "not authorized";

/// Von wem die Anfrage ist, oder von niemandem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub owner: Option<String>,
    pub reason: String,
}

impl Caller {
    pub fn authorized(&self) -> bool { self.owner.is_some() }
    fn niemand() -> Self { Self { owner: None, reason: DENIED.to_string() } }
}

/// Was fuer ein Zeichen gespeichert wird. Ein Zeichen hat schon viel Entropie,
/// ein einfacher Abdruck genuegt also; ein langsamer Hash bei JEDER Anfrage
/// waere eine selbst zugefuegte Dienstverweigerung, also genau der umgekehrte
/// Handel zu dem, den ein Passwort braucht.
pub fn token_digest(token: &str) -> String {
    let d = Sha256::digest(token.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Die zerlegte gespeicherte Form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub n: u64,
    pub r: u32,
    pub p: u32,
    pub salt: Vec<u8>,
    pub key: Vec<u8>,
}

fn hex_zu_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None; }
    (0..s.len()).step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Zerlegt die gespeicherte Form, oder nichts.
///
/// Ein unbekanntes Verfahren ist eine Abweisung und KEIN Rueckfall auf ein
/// schwaecheres. Ein Speicher mit einer Form, die dieser Bau nicht lesen kann,
/// heisst, dass jemand zurueckgerollt hat, und die Antwort ist "nein" und
/// nicht "dann eben durchlassen".
pub fn parse_stored(stored: &str) -> Option<Stored> {
    let teile: Vec<&str> = stored.split('$').collect();
    if teile.len() != 6 || teile[0] != "scrypt" { return None; }
    Some(Stored {
        n: teile[1].parse().ok()?,
        r: teile[2].parse().ok()?,
        p: teile[3].parse().ok()?,
        salt: hex_zu_bytes(teile[4])?,
        key: hex_zu_bytes(teile[5])?,
    })
}

/// Ob `offered` das Geheimnis hinter `stored` ist, mit der Ableitung als
/// Argument.
///
/// Eine unlesbare gespeicherte Form ist `false` und nie ein Fehler, den ein
/// Behandler in eine andere Antwort verwandeln koennte als ein falsches
/// Geheimnis: die GESTALT des Scheiterns darf sich auch nicht unterscheiden.
pub fn secret_matches_with<F>(stored: &str, offered: &str, ableitung: F) -> bool
where
    F: Fn(&str, &[u8], u64, u32, u32, usize) -> Option<Vec<u8>>,
{
    if stored.is_empty() || offered.is_empty() { return false; }
    let Some(s) = parse_stored(stored) else { return false };
    let Some(berechnet) = ableitung(offered, &s.salt, s.n, s.r, s.p, s.key.len()) else {
        return false;
    };
    berechnet.ct_eq(&s.key).into()
}

/// Macht aus einer Kopfzeile einen Rufer.
///
/// `besitzer_von` nimmt den Abdruck eines Zeichens und antwortet mit dem
/// Besitzer. Der Besitzer kommt von DORT und von nirgends sonst.
pub fn caller_for<F>(header: Option<&str>, besitzer_von: F) -> Caller
where
    F: Fn(&str) -> Option<String>,
{
    let Some(h) = header else { return Caller::niemand() };
    if h.is_empty() { return Caller::niemand(); }
    let mut teile = h.splitn(2, ' ');
    let (Some(art), Some(rest)) = (teile.next(), teile.next()) else {
        return Caller::niemand();
    };
    if !art.eq_ignore_ascii_case("bearer") || rest.trim().is_empty() {
        return Caller::niemand();
    }
    match besitzer_von(&token_digest(rest.trim())) {
        Some(owner) if !owner.is_empty() => Caller { owner: Some(owner), reason: "authorized".into() },
        _ => Caller::niemand(),
    }
}

// --------------------------------------------------- Die Ableitung selbst

/// scrypt, mit Parametern, die gewaehlt und nicht geerbt sind: `n = 2^15` mit
/// `r = 8` kostet 32 MiB und etwa ein Zehntel einer Sekunde je Pruefung. Genug,
/// dass Raten keine Strategie ist, klein genug, dass ein Geraet, das alle paar
/// Minuten abgleicht, es nie merkt.
pub const SCRYPT_LOG_N: u8 = 15;
pub const SCRYPT_R: u32 = 8;
pub const SCRYPT_P: u32 = 1;
pub const SALT_BYTES: usize = 16;
pub const KEY_BYTES: usize = 32;

fn bytes_zu_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn ableiten(secret: &str, salt: &[u8], n: u64, r: u32, p: u32, laenge: usize)
    -> Option<Vec<u8>>
{
    // Das Rahmenwerk nimmt den Logarithmus, die gespeicherte Form nennt die
    // Zahl. Eine Zahl, die keine Zweierpotenz ist, kommt aus einer fremden
    // oder kaputten Form und ist ein Nein, kein Fehler.
    if n == 0 || !n.is_power_of_two() { return None; }
    let log_n = n.trailing_zeros() as u8;
    // Die Laenge kommt allein vom Ausgabepuffer: `new_with_output_len` haengt
    // am Merkmal `phc`, und die Doku der Crate sagt ausdruecklich, dass eine
    // dort gesetzte Laenge ohnehin ignoriert wird. Also drei Argumente und
    // ein Puffer der gewuenschten Groesse, ohne ein Merkmal dafuer zu holen.
    let params = scrypt::Params::new(log_n, r, p).ok()?;
    let mut aus = vec![0u8; laenge];
    scrypt::scrypt(secret.as_bytes(), salt, &params, &mut aus).ok()?;
    Some(aus)
}

/// Die gespeicherte Form eines Geheimnisses: Verfahren, Parameter, Salz und
/// Schluessel, damit die Parameter spaeter steigen koennen, ohne das
/// Gespeicherte ungueltig zu machen.
pub fn hash_secret(secret: &str, salt: &[u8]) -> Result<String, String> {
    if secret.is_empty() { return Err("an empty secret is not a secret".into()); }
    let n: u64 = 1 << SCRYPT_LOG_N;
    let key = ableiten(secret, salt, n, SCRYPT_R, SCRYPT_P, KEY_BYTES)
        .ok_or_else(|| "the derivation refused these parameters".to_string())?;
    Ok(format!("scrypt${n}${SCRYPT_R}${SCRYPT_P}${}${}", bytes_zu_hex(salt), bytes_zu_hex(&key)))
}

/// Ob `offered` das Geheimnis hinter `stored` ist, mit der echten Ableitung.
pub fn secret_matches(stored: &str, offered: &str) -> bool {
    secret_matches_with(stored, offered, ableiten)
}

/// Ein frisches Geraete-Zeichen.
///
/// **Keine neue Abhaengigkeit dafuer**: die Quelle ist `/dev/urandom`, also
/// genau das, was auch Pythons `secrets` darunter benutzt, und auf dem einzigen
/// Ziel dieses Baus (Linux) immer da. Eine Crate fuer 32 Byte Zufall waere
/// mehr, als die Frage wert ist.
///
/// **Fail-closed:** laesst die Quelle sich nicht lesen, gibt es einen Fehler
/// und NIE ein schwaecheres Zeichen. Ein Zeichen aus einer Ersatzquelle waere
/// genau der stille Fehler, den niemand bemerkt.
pub fn new_token() -> Result<String, String> {
    Ok(hex(&zufall(32)?))
}

/// `n` Byte aus `/dev/urandom`, oder ein Fehler. EINE Quelle fuer alles, was
/// hier zufaellig sein muss: das Zeichen, das Salz einer Ableitung und der
/// Name einer Sitzung. Drei Stellen mit drei Quellen waeren drei Gelegenheiten,
/// eine davon schwach zu machen.
pub fn zufall(n: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")
        .map_err(|e| format!("no source of randomness: {e}"))?;
    let mut roh = vec![0u8; n];
    f.read_exact(&mut roh).map_err(|e| format!("the source of randomness fell short: {e}"))?;
    Ok(roh)
}

/// Bytes als Hex. Oeffentlich, weil der Sitzungsname dieselbe Schreibweise
/// braucht wie Pythons `secrets.token_hex`.
pub fn hex(b: &[u8]) -> String { bytes_zu_hex(b) }

/// Hex zurueck in Bytes, oder nichts.
pub fn hex_lesen(s: &str) -> Option<Vec<u8>> { hex_zu_bytes(s) }

// ------------------------------------------------- Fassung 6: ein Geheimnis

/// Wie lang ein Beweis ist: `HKDF-SHA256(M, "maer sign in v1")`, 32 Byte.
pub const PROOF_BYTES: usize = 32;

/// Wie lang das Salz eines Kontos ist, das die Geraete dehnen.
pub const KONTO_SALZ_BYTES: usize = 16;

/// Die Dehnung auf dem Geraet, die Fassung 6 vorschreibt. Weniger nimmt der
/// Dienst nicht an; mehr darf eine spaetere Fassung verlangen.
pub const KDF: &str = "pbkdf2-hmac-sha256";
pub const MIN_ROUNDS: u64 = 600_000;

/// Die gespeicherte Form eines BEWEISES, wie `hash_secret` fuer ein Passwort.
///
/// Der Dienst dehnt den Beweis noch einmal mit scrypt: gestohlene Daten
/// kosten dann je geratenem Passwort beide Dehnungen, die des Geraets und
/// diese. Gerechnet wird ueber die 32 rohen Bytes, nicht ueber ihre
/// base64-Schreibweise.
pub fn hash_proof(proof: &[u8], salt: &[u8]) -> Result<String, String> {
    if proof.len() != PROOF_BYTES { return Err("a proof is 32 bytes".into()); }
    let n: u64 = 1 << SCRYPT_LOG_N;
    let key = ableiten_bytes(proof, salt, n, SCRYPT_R, SCRYPT_P, KEY_BYTES)
        .ok_or_else(|| "the derivation refused these parameters".to_string())?;
    Ok(format!("scrypt${n}${SCRYPT_R}${SCRYPT_P}${}${}", bytes_zu_hex(salt), bytes_zu_hex(&key)))
}

/// Ob `proof` der Beweis hinter `stored` ist. Dieselben Regeln wie fuer ein
/// Geheimnis: unlesbar ist `false`, nie ein Fehler anderer Gestalt.
pub fn proof_matches(stored: &str, proof: &[u8]) -> bool {
    if stored.is_empty() || proof.len() != PROOF_BYTES { return false; }
    let Some(s) = parse_stored(stored) else { return false };
    let Some(berechnet) = ableiten_bytes(proof, &s.salt, s.n, s.r, s.p, s.key.len()) else {
        return false;
    };
    berechnet.ct_eq(&s.key).into()
}

fn ableiten_bytes(eingabe: &[u8], salt: &[u8], n: u64, r: u32, p: u32, laenge: usize)
    -> Option<Vec<u8>>
{
    if n == 0 || !n.is_power_of_two() { return None; }
    let params = scrypt::Params::new(n.trailing_zeros() as u8, r, p).ok()?;
    let mut aus = vec![0u8; laenge];
    scrypt::scrypt(eingabe, salt, &params, &mut aus).ok()?;
    Some(aus)
}

/// Die Zeit einer Pruefung verbrauchen, OHNE die Eingabe anzufassen.
///
/// Fuer ein `secret`, das an ein umgezogenes Konto geht: der Vertrag sagt, es
/// wird weder gedehnt noch verglichen noch geloggt. Die Antwort kaeme dann
/// aber sofort, und eine sofortige Abweisung verriete an der Dauer, dass das
/// Konto umgezogen ist. Also wird dieselbe Arbeit an einem FESTEN Wert getan.
pub fn zeit_verbrauchen() {
    let n: u64 = 1 << SCRYPT_LOG_N;
    let _ = ableiten_bytes(b"maer sync: time spent on nothing", &[0u8; SALT_BYTES],
                           n, SCRYPT_R, SCRYPT_P, KEY_BYTES);
}

/// Das Schein-Salz fuer einen Namen, der kein umgezogenes Konto ist.
///
/// Aus dem Geheimnis des Dienstes und dem Namen, jedes Mal gleich: fragt
/// jemand zweimal nach demselben unbekannten Namen, bekommt er zweimal
/// dasselbe, und ein wechselndes Salz verriete sonst, dass es kein echtes
/// ist.
pub fn schein_salz(geheimnis: &[u8], owner: &str) -> Vec<u8> {
    let hk = hkdf::Hkdf::<Sha256>::new(None, geheimnis);
    let mut info = b"maer salt v1 ".to_vec();
    info.extend_from_slice(owner.as_bytes());
    let mut aus = vec![0u8; KONTO_SALZ_BYTES];
    // Die Laenge ist fest und weit unter der Grenze von HKDF (255 * 32).
    hk.expand(&info, &mut aus).expect("16 Byte liegen unter der Grenze von HKDF");
    aus
}

/// Das Alphabet des Einmal-Passworts: 22 Buchstaben ohne die, die man
/// verwechselt (I und L mit 1, O und Q mit 0). Dasselbe Mass wie der Code, den
/// das Einmal-Passwort ersetzt.
pub const EINMAL_ALPHABET: &[u8; 22] = b"ABCDEFGHJKMNPRSTUVWXYZ";
pub const EINMAL_LAENGE: usize = 24;

/// Ein Einmal-Passwort fuer den Betreiber, oder ein Fehler.
///
/// Gleichverteilt: ein Byte wird nur genommen, wenn es unter dem groessten
/// Vielfachen von 22 liegt, das in ein Byte passt. `byte % 22` allein wuerde
/// die ersten Buchstaben bevorzugen.
pub fn einmal_passwort() -> Result<String, String> {
    let grenze = (256 / EINMAL_ALPHABET.len() * EINMAL_ALPHABET.len()) as u8;
    let mut aus = String::with_capacity(EINMAL_LAENGE);
    while aus.len() < EINMAL_LAENGE {
        for b in zufall(64)? {
            if b < grenze && aus.len() < EINMAL_LAENGE {
                aus.push(EINMAL_ALPHABET[(b as usize) % EINMAL_ALPHABET.len()] as char);
            }
        }
    }
    Ok(aus)
}
