# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The two paths of the encrypted session, driven as a client would.

The store and the rules are tested next door. What is left here is the part
only the application can be wrong about: the shapes on the wire, and above
all that **neither path asks for a token**.

That last one is not a convenience. `NK` lets the caller stay anonymous
through the handshake, which is exactly what lets a SIGN IN be protected: a
sign in carries the account secret and happens before there is any token to
prove anything with. A witness that only checked the happy path would pass
just as well over a service that demanded a token here and thereby protected
everything except the one exchange that matters most.
"""

import base64
import os
import tempfile
import unittest

from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from core.handshake import HandshakeState
from service.app import App
from service.identity import public_of
from service.store import Store


def drive(app, method, path, body=b"", headers=()):
    """One request through the application, the way a server would."""
    import asyncio

    scope = {"type": "http", "method": method, "path": path,
             "query_string": b"", "headers": list(headers), "client": ("1.2.3.4", 9)}
    sent = []

    async def receive():
        return {"type": "http.request", "body": body, "more_body": False}

    async def send(message):
        sent.append(message)

    asyncio.run(app(scope, receive, send))
    start = next(m for m in sent if m["type"] == "http.response.start")
    payload = b"".join(m.get("body", b"") for m in sent if m["type"] == "http.response.body")
    return start["status"], dict(start["headers"]), payload


class TheTwoPathsNeedNoToken(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.store = Store(os.path.join(self.dir, "store.sqlite"))
        self.key = X25519PrivateKey.generate()
        self.app = App(self.store, identity=(self.key, "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"))

    def tearDown(self):
        self.store.close()

    def test_the_key_is_handed_out_without_one(self):
        import json
        status, _, body = drive(self.app, "GET", "/v1/key")
        self.assertEqual(status, 200)
        answer = json.loads(body)
        self.assertEqual(base64.b64decode(answer["key"]), public_of(self.key))
        self.assertEqual(answer["fingerprint"], "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF")

    def test_every_other_path_still_wants_one(self):
        # The counter direction, and without it the case above could pass over
        # a service that had simply stopped asking anybody for anything.
        status, _, _ = drive(self.app, "GET", "/v1/doc/stands")
        self.assertEqual(status, 401)

    def test_a_handshake_becomes_a_session_without_one(self):
        caller = HandshakeState(initiator=True, remote_static=public_of(self.key))
        first = caller.write_message(b"")
        status, headers, reply = drive(self.app, "POST", "/v1/session", first)
        self.assertEqual(status, 200)
        name = headers[b"x-maer-session"].decode("ascii")
        self.assertEqual(len(name), 32)
        # And the answer really is the other half of the handshake: the
        # caller can finish it and the two sides can speak.
        caller.read_message(reply)
        send, _ = caller.split()
        sealed = send.encrypt_with_ad(b"", b"a position")
        self.assertEqual(self.app.sessions.unseal(name, sealed), b"a position")


class TheWaysOfNotBeingAHandshake(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.store = Store(os.path.join(self.dir, "store.sqlite"))
        self.key = X25519PrivateKey.generate()
        self.app = App(self.store, identity=(self.key, "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"))

    def tearDown(self):
        self.store.close()

    def test_rubbish_is_one_answer_and_says_nothing_else(self):
        # Every way of not being a handshake gets the same answer. Telling
        # them apart would say how far somebody probing had got.
        status, _, body = drive(self.app, "POST", "/v1/session", b"not a handshake")
        self.assertEqual(status, 400)
        self.assertIn(b"not a handshake", body)
        self.assertEqual(len(self.app.sessions), 0, "a refusal must leave no session")

    def test_an_empty_body_is_refused_too(self):
        status, _, _ = drive(self.app, "POST", "/v1/session", b"")
        self.assertEqual(status, 400)

    def test_a_handshake_for_a_different_key_is_refused(self):
        stranger = X25519PrivateKey.generate()
        caller = HandshakeState(initiator=True, remote_static=public_of(stranger))
        status, _, _ = drive(self.app, "POST", "/v1/session", caller.write_message(b""))
        self.assertEqual(status, 400)
        self.assertEqual(len(self.app.sessions), 0)

    def test_the_wrong_method_is_not_a_handshake(self):
        self.assertEqual(drive(self.app, "GET", "/v1/session")[0], 405)
        self.assertEqual(drive(self.app, "POST", "/v1/key")[0], 405)


class AServiceWithoutAKeyAnswersHonestly(unittest.TestCase):
    """An application assembled without an identity is a real thing: every
    witness that only wants documents builds one. It must not answer a
    handshake as nobody, and it must not pretend the path does not exist."""

    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.store = Store(os.path.join(self.dir, "store.sqlite"))
        self.app = App(self.store)

    def tearDown(self):
        self.store.close()

    def test_the_key_path_says_there_is_no_key(self):
        status, _, body = drive(self.app, "GET", "/v1/key")
        self.assertEqual(status, 503)
        self.assertIn(b"no static key", body)

    def test_the_session_path_says_the_same(self):
        status, _, body = drive(self.app, "POST", "/v1/session", b"anything")
        self.assertEqual(status, 503)
        self.assertIn(b"no static key", body)


if __name__ == "__main__":
    unittest.main()
