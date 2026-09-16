# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Where the encrypted sessions live between requests.

`core/handshake.py` makes them, `core/sessions.py` says when they end, and
this holds them. Nothing here decides anything: every rule it applies is
imported, so the rules stay testable without a store and the store stays a
container.

**It is a dictionary in the process, and that is not a shortcut.** The keys
cannot be written down anywhere: a session that survived a restart would be
a session whose keys were on disk, and then the restart is not the thing
that ended it, a backup is. The service already cannot run in more than one
process (its socket listeners are in-process too, and uvicorn refuses
`workers > 1` for an application that is not an import string), so this
joins a condition rather than creating one.

**A restart therefore drops every session, and that is by design.** The
client's answer is one new handshake and one retry, never a sentence to a
person.

**What is NOT here, deliberately:** any HTTP path. The store is a slice of
its own because adding a path is a contract change, and the contract is
byte identical in two repositories with a fence on each side. So the paths
arrive together with the half of the app that speaks them.
"""

import secrets
import time

from core.handshake import HandshakeState
from core.sessions import LIVE, Limits, room_for_one, standing

#: Bytes of the session name. It is not a secret, it only has to be
#: unguessable: whoever holds it can make a request LOOK like it belongs to
#: a session, and although the sealing would refuse the message, a guessable
#: name would let a stranger keep somebody else's session from being swept
#: or push it out of the store.
NAME_BYTES = 16


class UnknownSession(Exception):
    """No such session, or one that has ended. ONE exception for both on
    purpose: the caller's answer is the same (hand back a refusal the client
    answers with a new handshake), and telling them apart would say whether
    a name ever existed."""


class SessionStore:
    """The live sessions, by name.

    `now` is injected so a witness can move time without sleeping, which is
    the only way to test an hour of silence in a suite that must stay fast.
    """

    def __init__(self, limits=None, now=time.time):
        self.limits = limits or Limits()
        self._now = now
        self._rows = {}

    def __len__(self):
        return len(self._rows)

    # MARK: - Making one

    def open(self, first_message, static_private, prologue=b""):
        """Answer a handshake and keep the session. Returns `(name, reply)`.

        The room is made BEFORE the session is added, through the one door
        that sweeps and then evicts, because a dead row must never cost a
        living one its place.

        A handshake that does not add up raises out of `read_message`, and
        nothing is stored: a caller that cannot complete the exchange leaves
        no trace here, which is what keeps a failed flood from costing
        anything at all.
        """
        state = HandshakeState(initiator=False, prologue=prologue,
                               static_private=static_private)
        payload = state.read_message(first_message)
        reply = state.write_message(b"")
        send, receive = state.split()

        now = self._now()
        for name in room_for_one(self._as_rows(), now, self.limits):
            self._rows.pop(name, None)

        name = secrets.token_hex(NAME_BYTES)
        self._rows[name] = {"send": send, "receive": receive,
                            "made": now, "used": now, "proven": False}
        return name, reply, payload

    # MARK: - Using one

    def _live_row(self, name):
        row = self._rows.get(name)
        if row is None:
            raise UnknownSession("no such session")
        if standing(row["made"], row["used"], self._now(), row["proven"], self.limits) != LIVE:
            # Dropped on the spot rather than left for the next sweep: it
            # can never be used again, and holding it would let a stranger
            # keep a name alive by asking about it.
            self._rows.pop(name, None)
            raise UnknownSession("this session has ended")
        return row

    def unseal(self, name, ciphertext, ad=b""):
        """Open one message. Raises `UnknownSession`, or whatever the cipher
        raises when the message is not authentic.

        **The moment of use is recorded only on SUCCESS.** A stranger who
        knows a name and sends noise must not be able to keep that session
        alive by failing at it.
        """
        row = self._live_row(name)
        plaintext = row["receive"].decrypt_with_ad(ad, ciphertext)
        row["used"] = self._now()
        return plaintext

    def seal(self, name, plaintext, ad=b""):
        """Seal one message for the other side."""
        row = self._live_row(name)
        return row["send"].encrypt_with_ad(ad, plaintext)

    def prove(self, name):
        """Say that this session has carried a request that proved who was
        asking. From here on it gives way only after every handshake has.

        Separate from `unseal` because opening a message proves that somebody
        holds the keys, not that they hold an ACCOUNT. The token inside is
        what proves the second, and only the layer that reads it knows.
        """
        self._live_row(name)["proven"] = True

    # MARK: - Housekeeping

    def _as_rows(self):
        return [(name, row["made"], row["used"], row["proven"])
                for name, row in self._rows.items()]

    def sweep(self):
        """Drop what has ended, and say how many. Not on a clock: nothing in
        this service runs on one, and correctness never depends on this
        because every use checks the standing anyway."""
        now = self._now()
        gone = [name for name, made, used, proven in self._as_rows()
                if standing(made, used, now, proven, self.limits) != LIVE]
        for name in gone:
            self._rows.pop(name, None)
        return len(gone)
