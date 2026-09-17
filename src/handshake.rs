//! Wie die Sitzung entsteht: Noise, Muster `NK`, in der Schreibweise
//! `Noise_NK_25519_ChaChaPoly_SHA256`.
//!
//! **Nichts erfunden.** Das Muster steht im Rahmenwerk, und die zwei
//! veroeffentlichten Vektoren entscheiden, ob diese Umsetzung es trifft. `NK`,
//! weil das Geraet den statischen Schluessel des Dienstes vorher kennt und
//! selbst anonym bleibt; genau diese Anonymitaet schuetzt die ANMELDUNG, die
//! das Kontogeheimnis traegt und vor jedem Zeichen stattfindet.
//!
//! Die Ableitung ist HKDF-SHA256 mit leerem `info`, und das ist keine
//! Annaeherung: das Rahmenwerk kettet HMAC als `out1 = HMAC(tk, 0x01)`,
//! `out2 = HMAC(tk, out1 || 0x02)`, und genau das ist HKDF-Expand mit leerem
//! `info`, waehrend `temp_key = HMAC(ck, ikm)` HKDF-Extract mit `salt = ck` ist.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

pub const PROTOCOL_NAME: &[u8] = b"Noise_NK_25519_ChaChaPoly_SHA256";
pub const HASHLEN: usize = 32;
pub const KEYLEN: usize = 32;
pub const TAGLEN: usize = 16;
/// Die groesste Noise-Nachricht. Das Rahmenwerk legt sie fest.
pub const MAX_MESSAGE: usize = 65535;

#[derive(Debug, PartialEq, Eq)]
pub struct HandshakeError(pub String);

fn hash(data: &[u8]) -> [u8; HASHLEN] {
    Sha256::digest(data).into()
}

/// Die Ableitung des Rahmenwerks, zwei oder drei Ausgaben.
fn kdf(chaining_key: &[u8; HASHLEN], ikm: &[u8], outputs: usize) -> Vec<[u8; HASHLEN]> {
    let hk = Hkdf::<Sha256>::new(Some(chaining_key), ikm);
    let mut okm = vec![0u8; HASHLEN * outputs];
    hk.expand(&[], &mut okm).expect("HASHLEN mal drei ist erlaubt");
    okm.chunks(HASHLEN)
        .map(|c| {
            let mut a = [0u8; HASHLEN];
            a.copy_from_slice(c);
            a
        })
        .collect()
}

fn dh(private: &[u8; 32], public: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*private);
    secret.diffie_hellman(&PublicKey::from(*public)).to_bytes()
}

/// Ein Schluessel einer Richtung und sein Zaehler.
///
/// Der Zaehler ist der Grund, warum das ein eigenes Ding ist: ein zweimal
/// benutzter Zaehler mit demselben Schluessel bricht ChaCha20-Poly1305
/// vollstaendig. Er darf deshalb nie von aussen kommen, nur hier gezaehlt
/// werden.
pub struct CipherState {
    key: Option<[u8; KEYLEN]>,
    nonce: u64,
}

impl CipherState {
    pub fn new(key: Option<[u8; KEYLEN]>) -> Self {
        Self { key, nonce: 0 }
    }
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }
    /// Der Zaehler dieser Chiffre: vier Nullbyte, dann acht Byte little
    /// endian. Jede andere Schreibweise verschluesselt auch und passt zu
    /// niemandem.
    fn aead_nonce(&self) -> Nonce {
        let mut raw = [0u8; 12];
        raw[4..].copy_from_slice(&self.nonce.to_le_bytes());
        Nonce::from(raw)
    }
    pub fn encrypt_with_ad(&mut self, ad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let Some(key) = self.key else {
            return Ok(plaintext.to_vec());
        };
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let out = cipher
            .encrypt(&self.aead_nonce(), Payload { msg: plaintext, aad: ad })
            .map_err(|_| HandshakeError("encryption failed".into()))?;
        self.nonce += 1;
        Ok(out)
    }
    pub fn decrypt_with_ad(&mut self, ad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let Some(key) = self.key else {
            return Ok(ciphertext.to_vec());
        };
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let out = cipher
            .decrypt(&self.aead_nonce(), Payload { msg: ciphertext, aad: ad })
            .map_err(|_| HandshakeError("the sealed message does not open".into()))?;
        self.nonce += 1;
        Ok(out)
    }
}

/// Kettenschluessel und laufender Hash, wie das Rahmenwerk sie nennt.
pub struct SymmetricState {
    pub h: [u8; HASHLEN],
    chaining_key: [u8; HASHLEN],
    cipher: CipherState,
}

