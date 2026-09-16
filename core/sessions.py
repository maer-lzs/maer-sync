# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""When an encrypted session ends, and which one gives way when the room is full.

The handshake in `core/handshake.py` produces two transport keys and a
counter per direction. Those have to live somewhere between requests, and a
store that only ever grows is not a store, it is a leak with a schedule. So
the rule for ending one is written here, pure, before the store that holds
them exists.

**Why this is a rule and not a number in the transport.** Two reasons, both
measured rather than assumed:

1. **The handshake is ANONYMOUS, by the pattern's design.** `NK` means the
   caller proves nothing until it is inside the session; that is exactly what
   lets a SIGN IN be protected, which is the most valuable thing on the wire.
   The price is that anybody who can reach the service can ask for a session.
   **The cap below is what turns that from a memory exhaustion path into a
   bounded cost**, and a cap is a decision, so it is written down where it
   can be read and tested.
2. **A session that ends must not look like a failure.** The client's answer
   to "I do not know this session" is one new handshake and one retry, never
   a sentence to a person. A service that says "not authorized" for something
   routine sends whoever reads the log in the wrong direction, and that has
   cost this project two hours before.

**The numbers are grounded in the client's own cadences, measured today:**
the service carrier fetches every 30 s, writes at most every 60 s, and the
playback claim beats every 10 s. So a live app touches its session at least
once a minute. An hour of silence is a backgrounded or gone app, and the cost
of being wrong is ONE extra round trip, which is why idle may be generous.

**What this file does NOT decide:** how the keys are held, which is the
transport's business, and whether a session survives a restart, which it does
not and cannot, because the keys live in the process. That is not a gap in
this rule: the client's answer to a dropped session is the same either way.
"""

#: An hour without a word. Sixty times the slowest regular cadence, so a
#: working app can never trip it, and a backgrounded one pays one round trip.
IDLE_SECONDS = 3600

#: A day, even for a session that never goes quiet. Long lived keys are the
#: thing a session is supposed to avoid, and the framework's own counter is
#: not the limit anybody should approach.
MAX_AGE_SECONDS = 86400

#: How many may be held at once. A session is two keys, two counters and two
#: moments; a few hundred bytes, call it a kilobyte with the language's
#: overhead. Four thousand of them is about four megabytes against the
#: image's 256, and it is far more devices than a self hosted service has.
#: The number is a ceiling for the anonymous caller, not a capacity plan.
CAPACITY = 4096


class Limits:
    """The three numbers together, so a test can lower them without touching
    the module and a deployment could raise them in one place."""

    __slots__ = ("idle_seconds", "max_age_seconds", "capacity")

    def __init__(self, idle_seconds=IDLE_SECONDS, max_age_seconds=MAX_AGE_SECONDS,
                 capacity=CAPACITY):
        self.idle_seconds = idle_seconds
        self.max_age_seconds = max_age_seconds
        self.capacity = capacity


#: What `standing` answers. Plain strings rather than an enum: they travel
#: into a log line and a test assertion, and both read better this way.
LIVE = "live"
IDLE = "idle"
OLD = "old"


def standing(created_at, last_used_at, now, limits=None):
    """Whether a session may still be used, and if not, why not.

    The reason is part of the answer because the two endings are not the same
    thing to whoever reads a log: `idle` means a device went away, `old`
    means a busy session reached its age. A single "expired" would hide which
    of the two numbers is wrong when one of them turns out to be.

    Age is checked BEFORE idleness. A session can be both, and the older of
    the two conditions is the one that would have ended it first.
    """
    limits = limits or Limits()
    if now - created_at >= limits.max_age_seconds:
        return OLD
    if now - last_used_at >= limits.idle_seconds:
        return IDLE
    return LIVE


def give_way(sessions, limits=None):
    """Which sessions leave so that ONE more fits, least recently used first.

    `sessions` is a sequence of `(id, last_used_at)`. The answer is a list of
    ids, and it is empty whenever there is room. Making space for exactly one
    is deliberate: the caller is about to add one, and a rule that cleared a
    generous margin would throw away live sessions to save work later.

    **Least recently used and not oldest by creation:** a device that has
    been talking for hours is the one most likely to talk again, and a
    creation time would evict exactly that one.
    """
    limits = limits or Limits()
    if len(sessions) < limits.capacity:
        return []
    # One over the capacity, plus the one about to be added.
    surplus = len(sessions) - limits.capacity + 1
    return [name for name, _ in sorted(sessions, key=lambda pair: pair[1])[:surplus]]


def sweep(sessions, now, limits=None):
    """The ids to drop right now: everything that is not `live`.

    Separate from `give_way` on purpose. This one answers "what is dead",
    which is true of a session whether or not anybody wants room, and a store
    that only cleaned up under pressure would hold a thousand dead sessions
    on a quiet service. `give_way` answers "what must leave anyway", which is
    a different question with a different answer.
    """
    limits = limits or Limits()
    return [name for name, created_at, last_used_at in sessions
            if standing(created_at, last_used_at, now, limits) != LIVE]
