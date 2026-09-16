# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""That a session store cannot grow without end, and ends the right ones.

The cases are mostly about EDGES, because the middle of each rule is
uninteresting and the edges are where an off by one lives: exactly at a
limit, one below it, exactly at capacity.

**The case that carries the file is `TheAnonymousCallerIsBounded`.** The
handshake proves nothing about who is asking, on purpose, so the only thing
standing between a stranger and the service's memory is the cap. Every other
case here would pass with an unbounded store.
"""

import unittest

from core.sessions import (CAPACITY, IDLE, IDLE_SECONDS, LIVE, MAX_AGE_SECONDS, OLD,
                           Limits, give_way, standing, sweep)

NOW = 1_000_000.0


class WhenASessionEnds(unittest.TestCase):
    def test_a_fresh_session_is_live(self):
        self.assertEqual(standing(NOW, NOW, NOW), LIVE)

    def test_a_session_used_a_minute_ago_is_live(self):
        # The client's slowest regular cadence is sixty seconds, so this is
        # the ordinary case and it must never end.
        self.assertEqual(standing(NOW - 600, NOW - 60, NOW), LIVE)

    def test_silence_past_the_idle_limit_ends_it(self):
        self.assertEqual(standing(NOW - IDLE_SECONDS - 1, NOW - IDLE_SECONDS, NOW), IDLE)

    def test_one_second_short_of_the_idle_limit_is_still_live(self):
        self.assertEqual(standing(NOW - IDLE_SECONDS, NOW - IDLE_SECONDS + 1, NOW), LIVE)

    def test_a_busy_session_still_ends_at_its_age(self):
        # Used one second ago, so nothing about idleness applies. The age
        # limit is the whole point of this case: without it a chatty client
        # would keep one set of keys alive forever.
        self.assertEqual(standing(NOW - MAX_AGE_SECONDS, NOW - 1, NOW), OLD)

    def test_age_is_answered_before_idleness(self):
        # A session can be both. The reason travels into a log line, and the
        # condition that would have ended it FIRST is the honest answer.
        self.assertEqual(standing(NOW - MAX_AGE_SECONDS - 10_000, NOW - IDLE_SECONDS - 1, NOW), OLD)

    def test_the_two_endings_are_told_apart(self):
        # A single "expired" would hide which of the two numbers is wrong on
        # the day one of them turns out to be.
        self.assertNotEqual(IDLE, OLD)


class TheAnonymousCallerIsBounded(unittest.TestCase):
    """The decision of this file, and the reason it exists.

    `NK` lets the caller stay anonymous through the handshake, which is what
    protects a sign in. The price is that anybody who can reach the service
    can ask for a session, so the store MUST be bounded. Every other case
    here would pass just as happily with a store that grows forever.
    """

    def test_below_capacity_nobody_gives_way(self):
        sessions = [(f"s{i}", NOW - i) for i in range(10)]
        self.assertEqual(give_way(sessions, Limits(capacity=100)), [])

    def test_at_capacity_exactly_one_gives_way(self):
        limits = Limits(capacity=4)
        sessions = [("a", NOW - 1), ("b", NOW - 2), ("c", NOW - 3), ("d", NOW - 4)]
        self.assertEqual(give_way(sessions, limits), ["d"],
                         "the least recently used one, and only one, because one is being added")

    def test_over_capacity_enough_give_way_to_leave_room_for_one(self):
        limits = Limits(capacity=4)
        sessions = [("a", NOW - 1), ("b", NOW - 2), ("c", NOW - 3),
                    ("d", NOW - 4), ("e", NOW - 5), ("f", NOW - 6)]
        self.assertEqual(give_way(sessions, limits), ["f", "e", "d"])

    def test_the_busy_one_survives_even_when_it_is_the_oldest(self):
        # Least recently USED and not oldest by creation: a device that has
        # been talking for hours is the one most likely to talk again, and
        # evicting it would be the opposite of what the store is for.
        limits = Limits(capacity=2)
        talking_for_hours = ("old-but-busy", NOW - 1)
        made_and_forgotten = ("new-but-quiet", NOW - 5000)
        self.assertEqual(give_way([talking_for_hours, made_and_forgotten], limits),
                         ["new-but-quiet"])

    def test_a_flood_of_strangers_cannot_grow_the_store(self):
        # The shape of the attack, written out: a stranger asks for sessions
        # as fast as it can. Each new one pushes exactly one out, so the
        # store stays at its ceiling instead of climbing.
        limits = Limits(capacity=3)
        held = [("mine", NOW)]
        for i in range(50):
            leaving = give_way(held, limits)
            held = [pair for pair in held if pair[0] not in leaving]
            held.append((f"stranger{i}", NOW - 1000))
        self.assertLessEqual(len(held), limits.capacity)
        # And the live one survives the flood, because the strangers are the
        # least recently used ones the moment they stop asking.
        self.assertIn("mine", [name for name, _ in held])

    def test_the_shipped_capacity_is_a_ceiling_and_not_a_plan(self):
        # The number is asserted so that raising it is a decision somebody
        # makes here, with the arithmetic in front of them, rather than a
        # quiet edit. Four thousand sessions at about a kilobyte is about
        # four megabytes against the image's 256.
        self.assertEqual(CAPACITY, 4096)


class SweepingIsNotTheSameQuestion(unittest.TestCase):
    def test_dead_sessions_go_even_when_there_is_plenty_of_room(self):
        # A store that only cleaned up under pressure would hold a thousand
        # dead sessions on a quiet service, and the memory of a service
        # nobody uses is exactly where nobody looks.
        sessions = [("live", NOW - 10, NOW - 1),
                    ("quiet", NOW - 10_000, NOW - IDLE_SECONDS - 1),
                    ("ancient", NOW - MAX_AGE_SECONDS - 1, NOW - 1)]
        self.assertEqual(sorted(sweep(sessions, NOW)), ["ancient", "quiet"])

    def test_a_store_of_live_sessions_sweeps_to_nothing(self):
        sessions = [(f"s{i}", NOW - 10, NOW - 1) for i in range(5)]
        self.assertEqual(sweep(sessions, NOW), [])

    def test_sweeping_an_empty_store_is_not_an_error(self):
        self.assertEqual(sweep([], NOW), [])


if __name__ == "__main__":
    unittest.main()
