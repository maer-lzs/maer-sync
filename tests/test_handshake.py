# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The handshake against SOMEBODY ELSE'S numbers, and then against itself.

The order matters. A round trip proves that a construction agrees with
itself, and a construction that is subtly not Noise agrees with itself
perfectly. So the first two cases replay published vectors byte for byte,
from two implementations in different languages that do not know each other
and that used different keys. Only after that do the behaviour cases run.

The vectors are kept in `tests/vectors/`, not fetched: a suite that reaches
the network measures the network.
"""
import json
import pathlib
import unittest

from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from core.handshake import HandshakeState

ROOT = pathlib.Path(__file__).resolve().parent
VECTORS = json.loads((ROOT / "vectors" / "noise-nk.json").read_text(encoding="utf-8"))


def _key(hex_string):
    return X25519PrivateKey.from_private_bytes(bytes.fromhex(hex_string))


class VectorTests(unittest.TestCase):
    def test_both_published_vectors_are_reproduced_byte_for_byte(self):
        vectors = VECTORS["vectors"]
        # The precondition, and without it this file could pass while
        # measuring nothing: there are two, and they are not the same one
        # twice.
        self.assertEqual(len(vectors), 2)
        self.assertNotEqual(vectors[0]["init_ephemeral"], vectors[1]["init_ephemeral"],
                            "two vectors with the same keys would be one vector")

        for vector in vectors:
            with self.subTest(protocol=vector["protocol_name"]):
                self.assertEqual(vector["protocol_name"], "Noise_NK_25519_ChaChaPoly_SHA256")
                self._replay(vector)

    def _replay(self, vector):
        raw = bytes.fromhex
        initiator = HandshakeState(
            initiator=True,
            prologue=raw(vector["init_prologue"]),
            remote_static=raw(vector["init_remote_static"]),
            ephemeral_private=_key(vector["init_ephemeral"]))
        responder = HandshakeState(
            initiator=False,
            prologue=raw(vector["resp_prologue"]),
            static_private=_key(vector["resp_static"]),
            ephemeral_private=_key(vector["resp_ephemeral"]))

        messages = vector["messages"]
        first = initiator.write_message(raw(messages[0]["payload"]))
        self.assertEqual(first.hex(), messages[0]["ciphertext"],
                         "the initiator's message is not the published one")
        self.assertEqual(responder.read_message(first).hex(), messages[0]["payload"])

        second = responder.write_message(raw(messages[1]["payload"]))
        self.assertEqual(second.hex(), messages[1]["ciphertext"],
                         "the responder's answer is not the published one")
        self.assertEqual(initiator.read_message(second).hex(), messages[1]["payload"])

        # The handshake hash is what a later channel binding would stand on,
        # so it is held even though nothing uses it yet.
        if "handshake_hash" in vector:
            self.assertEqual(initiator.handshake_hash.hex(), vector["handshake_hash"])
            self.assertEqual(responder.handshake_hash.hex(), vector["handshake_hash"])

        i_send, i_receive = initiator.split()
        r_send, r_receive = responder.split()
        for index, message in enumerate(messages[2:], start=2):
            writer, reader = (i_send, r_receive) if index % 2 == 0 else (r_send, i_receive)
            sealed = writer.encrypt_with_ad(b"", raw(message["payload"]))
            self.assertEqual(sealed.hex(), message["ciphertext"],
                             f"transport message {index} is not the published one")
            self.assertEqual(reader.decrypt_with_ad(b"", raw(message["ciphertext"])).hex(),
                             message["payload"])


class BehaviourTests(unittest.TestCase):
    def _pair(self, responder_static=None, initiator_believes=None):
        responder_static = responder_static or X25519PrivateKey.generate()
        believed = initiator_believes or responder_static.public_key().public_bytes_raw()
        initiator = HandshakeState(initiator=True, remote_static=believed)
        responder = HandshakeState(initiator=False, static_private=responder_static)
        return initiator, responder

    def test_a_fresh_pair_agrees_on_both_directions(self):
        initiator, responder = self._pair()
        responder.read_message(initiator.write_message(b"hello"))
        initiator.read_message(responder.write_message(b"there"))
        i_send, i_receive = initiator.split()
        r_send, r_receive = responder.split()
        sealed = i_send.encrypt_with_ad(b"", b"a position")
        self.assertEqual(r_receive.decrypt_with_ad(b"", sealed), b"a position")
        back = r_send.encrypt_with_ad(b"", b"an answer")
        self.assertEqual(i_receive.decrypt_with_ad(b"", back), b"an answer")

    def test_two_handshakes_never_look_alike(self):
        # Without a fresh ephemeral per handshake the whole thing collapses,
        # and nothing in a round trip would notice.
        first, _ = self._pair()
        second, _ = self._pair()
        self.assertNotEqual(first.write_message(b"x"), second.write_message(b"x"))

    def test_the_wrong_static_key_gets_no_session(self):
        # Exactly the swapped counterpart: somebody answers, but not the one
        # whose key is pinned. It must fail, not quietly work.
        real = X25519PrivateKey.generate()
        stranger = X25519PrivateKey.generate()
        initiator, _ = self._pair(responder_static=real,
                                  initiator_believes=stranger.public_key().public_bytes_raw())
        responder = HandshakeState(initiator=False, static_private=real)
        with self.assertRaises(Exception):
            responder.read_message(initiator.write_message(b"hello"))

    def test_a_changed_byte_is_refused(self):
        initiator, responder = self._pair()
        message = bytearray(initiator.write_message(b"hello"))
        message[-1] ^= 0x01
        with self.assertRaises(Exception):
            responder.read_message(bytes(message))

    def test_a_different_prologue_gets_no_session(self):
        # The prologue is where the two sides state what they agreed on
        # before speaking. If they disagree, the session must not come up.
        responder_static = X25519PrivateKey.generate()
        public = responder_static.public_key().public_bytes_raw()
        initiator = HandshakeState(initiator=True, prologue=b"v1", remote_static=public)
        responder = HandshakeState(initiator=False, prologue=b"v2",
                                   static_private=responder_static)
        with self.assertRaises(Exception):
            responder.read_message(initiator.write_message(b"hello"))

    def test_the_counter_moves_so_a_nonce_is_never_reused(self):
        initiator, responder = self._pair()
        responder.read_message(initiator.write_message(b""))
        initiator.read_message(responder.write_message(b""))
        i_send, _ = initiator.split()
        _, r_receive = responder.split()
        first = i_send.encrypt_with_ad(b"", b"same")
        second = i_send.encrypt_with_ad(b"", b"same")
        self.assertNotEqual(first, second, "the same plaintext twice must not seal the same")
        self.assertEqual(r_receive.decrypt_with_ad(b"", first), b"same")
        self.assertEqual(r_receive.decrypt_with_ad(b"", second), b"same")


if __name__ == "__main__":
    unittest.main()
