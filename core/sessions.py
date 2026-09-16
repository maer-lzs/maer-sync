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
   **A cap alone does NOT answer that, and the first version of this file
   claimed it did.** A cap bounds the memory and turns exhaustion into
   DISPLACEMENT: a stranger who keeps asking pushes real devices out one by
   one, with no crash, no growth and no log line, and the design even
   declares the result normal, because a dropped session is supposed to cost
   one round trip and not a sentence. So the rule below asks a second
   question before it asks which session is oldest: **has this session ever
   carried a request that proved who was asking?** One that has not is a
   handshake and nothing more, and handshakes give way first.
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

#: An hour without a word. The client's regular cadences are 10, 30 and 60
#: seconds, so a working app can never trip this, and a backgrounded one pays
#: one round trip. **Those cadences say "Chosen, not measured" in their own
#: source**, so the margin is what carries the number, not their precision:
#: even at a cadence of 300 s this is twelve times the room.
IDLE_SECONDS = 3600

#: A session that has never proved anything gets a MINUTE, not an hour.
#: A real device hands over its handshake and makes its request immediately;
#: a handshake that is still unused a minute later belongs to somebody who
#: was not going to make one. This is what drains a flood continuously
#: instead of waiting for the cap to notice it.
UNPROVEN_IDLE_SECONDS = 60

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

    __slots__ = ("idle_seconds", "unproven_idle_seconds", "max_age_seconds", "capacity")

    def __init__(self, idle_seconds=IDLE_SECONDS,
                 unproven_idle_seconds=UNPROVEN_IDLE_SECONDS,
                 max_age_seconds=MAX_AGE_SECONDS, capacity=CAPACITY):
        self.idle_seconds = idle_seconds
        self.unproven_idle_seconds = unproven_idle_seconds
        self.max_age_seconds = max_age_seconds
        self.capacity = capacity


#: What `standing` answers. Plain strings rather than an enum: they travel
#: into a log line and a test assertion, and both read better this way.
LIVE = "live"
IDLE = "idle"
OLD = "old"


def standing(created_at, last_used_at, now, proven, limits=None):
    """Whether a session may still be used, and if not, why not.

    `proven` says whether this session has ever carried a request that
    proved who was asking. It has NO default on purpose: a default would be
    a guess about the most security relevant input here, and every caller
    knows the answer. Fail at the call site, not quietly in the middle.

    The reason is part of the answer because the endings are not the same
    thing to whoever reads a log: `idle` means a device went away, `old`
    means a busy session reached its age. A single "expired" would hide
    which of the numbers is wrong when one of them turns out to be.

    Age is checked BEFORE idleness. A session can be both, and the older of
    the two conditions is the one that would have ended it first.
    """
    limits = limits or Limits()
    if now - created_at >= limits.max_age_seconds:
        return OLD
    silence = limits.idle_seconds if proven else limits.unproven_idle_seconds
    if now - last_used_at >= silence:
        return IDLE
    return LIVE


def give_way(sessions, limits=None):
    """Which sessions leave so that ONE more fits.

    `sessions` is a sequence of `(id, last_used_at, proven)`. The answer is a
    list of ids, empty whenever there is room.

    **Unproven first, and that is the whole point of this function.** Least
    recently used ALONE is what the first version did, and it is exactly
    wrong under the one pressure this store will actually meet: the
    handshake is anonymous, so a stranger can make sessions as fast as the
    network allows, and every new one is the most recently used thing in the
    store. Under plain LRU the strangers are therefore always the survivors
    and the real devices are always the ones pushed out, continuously, with
    no crash and no log line. Asking "has this one ever proved anything"
    first turns that around: a flood can only displace other handshakes.

    **The limit, and it is real:** a flood still competes with a REAL
    device's first handshake, because that one is unproven too until its
    first request lands. So a stranger can delay somebody signing in; it
    cannot push out anybody who already is. The short idle life of an
    unproven session is the other half of that, and neither half is a claim
    that the store is free.

    Making space for exactly one is deliberate: the caller is about to add
    one, and a rule that cleared a generous margin would throw away live
    sessions to save work later.

    Within each group, least recently used and not oldest by creation: a
    device that has been talking for hours is the one most likely to talk
    again, and a creation time would evict exactly that one.
    """
    limits = limits or Limits()
    if len(sessions) < limits.capacity:
        return []
    surplus = len(sessions) - limits.capacity + 1
    # Unproven before proven, then least recently used inside each group.
    order = sorted(sessions, key=lambda row: (bool(row[2]), row[1]))
    return [row[0] for row in order[:surplus]]


def sweep(sessions, now, limits=None):
    """The ids to drop right now: everything that is not `live`.

    `sessions` is a sequence of `(id, created_at, last_used_at, proven)`.

    Separate from `give_way` on purpose. This one answers "what is dead",
    which is true of a session whether or not anybody wants room, and a
    store that only cleaned up under pressure would hold a thousand dead
    sessions on a quiet service. `give_way` answers "what must leave
    anyway", which is a different question with a different answer.

    Against a flood this is the half that works without pressure: an unused
    handshake dies after a minute, so the strangers drain continuously
    rather than sitting at the ceiling until somebody needs the room.
    """
    limits = limits or Limits()
    return [name for name, created_at, last_used_at, proven in sessions
            if standing(created_at, last_used_at, now, proven, limits) != LIVE]


def room_for_one(sessions, now, limits=None):
    """Everything that must leave so that one more session fits. ONE door.

    `sessions` is a sequence of `(id, created_at, last_used_at, proven)`, and
    the answer is the ids to drop, dead ones included.

    **This exists because the ORDER is the decision, and a caller that made
    two calls would get it wrong.** Sweeping first and giving way afterwards
    is not the same as the reverse, and the difference is not academic:

        two proven sessions, silent for hours, and one real device in the
        middle of its handshake, in a store of three.

    Give way first and the rule evicts the real device, because a handshake
    gives way before a proven session does and both dead rows still occupy
    their places. Sweep first and the two dead ones go, the device stays, and
    nothing gives way at all. **A dead session must never cost a living one
    its place.**

    **And this is also the answer to when sweeping happens.** The service has
    no clock, measured: not one `create_task`, timer or sleeping loop
    anywhere in it, and its lifespan handler only opens and closes the store.
    A background task would be a new KIND of thing here, one that has to be
    cancelled on shutdown and that can die without anybody noticing.

    It does not need one. **Correctness never depends on sweeping**, because
    a session is checked with `standing` when it is USED, so an expired one
    is refused whether or not anybody has swept it. Sweeping is housekeeping
    for memory alone, and the store only grows on a handshake. So this runs
    where the growth is, and nowhere else.

    The price, stated: a service that gets no more handshakes keeps its dead
    rows until one arrives. They are bounded by the capacity, which is the
    number that was already answering for memory.
    """
    limits = limits or Limits()
    dead = set(sweep(sessions, now, limits))
    alive = [(name, last_used_at, proven)
             for name, _, last_used_at, proven in sessions if name not in dead]
    return sorted(dead) + give_way(alive, limits)
