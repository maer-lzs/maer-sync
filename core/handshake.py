# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The encrypted session, so that `http` stops meaning `in the clear`.

The service accepts an `http` address on purpose: the people it is built for
are the ones who cannot get a certificate, and demanding one would lock out
exactly them. But without TLS every request carried its bearer token in the
clear, and whoever reads it owns the account, deletion included. That is the
measured reason this exists, not a general wish for more cryptography.

**No invention here.** This is Noise, pattern `NK`, in the spelling of the
framework's revision 34, and nothing else:

    Noise_NK_25519_ChaChaPoly_SHA256

`NK` and not `IK`: the responder's static key is known to the initiator
(pinned on the device), and the initiator stays anonymous in the handshake
because it already proves who it is INSIDE the session, with the token it has
had all along. Giving the device a static key as well would be a second
identity beside the token, and one identity is the point.

**The building blocks are borrowed, the pattern is written down, and the
result is held against SOMEBODY ELSE'S numbers:** the suite runs the two
published vectors for this exact protocol, from two independent
implementations that do not know each other. Passing your own round trip
proves that a construction agrees with itself; only a foreign vector proves
it is the protocol.

What this does NOT do, said here so nobody reads more into it:

- It does not authenticate the initiator. The token does that, one layer up.
- It does not replace `https`. Where TLS is there, this rides inside it and
  changes nothing about it.
- It says nothing about the media stream. That never goes through this
  service at all.
"""
import hashlib
import hmac

from cryptography.hazmat.primitives.asymmetric.x25519 import (X25519PrivateKey,
                                                              X25519PublicKey)
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

#: The one name, and it is exactly 32 bytes, which the framework's rules for
#: the initial hash depend on: a name of at most HASHLEN bytes is used padded
#: with zeros instead of hashed.
PROTOCOL_NAME = b"Noise_NK_25519_ChaChaPoly_SHA256"
HASHLEN = 32
KEYLEN = 32
TAGLEN = 16
#: The largest Noise message. The framework fixes it; a longer payload has to
#: be split by the caller, and the service's own body limit is far below it.
MAX_MESSAGE = 65535


def _hash(data):
    return hashlib.sha256(data).digest()


def _hmac(key, data):
    return hmac.new(key, data, hashlib.sha256).digest()


def hkdf(chaining_key, input_key_material, outputs):
    """The framework's HKDF, which is HMAC chained and nothing else."""
    temp_key = _hmac(chaining_key, input_key_material)
    out1 = _hmac(temp_key, b"\x01")
    out2 = _hmac(temp_key, out1 + b"\x02")
    if outputs == 2:
        return out1, out2
    out3 = _hmac(temp_key, out2 + b"\x03")
    return out1, out2, out3


def dh(private_key, public_key_bytes):
    """X25519, the only DH this protocol name allows."""
    return private_key.exchange(X25519PublicKey.from_public_bytes(public_key_bytes))


class CipherState:
    """One direction's key and its counter.

    The counter is the whole reason this is an object: a nonce reused with
    the same key breaks ChaCha20-Poly1305 completely, so it may never be
    supplied from outside, only counted here.
    """

    def __init__(self, key=None):
        self.key = key
        self.nonce = 0

    def has_key(self):
        return self.key is not None

    def _aead_nonce(self):
        # The framework's nonce for this cipher: four zero bytes, then the
        # counter as eight bytes, little endian. Writing it any other way
        # still encrypts and still interoperates with nobody.
        return b"\x00" * 4 + self.nonce.to_bytes(8, "little")

    def encrypt_with_ad(self, ad, plaintext):
        if not self.has_key():
            return plaintext
        out = ChaCha20Poly1305(self.key).encrypt(self._aead_nonce(), plaintext, ad)
        self.nonce += 1
        return out

    def decrypt_with_ad(self, ad, ciphertext):
        if not self.has_key():
            return ciphertext
        out = ChaCha20Poly1305(self.key).decrypt(self._aead_nonce(), ciphertext, ad)
        self.nonce += 1
        return out


