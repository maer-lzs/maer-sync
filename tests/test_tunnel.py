# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Requests INSIDE a session, driven as a device would drive them.

The device opens a handshake, then sends every ordinary request of this
service sealed, as a frame, to the session path with the session's name in
a header, and unseals the reply. What is measured here is the part only the
application can get wrong: that the inside is the ordinary wire (a sign in,
a read, a write, a listing all work in there and nowhere is anything
learned twice), that an ended session is one number and a broken message
another, and that the plain ways can be turned off without touching the
session's own two paths.
"""

import os
import tempfile
import unittest

from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from core.frames import pack, reply_of, unpack
from core.handshake import HandshakeState
from core.sessions import Limits
from service.app import App
from service.identity import public_of
from service.sessions import SessionStore
from service.store import Store
from tests.test_session_paths import drive

import json

SECRET = "a passphrase with some length"


class Device:
    """One device: a handshake, then sealed requests in that session."""

    def __init__(self, app, key):
        caller = HandshakeState(initiator=True, remote_static=public_of(key))
        status, headers, reply = drive(app, "POST", "/v1/session", caller.write_message(b""))
        assert status == 200, status
        self.name = headers[b"x-maer-session"].decode("ascii")
        caller.read_message(reply)
        self.send, self.receive = caller.split()
        self.app = app

    def request(self, method, target, headers=None, body=b""):
        """`(inner status, inner headers, inner body)`, or the OUTER answer
        as `("outer", status, body)` when the service did not open the
        message at all."""
        frame = pack({"method": method, "target": target, "headers": headers or {}}, body)
        status, _, reply = drive(self.app, "POST", "/v1/session", self.send.encrypt_with_ad(b"", frame),
                                 [(b"x-maer-session", self.name.encode("ascii"))])
        if status != 200:
            return "outer", status, reply
        head, inner_body = unpack(self.receive.decrypt_with_ad(b"", reply))
        inner_status, inner_headers = reply_of(head)
        return inner_status, inner_headers, inner_body


class TunnelCase(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.store = Store(os.path.join(self.dir, "store.sqlite"))
        self.store.create_account("the-owner", SECRET)
        self.key = X25519PrivateKey.generate()
        self.clock = [1_000_000.0]
        self.sessions = SessionStore(limits=Limits(), now=lambda: self.clock[0])
        self.app = App(self.store, identity=(self.key, "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"), sessions=self.sessions)

    def tearDown(self):
        self.store.close()

    def signed_in(self):
        device = Device(self.app, self.key)
        status, _, body = device.request("POST", "/v1/token",
                                         {"Content-Type": "application/json"},
                                         json.dumps({"owner": "the-owner", "secret": SECRET, "device": "d"}).encode())
        self.assertEqual(status, 200)
        token = json.loads(body)["token"]
        return device, {"Authorization": f"Bearer {token}"}


class TheInsideIsTheOrdinaryWire(TunnelCase):
    def test_a_sign_in_a_write_a_read_and_a_listing_all_work_inside(self):
        device, auth = self.signed_in()
        self.assertEqual(device.request("GET", "/v1/doc/stands?scope=", auth)[0], 404, "not there yet")
        status, headers, _ = device.request("PUT", "/v1/doc/stands?scope=",
                                            dict(auth, **{"X-Maer-Format": "1"}), b'{"k":"v"}')
        self.assertEqual(status, 200)
        self.assertEqual(headers["etag"], "1", "the version comes back inside, as it does outside")
        status, headers, body = device.request("GET", "/v1/doc/stands?scope=", auth)
        self.assertEqual((status, body, headers["x-maer-format"]), (200, b'{"k":"v"}', "1"))
        status, _, body = device.request("GET", "/v1/docs/stands", auth)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)["documents"][0]["version"], "1")

    def test_the_gates_inside_are_the_gates_outside(self):
        # The same write rules: a version that moved is a 412 inside too,
        # and nothing about them was written twice.
        device, auth = self.signed_in()
        put = dict(auth, **{"X-Maer-Format": "1"})
        device.request("PUT", "/v1/doc/stands?scope=", put, b'{"k":"v"}')
        status, headers, _ = device.request("PUT", "/v1/doc/stands?scope=",
                                            dict(put, **{"If-Match": "0"}), b'{"k":"w"}')
        self.assertEqual((status, headers["etag"]), (412, "1"))
        self.assertEqual(device.request("GET", "/v1/doc/stands?scope=")[0], 401, "no token inside is no token")

    def test_a_token_from_inside_works_outside_and_the_other_way_round(self):
        # One store, one kind of token: the session is a wrapping, not a
        # second account.
        device, auth = self.signed_in()
        status, _, _ = drive(self.app, "GET", "/v1/doc/stands",
                             headers=[(b"authorization", auth["Authorization"].encode())])
        self.assertEqual(status, 404)

    def test_the_session_is_proven_by_a_sign_in_and_outlives_the_short_idle(self):
        device, _ = self.signed_in()
        self.clock[0] += 61
        self.assertTrue(self.sessions.is_live(device.name), "a proven session is not swept like a bare handshake")
        bare = Device(self.app, self.key)
        self.clock[0] += 61
        self.assertFalse(self.sessions.is_live(bare.name), "an unproven one is")


class WhatIsNotARequestInside(TunnelCase):
    def test_an_ended_session_is_410_and_a_new_handshake_mends_it(self):
        device, auth = self.signed_in()
        self.clock[0] += 3601
        self.assertEqual(device.request("GET", "/v1/doc/stands?scope=", auth)[:2], ("outer", 410))
        again = Device(self.app, self.key)
        self.assertEqual(again.request("GET", "/v1/doc/stands?scope=", auth)[0], 404)

    def test_a_made_up_name_is_410_too(self):
        device, auth = self.signed_in()
        device.name = "00" * 16
        self.assertEqual(device.request("GET", "/v1/doc/stands?scope=", auth)[:2], ("outer", 410))

    def test_noise_under_a_real_name_is_400_and_the_session_survives_it(self):
        device, auth = self.signed_in()
        status, _, body = drive(self.app, "POST", "/v1/session", b"not sealed at all",
                                [(b"x-maer-session", device.name.encode("ascii"))])
        self.assertEqual(status, 400)
        self.assertIn(b"not a sealed message", body)
        # The counter did not move on the failure, so the next real message
        # still opens.
        self.assertEqual(device.request("GET", "/v1/doc/stands?scope=", auth)[0], 404)

    def test_a_sealed_thing_that_is_not_a_request_is_400(self):
        device, _ = self.signed_in()
        sealed = device.send.encrypt_with_ad(b"", b"\x00\x02{}")
        status, _, body = drive(self.app, "POST", "/v1/session", sealed,
                                [(b"x-maer-session", device.name.encode("ascii"))])
        self.assertEqual(status, 400)
        self.assertIn(b"not a request", body)

    def test_the_session_does_not_nest_and_the_key_is_not_asked_inside(self):
        device, _ = self.signed_in()
        self.assertEqual(device.request("POST", "/v1/session")[:2], ("outer", 400))
        self.assertEqual(device.request("GET", "/v1/key")[:2], ("outer", 400))


class ThePlainWaysCanBeTurnedOff(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.store = Store(os.path.join(self.dir, "store.sqlite"))
        self.store.create_account("the-owner", SECRET)
        self.token = self.store.issue_token("the-owner", SECRET)
        self.key = X25519PrivateKey.generate()
        self.app = App(self.store, identity=(self.key, "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"), plain_ways=False)

    def tearDown(self):
        self.store.close()

    def test_a_plain_request_is_told_to_come_inside(self):
        auth = [(b"authorization", f"Bearer {self.token}".encode())]
        for method, path in (("GET", "/v1/doc/stands"), ("GET", "/v1/docs/stands"), ("POST", "/v1/token")):
            status, _, body = drive(self.app, method, path, headers=auth)
            self.assertEqual(status, 426, path)
            self.assertIn(b"inside a session", body)

    def test_the_session_paths_and_the_health_route_still_answer(self):
        self.assertEqual(drive(self.app, "GET", "/v1/health")[0], 200)
        self.assertEqual(drive(self.app, "GET", "/v1/key")[0], 200)
        device = Device(self.app, self.key)
        status, _, body = device.request("POST", "/v1/token", {"Content-Type": "application/json"},
                                         json.dumps({"owner": "the-owner", "secret": SECRET}).encode())
        self.assertEqual(status, 200)
        token = json.loads(body)["token"]
        self.assertEqual(device.request("GET", "/v1/doc/stands?scope=", {"Authorization": f"Bearer {token}"})[0], 404)

    def test_the_switch_reads_the_environment(self):
        for value, expected in (("on", True), ("off", False), ("0", False), ("yes", True)):
            os.environ["MAER_SYNC_PLAIN_WAYS"] = value
            try:
                self.assertEqual(App(self.store).plain_ways, expected, value)
            finally:
                del os.environ["MAER_SYNC_PLAIN_WAYS"]
        self.assertTrue(App(self.store).plain_ways, "on by default: devices in the clear keep syncing until they are updated")


class TheSocketInsideASession(TunnelCase):
    def socket(self, device, first, then=()):
        seen = []
        incoming = [{"type": "websocket.connect"}] + list(first) + list(then)

        async def receive():
            return incoming.pop(0) if incoming else {"type": "websocket.disconnect"}

        async def send(message):
            seen.append(message)

        import asyncio
        asyncio.run(self.app({"type": "websocket", "path": "/v1/changes",
                              "headers": [(b"x-maer-session", device.name.encode("ascii"))],
                              "client": ("1.2.3.4", 9)}, receive, send))
        return seen

    def sealed_first(self, device, headers):
        return [{"type": "websocket.receive",
                 "bytes": device.send.encrypt_with_ad(b"", pack({"method": "GET", "target": "/v1/changes",
                                                                  "headers": headers}))}]

    def test_the_first_frame_carries_the_token_and_announcements_come_back_sealed(self):
        device, auth = self.signed_in()
        # The socket speaks in ITS OWN session: a stream and the replies of
        # a request session would race each other's counters.
        line = Device(self.app, self.key)
        seen = []

        # Drive the socket by hand so an announcement can be sent while it
        # is open: connect, the sealed first frame, then a write by another
        # path, then the disconnect.
        incoming = [{"type": "websocket.connect"}] + self.sealed_first(line, auth)
        announced = [False]

        async def receive():
            if incoming:
                return incoming.pop(0)
            if not announced[0]:
                announced[0] = True
                await self.app.listeners.announce("the-owner", {"kind": "stands", "scope": "", "version": "9"})
                return {"type": "websocket.receive", "text": "ignored"}
            return {"type": "websocket.disconnect"}

        async def send(message):
            seen.append(message)

        import asyncio
        asyncio.run(self.app({"type": "websocket", "path": "/v1/changes",
                              "headers": [(b"x-maer-session", line.name.encode("ascii"))],
                              "client": ("1.2.3.4", 9)}, receive, send))
        self.assertEqual(seen[0]["type"], "websocket.accept")
        frames = [m for m in seen if m["type"] == "websocket.send"]
        self.assertEqual(len(frames), 1)
        self.assertIsNone(frames[0].get("text"), "nothing leaves this socket in the clear")
        opened = line.receive.decrypt_with_ad(b"", frames[0]["bytes"])
        self.assertEqual(json.loads(opened), {"kind": "stands", "scope": "", "version": "9"})
        self.assertEqual(self.app.listeners.count("the-owner"), 0, "gone after the disconnect")

    def test_a_first_frame_without_a_token_closes_after_the_accept(self):
        line = Device(self.app, self.key)
        seen = self.socket(line, self.sealed_first(line, {}))
        self.assertEqual([m["type"] for m in seen], ["websocket.accept", "websocket.close"])
        self.assertEqual(seen[1]["code"], 4401)
        self.assertEqual(self.app.listeners.count("the-owner"), 0)

    def test_a_made_up_name_is_turned_away_before_the_accept(self):
        line = Device(self.app, self.key)
        line.name = "00" * 16
        seen = self.socket(line, [])
        self.assertEqual(seen[0]["type"], "websocket.close")
        self.assertEqual(seen[0]["code"], 4410)

    def test_a_first_frame_that_is_not_sealed_closes(self):
        line = Device(self.app, self.key)
        seen = self.socket(line, [{"type": "websocket.receive", "text": "hello"}])
        self.assertEqual(seen[-1]["code"], 4400)


if __name__ == "__main__":
    unittest.main()
