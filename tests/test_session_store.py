# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""That the store holds what the handshake made, and lets go when it should.

The rules are tested next door, purely. What is left here is the part a pure
test cannot reach: that a real handshake goes through it, that a message
sealed on one side opens on the other, and that the ways of NOT being a
session all come back as the same refusal.

Time is injected rather than slept, which is the only way to measure an hour
of silence in a suite that has to stay fast.
"""

import unittest

from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from core.handshake import HandshakeState
from core.sessions import Limits
from service.identity import public_of
from service.sessions import SessionStore, UnknownSession


class Clock:
    """A clock a witness can move."""

    def __init__(self, at=1_000_000.0):
        self.at = at

    def __call__(self):
        return self.at

    def forward(self, seconds):
        self.at += seconds


def a_caller(service_public, prologue=b""):
    """The device's side of the handshake, up to its first message."""
    state = HandshakeState(initiator=True, prologue=prologue, remote_static=service_public)
    return state, state.write_message(b"")


class AHandshakeBecomesASession(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.service_key = X25519PrivateKey.generate()
        self.store = SessionStore(now=self.clock)

    def _opened(self, prologue=b""):
        caller, first = a_caller(public_of(self.service_key), prologue)
        name, reply, _ = self.store.open(first, self.service_key, prologue)
        caller.read_message(reply)
        send, receive = caller.split()
        return name, send, receive

    def test_a_message_sealed_by_the_device_opens_here(self):
        name, send, _ = self._opened()
        sealed = send.encrypt_with_ad(b"", b"a position")
        self.assertEqual(self.store.unseal(name, sealed), b"a position")

    def test_an_answer_sealed_here_opens_at_the_device(self):
        name, _, receive = self._opened()
        sealed = self.store.seal(name, b"an answer")
        self.assertEqual(receive.decrypt_with_ad(b"", sealed), b"an answer")

    def test_two_sessions_do_not_share_keys(self):
        # The one thing a single round trip cannot show: that the store keeps
        # them apart. A message from one must not open under the other.
        first, send_one, _ = self._opened()
        second, _, _ = self._opened()
        sealed = send_one.encrypt_with_ad(b"", b"mine")
        self.assertEqual(self.store.unseal(first, sealed), b"mine")
        with self.assertRaises(Exception):
            self.store.unseal(second, send_one.encrypt_with_ad(b"", b"mine"))

    def test_a_different_prologue_never_becomes_a_session(self):
        caller, first = a_caller(public_of(self.service_key), b"v1")
        with self.assertRaises(Exception):
            self.store.open(first, self.service_key, b"v2")
        self.assertEqual(len(self.store), 0,
                         "a handshake that does not add up must leave no trace")

    def test_a_failed_handshake_costs_the_store_nothing(self):
        # What keeps a flood of BROKEN handshakes free: nothing is kept until
        # the exchange has actually completed.
        for _ in range(20):
            with self.assertRaises(Exception):
                self.store.open(b"not a handshake at all", self.service_key)
        self.assertEqual(len(self.store), 0)


class TheWaysOfNotBeingASession(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.service_key = X25519PrivateKey.generate()
        self.store = SessionStore(now=self.clock)
        caller, first = a_caller(public_of(self.service_key))
        self.name, reply, _ = self.store.open(first, self.service_key)
        caller.read_message(reply)
        self.send, self.receive = caller.split()

    def test_a_name_nobody_made_is_unknown(self):
        with self.assertRaises(UnknownSession):
            self.store.unseal("00" * 16, b"anything")

    def test_a_session_that_went_quiet_is_unknown(self):
        self.clock.forward(2 * 3600)
        with self.assertRaises(UnknownSession):
            self.store.seal(self.name, b"too late")

    def test_an_ended_session_is_dropped_on_the_spot(self):
        # Not left for a later sweep: it can never be used again, and holding
        # it would let a stranger keep a name in the store by asking about it.
        self.clock.forward(2 * 3600)
        with self.assertRaises(UnknownSession):
            self.store.seal(self.name, b"too late")
        self.assertEqual(len(self.store), 0)

    def test_noise_under_a_known_name_does_not_keep_it_alive(self):
        """The moment of use is recorded on SUCCESS only.

        Without that, anybody who learns a name can hold somebody else's
        session open forever by sending rubbish at it, which is the opposite
        of what the idle limit is for.
        """
        # PROVEN first, and that is not decoration. The first version of
        # this witness moved three thousand seconds on an UNPROVEN session,
        # which is over its minute long before the noise arrives: the store
        # refused at the door and the recording line was never reached, so
        # the case passed with the recording moved in FRONT of the decrypt.
        # The red probe said so, the reading did not.
        self.store.prove(self.name)
        self.clock.forward(3000)
        with self.assertRaises(Exception):
            self.store.unseal(self.name, b"rubbish")
        # Past the hour, counting from the last REAL use and not from the noise.
        self.clock.forward(700)
        with self.assertRaises(UnknownSession):
            self.store.seal(self.name, b"should be gone")

    def test_a_real_message_does_keep_it_alive(self):
        """The other direction, or the case above would also pass if nothing
        ever refreshed a session.

        The session is PROVEN first, and the first version of this witness
        was not: an unproven handshake gets a minute, so three thousand
        seconds ended it and the store said so. The rule was right and the
        witness had forgotten it.
        """
        self.store.prove(self.name)
        self.clock.forward(3000)
        self.store.unseal(self.name, self.send.encrypt_with_ad(b"", b"still here"))
        self.clock.forward(700)
        self.assertTrue(self.store.seal(self.name, b"and still here"))

    def test_a_handshake_in_use_stays_alive_inside_its_minute(self):
        # The unproven half of the same question: a sign in is an exchange,
        # and it must not end in the middle of itself.
        self.clock.forward(30)
        self.store.unseal(self.name, self.send.encrypt_with_ad(b"", b"signing in"))
        self.clock.forward(30)
        self.assertTrue(self.store.seal(self.name, b"a token"))


class TheStoreStaysBounded(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.service_key = X25519PrivateKey.generate()
        self.store = SessionStore(limits=Limits(capacity=3), now=self.clock)

    def _open_one(self):
        caller, first = a_caller(public_of(self.service_key))
        name, reply, _ = self.store.open(first, self.service_key)
        caller.read_message(reply)
        return name

    def test_a_flood_cannot_grow_it(self):
        for _ in range(30):
            self._open_one()
        self.assertLessEqual(len(self.store), 3)

    def test_a_flood_cannot_push_out_a_proven_session(self):
        # The same property the pure rule holds, driven through the real
        # store with real handshakes, because a store could apply the rule
        # to the wrong list and the pure test would never know.
        mine = self._open_one()
        self.store.prove(mine)
        for _ in range(30):
            self.clock.forward(1)
            self._open_one()
        self.assertTrue(self.store.seal(mine, b"still mine"))

    def test_proving_needs_a_live_session(self):
        name = self._open_one()
        self.clock.forward(2 * 3600)
        with self.assertRaises(UnknownSession):
            self.store.prove(name)

    def test_sweeping_says_how_many_it_dropped(self):
        for _ in range(3):
            self._open_one()
        self.assertEqual(self.store.sweep(), 0)
        self.clock.forward(120)
        self.assertEqual(self.store.sweep(), 3, "unproven handshakes get a minute")
        self.assertEqual(len(self.store), 0)


class TheNameIsUnguessable(unittest.TestCase):
    def test_two_sessions_never_share_a_name(self):
        clock = Clock()
        key = X25519PrivateKey.generate()
        store = SessionStore(limits=Limits(capacity=1000), now=clock)
        names = set()
        for _ in range(200):
            caller, first = a_caller(public_of(key))
            name, reply, _ = store.open(first, key)
            caller.read_message(reply)
            names.add(name)
        self.assertEqual(len(names), 200)
        # Long enough that guessing one is not a thing anybody does.
        self.assertTrue(all(len(name) == 32 for name in names))


if __name__ == "__main__":
    unittest.main()