class SymmetricState:
    """The chaining key and the running hash, as the framework names them."""

    def __init__(self, protocol_name=PROTOCOL_NAME):
        if len(protocol_name) <= HASHLEN:
            self.h = protocol_name + b"\x00" * (HASHLEN - len(protocol_name))
        else:
            self.h = _hash(protocol_name)
        self.chaining_key = self.h
        self.cipher = CipherState()

    def mix_key(self, input_key_material):
        self.chaining_key, temp_key = hkdf(self.chaining_key, input_key_material, 2)
        self.cipher = CipherState(temp_key)

    def mix_hash(self, data):
        self.h = _hash(self.h + data)

    def encrypt_and_hash(self, plaintext):
        ciphertext = self.cipher.encrypt_with_ad(self.h, plaintext)
        self.mix_hash(ciphertext)
        return ciphertext

    def decrypt_and_hash(self, ciphertext):
        plaintext = self.cipher.decrypt_with_ad(self.h, ciphertext)
        self.mix_hash(ciphertext)
        return plaintext

    def split(self):
        temp_k1, temp_k2 = hkdf(self.chaining_key, b"", 2)
        return CipherState(temp_k1), CipherState(temp_k2)


class HandshakeState:
    """One side of `NK`, from its first message to its two transport keys.

    Two messages and that is all: the initiator sends its ephemeral key and
    mixes in the DH with the responder's known static key, the responder
    answers with its own ephemeral and mixes in the DH of the two ephemerals.
    Whoever does not hold the responder's private static key cannot produce
    the second message, which is what pinning the key buys.
    """

    def __init__(self, *, initiator, prologue=b"", static_private=None,
                 remote_static=None, ephemeral_private=None):
        self.initiator = initiator
        self.symmetric = SymmetricState()
        self.symmetric.mix_hash(prologue)
        self.static_private = static_private
        self.remote_static = remote_static
        # Injectable ONLY so the published vectors can be replayed. Every
        # other caller leaves it alone and gets a fresh key; a handshake that
        # reuses an ephemeral is not a handshake.
        self.ephemeral_private = ephemeral_private
        self.remote_ephemeral = None
        # The pre-message of NK: the responder's static key is known to both
        # sides before anything is sent, so both mix it in.
        if initiator:
            self.symmetric.mix_hash(remote_static)
        else:
            self.symmetric.mix_hash(
                static_private.public_key().public_bytes_raw())

    @property
    def handshake_hash(self):
        return self.symmetric.h

    def write_message(self, payload=b""):
        """The initiator's first message, or the responder's answer."""
        if self.ephemeral_private is None:
            self.ephemeral_private = X25519PrivateKey.generate()
        e_public = self.ephemeral_private.public_key().public_bytes_raw()
        self.symmetric.mix_hash(e_public)
        if self.initiator:
            self.symmetric.mix_key(dh(self.ephemeral_private, self.remote_static))
        else:
            self.symmetric.mix_key(dh(self.ephemeral_private, self.remote_ephemeral))
        return e_public + self.symmetric.encrypt_and_hash(payload)

    def read_message(self, message):
        """The other side's message, and what it carried."""
        if len(message) < KEYLEN:
            raise ValueError("a handshake message carries an ephemeral key first")
        self.remote_ephemeral = message[:KEYLEN]
        self.symmetric.mix_hash(self.remote_ephemeral)
        if self.initiator:
            self.symmetric.mix_key(dh(self.ephemeral_private, self.remote_ephemeral))
        else:
            self.symmetric.mix_key(dh(self.static_private, self.remote_ephemeral))
        return self.symmetric.decrypt_and_hash(message[KEYLEN:])

    def split(self):
        """The two transport keys: the initiator sends with the first."""
        send, receive = self.symmetric.split()
        return (send, receive) if self.initiator else (receive, send)