impl SymmetricState {
    pub fn new(protocol_name: &[u8]) -> Self {
        // Ein Name von hoechstens HASHLEN Byte wird mit Nullen aufgefuellt,
        // NICHT gehasht. Der eine Name hier ist genau 32 Byte lang, und die
        // Regel des Rahmenwerks haengt daran.
        let mut h = [0u8; HASHLEN];
        if protocol_name.len() <= HASHLEN {
            h[..protocol_name.len()].copy_from_slice(protocol_name);
        } else {
            h = hash(protocol_name);
        }
        Self { h, chaining_key: h, cipher: CipherState::new(None) }
    }
    pub fn mix_key(&mut self, ikm: &[u8]) {
        let out = kdf(&self.chaining_key, ikm, 2);
        self.chaining_key = out[0];
        self.cipher = CipherState::new(Some(out[1]));
    }
    pub fn mix_hash(&mut self, data: &[u8]) {
        let mut buf = Vec::with_capacity(HASHLEN + data.len());
        buf.extend_from_slice(&self.h);
        buf.extend_from_slice(data);
        self.h = hash(&buf);
    }
    pub fn encrypt_and_hash(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let h = self.h;
        let ciphertext = self.cipher.encrypt_with_ad(&h, plaintext)?;
        self.mix_hash(&ciphertext);
        Ok(ciphertext)
    }
    pub fn decrypt_and_hash(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let h = self.h;
        let plaintext = self.cipher.decrypt_with_ad(&h, ciphertext)?;
        self.mix_hash(ciphertext);
        Ok(plaintext)
    }
    pub fn split(&self) -> (CipherState, CipherState) {
        let out = kdf(&self.chaining_key, b"", 2);
        (CipherState::new(Some(out[0])), CipherState::new(Some(out[1])))
    }
}

/// Eine Seite von `NK`, von ihrer ersten Nachricht bis zu den zwei
/// Sitzungsschluesseln.
pub struct HandshakeState {
    initiator: bool,
    pub symmetric: SymmetricState,
    static_private: Option<[u8; 32]>,
    remote_static: Option<[u8; 32]>,
    ephemeral_private: Option<[u8; 32]>,
    remote_ephemeral: Option<[u8; 32]>,
}

impl HandshakeState {
    /// `ephemeral_private` ist NUR einspeisbar, damit die veroeffentlichten
    /// Vektoren nachgefahren werden koennen. Jeder andere Rufer laesst es
    /// weg; ein Handschlag, der einen fluechtigen Schluessel wiederverwendet,
    /// ist keiner.
    pub fn new(
        initiator: bool,
        prologue: &[u8],
        static_private: Option<[u8; 32]>,
        remote_static: Option<[u8; 32]>,
        ephemeral_private: Option<[u8; 32]>,
    ) -> Result<Self, HandshakeError> {
        let mut symmetric = SymmetricState::new(PROTOCOL_NAME);
        symmetric.mix_hash(prologue);
        // Die Vorab-Nachricht von NK: der statische Schluessel des
        // Antwortenden ist beiden vorher bekannt, also mischen ihn beide ein.
        if initiator {
            let rs = remote_static.ok_or_else(|| HandshakeError("NK needs the responder's static key".into()))?;
            symmetric.mix_hash(&rs);
        } else {
            let sp = static_private.ok_or_else(|| HandshakeError("the responder needs its static key".into()))?;
            let pubk = PublicKey::from(&StaticSecret::from(sp));
            symmetric.mix_hash(pubk.as_bytes());
        }
        Ok(Self { initiator, symmetric, static_private, remote_static, ephemeral_private, remote_ephemeral: None })
    }

    pub fn handshake_hash(&self) -> [u8; HASHLEN] {
        self.symmetric.h
    }

    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let eph = self
            .ephemeral_private
            .ok_or_else(|| HandshakeError("no ephemeral key was provided".into()))?;
        let e_public = PublicKey::from(&StaticSecret::from(eph));
        self.symmetric.mix_hash(e_public.as_bytes());
        let peer = if self.initiator {
            self.remote_static.ok_or_else(|| HandshakeError("the responder's static key is missing".into()))?
        } else {
            self.remote_ephemeral.ok_or_else(|| HandshakeError("the initiator's ephemeral key is missing".into()))?
        };
        self.symmetric.mix_key(&dh(&eph, &peer));
        let mut out = e_public.as_bytes().to_vec();
        out.extend_from_slice(&self.symmetric.encrypt_and_hash(payload)?);
        Ok(out)
    }

    pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        if message.len() < KEYLEN {
            return Err(HandshakeError("the message is shorter than a key".into()));
        }
        let mut re = [0u8; 32];
        re.copy_from_slice(&message[..KEYLEN]);
        self.remote_ephemeral = Some(re);
        self.symmetric.mix_hash(&re);
        // Der Anrufer mischt `ee` ein, der Antwortende `es`. Ohne den zweiten
        // Fall bleibt seine Chiffre OHNE Schluessel, und dann gibt
        // `decrypt_with_ad` den Text unveraendert zurueck: der Handschlag
        // sieht aus als ginge er auf, und ein gekipptes Byte faellt nicht auf.
        // Genau so ist dieser Fehler hier aufgefallen.
        if self.initiator {
            let eph = self
                .ephemeral_private
                .ok_or_else(|| HandshakeError("no ephemeral key was provided".into()))?;
            self.symmetric.mix_key(&dh(&eph, &re));
        } else {
            let sp = self
                .static_private
                .ok_or_else(|| HandshakeError("the responder needs its static key".into()))?;
            self.symmetric.mix_key(&dh(&sp, &re));
        }
        self.symmetric.decrypt_and_hash(&message[KEYLEN..])
    }

    /// Die zwei Sitzungsschluessel: der ANRUFER sendet mit dem ersten.
    ///
    /// Beim Antwortenden sind sie deshalb vertauscht. Ohne den Tausch
    /// verschluesseln beide Seiten mit demselben Schluessel und entschluesseln
    /// mit dem anderen, und die Sitzungsnachrichten der Vektoren passen nicht.
    pub fn split(&self) -> (CipherState, CipherState) {
        let (send, receive) = self.symmetric.split();
        if self.initiator { (send, receive) } else { (receive, send) }
    }
}
