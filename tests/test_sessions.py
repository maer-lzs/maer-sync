# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""That a session store cannot grow without end, and ends the RIGHT ones.

The cases are mostly about EDGES, because the middle of each rule is
uninteresting and the edges are where an off by one lives: exactly at a
limit, one below it, exactly at capacity.

**The case that carries the file is `AFloodCannotPushOutARealDevice`, and
the first version of it was wrong in a way worth writing down.** It gave the
attacker's sessions a last use of `NOW - 1000`, a thousand seconds in the
past. A session that was just asked for is used NOW, so the fixture handed
the attacker a history he cannot have, and that history is precisely what
made him harmless: with it, every stranger is instantly the least recently
used thing in the store and pushes himself back out.

Run with honest stamps and the same rule, the real session was gone after
fifty requests. So the case proved a property of the fixture, not of the
rule, and it read as a green tick over an open door.

**The shape to remember:** a witness whose fixture is unrealistic in exactly
the direction that makes it pass. Related to "a measuring case must assure
its precondition", one turn further: here the precondition was not missing,
it was flattering.
"""

import unittest

from core.sessions import (CAPACITY, IDLE, IDLE_SECONDS, LIVE, MAX_AGE_SECONDS, OLD,
                           UNPROVEN_IDLE_SECONDS, Limits, give_way, standing, sweep)

NOW = 1_000_000.0

#: Readable names for the third field, because `True` in a tuple says
#: nothing at the call site about which question it answers.
PROVEN = True
JUST_A_HANDSHAKE = False


class WhenASessionEnds(unittest.TestCase):
    def test_a_fresh_session_is_live(self):
        self.assertEqual(standing(NOW, NOW, NOW, PROVEN), LIVE)

    def test_a_session_used_a_minute_ago_is_live(self):
        # The client's slowest regular cadence is sixty seconds, so this is
        # the ordinary case and it must never end.
        self.assertEqual(standing(NOW - 600, NOW - 60, NOW, PROVEN), LIVE)

    def test_silence_past_the_idle_limit_ends_it(self):
        self.assertEqual(
            standing(NOW - IDLE_SECONDS - 1, NOW - IDLE_SECONDS, NOW, PROVEN), IDLE)

    def test_one_second_short_of_the_idle_limit_is_still_live(self):
        self.assertEqual(
            standing(NOW - IDLE_SECONDS, NOW - IDLE_SECONDS + 1, NOW, PROVEN), LIVE)

    def test_a_busy_session_still_ends_at_its_age(self):
        # Used one second ago, so nothing about idleness applies. The age
        # limit is the whole point of this case: without it a chatty client
        # would keep one set of keys alive forever.
        self.assertEqual(standing(NOW - MAX_AGE_SECONDS, NOW - 1, NOW, PROVEN), OLD)

    def test_age_is_answered_before_idleness(self):
        # A session can be both. The reason travels into a log line, and the
        # condition that would have ended it FIRST is the honest answer.
        self.assertEqual(
            standing(NOW - MAX_AGE_SECONDS - 10_000, NOW - IDLE_SECONDS - 1, NOW, PROVEN),
            OLD)

    def test_the_two_endings_are_told_apart(self):
        # A single "expired" would hide which of the two numbers is wrong on
        # the day one of them turns out to be.
        self.assertNotEqual(IDLE, OLD)


class AHandshakeThatProvesNothingGetsAMinute(unittest.TestCase):
    """The half that works without pressure.

    A real device hands over its handshake and makes its request at once. A
    handshake still unused a minute later belongs to somebody who was never
    going to make one, and letting it sit for an hour like a working session
    is what lets a flood park at the ceiling.
    """

    def test_an_unused_handshake_dies_after_a_minute(self):
        self.assertEqual(
            standing(NOW - 100, NOW - UNPROVEN_IDLE_SECONDS, NOW, JUST_A_HANDSHAKE), IDLE)

    def test_a_proven_session_of_the_same_age_is_still_live(self):
        # The same numbers, one field different. Without this the case above
        # would also pass if the short life applied to everybody, and that
        # would end real sessions every minute.
        self.assertEqual(
            standing(NOW - 100, NOW - UNPROVEN_IDLE_SECONDS, NOW, PROVEN), LIVE)

    def test_a_handshake_used_a_moment_ago_is_live(self):
        self.assertEqual(standing(NOW - 1, NOW - 1, NOW, JUST_A_HANDSHAKE), LIVE)


class TheAnonymousCallerIsBounded(unittest.TestCase):
    def test_below_capacity_nobody_gives_way(self):
        sessions = [(f"s{i}", NOW - i, PROVEN) for i in range(10)]
        self.assertEqual(give_way(sessions, Limits(capacity=100)), [])

    def test_at_capacity_exactly_one_gives_way(self):
        limits = Limits(capacity=4)
        sessions = [("a", NOW - 1, PROVEN), ("b", NOW - 2, PROVEN),
                    ("c", NOW - 3, PROVEN), ("d", NOW - 4, PROVEN)]
        self.assertEqual(give_way(sessions, limits), ["d"],
                         "the least recently used one, and only one, because one is being added")

    def test_over_capacity_enough_give_way_to_leave_room_for_one(self):
        limits = Limits(capacity=4)
        sessions = [("a", NOW - 1, PROVEN), ("b", NOW - 2, PROVEN), ("c", NOW - 3, PROVEN),
                    ("d", NOW - 4, PROVEN), ("e", NOW - 5, PROVEN), ("f", NOW - 6, PROVEN)]
        self.assertEqual(give_way(sessions, limits), ["f", "e", "d"])

    def test_the_busy_one_survives_even_when_it_is_the_oldest(self):
        # Least recently USED and not oldest by creation, inside a group.
        limits = Limits(capacity=2)
        talking_for_hours = ("old-but-busy", NOW - 1, PROVEN)
        made_and_forgotten = ("new-but-quiet", NOW - 5000, PROVEN)
        self.assertEqual(give_way([talking_for_hours, made_and_forgotten], limits),
                         ["new-but-quiet"])

    def test_a_handshake_gives_way_before_a_proven_session_does(self):
        # Even when the handshake is the one used a moment ago and the proven
        # session has been quiet for an hour. This is the ordering, stated on
        # its own so that it cannot be lost inside the flood case.
        limits = Limits(capacity=2)
        fresh_handshake = ("stranger", NOW, JUST_A_HANDSHAKE)
        quiet_but_proven = ("mine", NOW - 3000, PROVEN)
        self.assertEqual(give_way([fresh_handshake, quiet_but_proven], limits), ["stranger"])

    def test_the_shipped_capacity_is_a_ceiling_and_not_a_plan(self):
        # The number is asserted so that raising it is a decision somebody
        # makes here, with the arithmetic in front of them, rather than a
        # quiet edit. Four thousand sessions at about a kilobyte is about
        # four megabytes against the image's 256.
        self.assertEqual(CAPACITY, 4096)


class AFloodCannotPushOutARealDevice(unittest.TestCase):
    """The attack, with stamps a stranger could really produce.

    Every session the attacker asks for is used AT THE MOMENT it is made, so
    it is the most recently used row in the store. Under plain least
    recently used that makes the attacker the permanent survivor and the
    real device the permanent victim, continuously and silently.
    """

    def _flood(self, limits, rounds=50):
        held = [("mine", NOW, PROVEN)]
        for i in range(rounds):
            leaving = give_way(held, limits)
            held = [row for row in held if row[0] not in leaving]
            # Made now, used now. Not a thousand seconds ago.
            held.append((f"stranger{i}", NOW + i, JUST_A_HANDSHAKE))
        return held

    def test_the_store_stays_at_its_ceiling(self):
        limits = Limits(capacity=3)
        self.assertLessEqual(len(self._flood(limits)), limits.capacity)

    def test_the_real_session_is_still_there_afterwards(self):
        limits = Limits(capacity=3)
        held = self._flood(limits)
        self.assertIn("mine", [name for name, _, _ in held],
                      "a stranger who keeps asking must not be able to displace a device "
                      "that has already proved who it is")

    def test_it_survives_a_flood_far_larger_than_the_store(self):
        # A hundred times the capacity, so that no arrangement of a few
        # rounds can be what carries the case.
        limits = Limits(capacity=5)
        held = self._flood(limits, rounds=500)
        self.assertIn("mine", [name for name, _, _ in held])

    def test_the_strangers_drain_by_themselves_once_they_stop(self):
        # The other half, and it needs no pressure at all: unused handshakes
        # die after a minute, so the store empties instead of sitting full.
        limits = Limits(capacity=3)
        held = self._flood(limits)
        later = NOW + 1000
        dead = sweep([(name, NOW, used, proven) for name, used, proven in held], later, limits)
        self.assertEqual(sorted(dead), sorted(name for name, _, proven in held if not proven))


class SweepingIsNotTheSameQuestion(unittest.TestCase):
    def test_dead_sessions_go_even_when_there_is_plenty_of_room(self):
        # A store that only cleaned up under pressure would hold a thousand
        # dead sessions on a quiet service, and the memory of a service
        # nobody uses is exactly where nobody looks.
        sessions = [("live", NOW - 10, NOW - 1, PROVEN),
                    ("quiet", NOW - 10_000, NOW - IDLE_SECONDS - 1, PROVEN),
                    ("ancient", NOW - MAX_AGE_SECONDS - 1, NOW - 1, PROVEN)]
        self.assertEqual(sorted(sweep(sessions, NOW)), ["ancient", "quiet"])

    def test_a_store_of_live_sessions_sweeps_to_nothing(self):
        sessions = [(f"s{i}", NOW - 10, NOW - 1, PROVEN) for i in range(5)]
        self.assertEqual(sweep(sessions, NOW), [])

    def test_sweeping_an_empty_store_is_not_an_error(self):
        self.assertEqual(sweep([], NOW), [])


if __name__ == "__main__":
    unittest.main()
