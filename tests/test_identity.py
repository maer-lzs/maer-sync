# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""That the service keeps ONE identity, and that losing it is loud.

The handshake is only worth its vectors if the key behind it stays put. So
the cases here are mostly about the ways it could move: a changed file, a
missing file, a damaged file, a database restored somewhere else. Each of
those has exactly one right answer and several plausible wrong ones, and the
wrong ones all look like the service working.

**The witness that carries the slice is `TheKeyIsNotInTheDatabase`.** Every
other case here would pass just as happily if the private key were a row in
the store, and that is the one construction this slice exists to refuse.
"""

import base64
import os
import pathlib
import tempfile
import unittest

from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from core.identity import ADOPT, ALPHABET, MAKE_NEW, decide, fingerprint
from service.identity import (RefusedToServe, adopt, key_path, make_new, open_identity,
                              public_of, read_key, standing, write_key)
from service.store import Store


def a_key():
    return X25519PrivateKey.generate()


class TheWrittenForm(unittest.TestCase):
    def test_it_is_six_groups_of_four(self):
        printed = fingerprint(public_of(a_key()))
        groups = printed.split("-")
        self.assertEqual(len(groups), 6)
        self.assertTrue(all(len(g) == 4 for g in groups), printed)
        self.assertTrue(all(c in ALPHABET for g in groups for c in g), printed)

    def test_the_confusable_letters_are_not_in_the_alphabet(self):
        # The whole reason for this alphabet rather than plain base32: a
        # person compares two of these by eye, and O against 0 is the pair
        # that makes a mismatch look like a match.
        for confusable in "ILOU":
            self.assertNotIn(confusable, ALPHABET)

    def test_the_same_key_always_prints_the_same_and_two_keys_do_not(self):
        one, other = a_key(), a_key()
        self.assertEqual(fingerprint(public_of(one)), fingerprint(public_of(one)))
        self.assertNotEqual(fingerprint(public_of(one)), fingerprint(public_of(other)))

    def test_a_truncated_key_is_refused_rather_than_fingerprinted(self):
        # A short key means something was cut on the way here, which is
        # exactly when a fingerprint must not come back looking fine.
        with self.assertRaises(ValueError):
            fingerprint(public_of(a_key())[:31])


class TheDecision(unittest.TestCase):
    def test_nothing_here_and_nothing_remembered_makes_the_first_key(self):
        verdict = decide(None, None)
        self.assertEqual(verdict.name, "make_one")
        self.assertTrue(verdict.may_serve)

    def test_a_key_here_that_the_store_never_saw_is_recorded(self):
        verdict = decide("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF", None)
        self.assertEqual(verdict.name, "remember")
        self.assertTrue(verdict.may_serve)

    def test_the_expected_key_carries_on(self):
        verdict = decide("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF", "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF")
        self.assertEqual(verdict.name, "carry_on")
        self.assertTrue(verdict.may_serve)

    def test_a_different_key_refuses_and_names_the_way_out(self):
        verdict = decide("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF", "ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT")
        self.assertEqual(verdict.name, "changed")
        self.assertFalse(verdict.may_serve)
        # A refusal that reaches a terminal without saying what to do about
        # it is only half a refusal, so the command is part of the promise.
        self.assertIn(ADOPT, verdict.reason)
        self.assertIn("ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT", verdict.reason)

    def test_a_missing_key_refuses_and_names_the_way_out(self):
        verdict = decide(None, "ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT")
        self.assertEqual(verdict.name, "gone")
        self.assertFalse(verdict.may_serve)
        self.assertIn(MAKE_NEW, verdict.reason)


class OnDisk(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.db = os.path.join(self.dir, "store.sqlite")
        self.key = pathlib.Path(self.dir) / "identity.key"
        self.store = Store(self.db)

    def tearDown(self):
        self.store.close()

    def test_the_first_start_makes_a_key_and_remembers_it(self):
        private, printed = open_identity(self.store, self.key)
        self.assertTrue(self.key.exists())
        self.assertEqual(self.store.remembered_key(), printed)
        self.assertEqual(fingerprint(public_of(private)), printed)

    def test_the_key_file_is_owner_only(self):
        open_identity(self.store, self.key)
        self.assertEqual(self.key.stat().st_mode & 0o777, 0o600)

    def test_a_second_start_does_not_replace_the_key(self):
        first, printed = open_identity(self.store, self.key)
        again, printed_again = open_identity(self.store, self.key)
        self.assertEqual(first.private_bytes_raw(), again.private_bytes_raw())
        self.assertEqual(printed, printed_again)

    def test_a_changed_key_file_refuses_and_leaves_the_memory_alone(self):
        _, printed = open_identity(self.store, self.key)
        write_key(self.key, a_key())
        with self.assertRaises(RefusedToServe):
            open_identity(self.store, self.key)
        # The memory must survive the refusal, or the second attempt would
        # look like the permissive `remember` case and let the stranger in.
        self.assertEqual(self.store.remembered_key(), printed)

    def test_a_missing_key_file_refuses_instead_of_growing_a_new_one(self):
        _, printed = open_identity(self.store, self.key)
        self.key.unlink()
        with self.assertRaises(RefusedToServe):
            open_identity(self.store, self.key)
        self.assertFalse(self.key.exists(), "a refusal must not lay down a key")
        self.assertEqual(self.store.remembered_key(), printed)

    def test_a_damaged_key_file_is_not_read_as_absence(self):
        # Absence means "this service is new". A damaged file means somebody
        # has a key and it is not this one, and starting fresh on top of it
        # would throw the real one away.
        self.key.write_text("this is not a key\n", encoding="utf-8")
        with self.assertRaises(RefusedToServe):
            read_key(self.key)
        self.assertEqual(self.key.read_text(encoding="utf-8"), "this is not a key\n")

    def test_a_key_of_the_wrong_length_is_refused_too(self):
        self.key.write_text(base64.b64encode(b"short").decode("ascii"), encoding="utf-8")
        with self.assertRaises(RefusedToServe):
            read_key(self.key)

    def test_a_store_restored_beside_its_own_key_adopts_it_without_being_asked(self):
        # The ordinary recovery: the database comes back from a backup that
        # predates the memory, the machine still holds its key. Refusing here
        # would break exactly the case this construction is meant to allow.
        private = a_key()
        write_key(self.key, private)
        carried, printed = open_identity(self.store, self.key)
        self.assertEqual(carried.private_bytes_raw(), private.private_bytes_raw())
        self.assertEqual(self.store.remembered_key(), printed)


class TheOperatorSaysItIsWanted(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.db = os.path.join(self.dir, "store.sqlite")
        self.key = pathlib.Path(self.dir) / "identity.key"
        self.store = Store(self.db)

    def tearDown(self):
        self.store.close()

    def test_adopt_takes_the_key_that_is_there_and_says_what_was_there_before(self):
        _, was = open_identity(self.store, self.key)
        write_key(self.key, a_key())
        found, previously = adopt(self.store, self.key)
        self.assertEqual(previously, was)
        self.assertNotEqual(found, was)
        self.assertEqual(self.store.remembered_key(), found)
        # And the service starts again, which is the point of the command.
        self.assertTrue(standing(self.store, self.key)[0].may_serve)

    def test_adopt_refuses_to_quietly_make_a_key(self):
        # An "adopt" that generates when there is nothing to adopt would be
        # the silent replacement the whole module is against.
        with self.assertRaises(RefusedToServe):
            adopt(self.store, self.key)
        self.assertFalse(self.key.exists())

    def test_new_makes_one_and_keeps_the_old_file_beside_it(self):
        _, was = open_identity(self.store, self.key)
        before = self.key.read_text(encoding="utf-8")
        found, previously, kept = make_new(self.store, self.key)
        self.assertEqual(previously, was)
        self.assertNotEqual(found, was)
        self.assertEqual(self.store.remembered_key(), found)
        # A rotation done by mistake must not be unrecoverable.
        self.assertIsNotNone(kept)
        self.assertEqual(pathlib.Path(kept).read_text(encoding="utf-8"), before)

    def test_showing_the_standing_changes_nothing(self):
        _, printed = open_identity(self.store, self.key)
        raw = self.key.read_bytes()
        verdict, _, found = standing(self.store, self.key)
        self.assertTrue(verdict.may_serve)
        self.assertEqual(found, printed)
        self.assertEqual(self.key.read_bytes(), raw)
        self.assertEqual(self.store.remembered_key(), printed)


class TheKeyIsNotInTheDatabase(unittest.TestCase):
    """The decision of this slice, measured rather than asserted in prose.

    Every other case in this file would pass with the private key stored as
    a row. This one is the reason it is not: `python3 -m service backup`
    hands the database to wherever backups go, and a private key in it lets
    anybody holding an old copy answer as this service to every device that
    ever pinned it.
    """

    def test_the_private_key_does_not_appear_in_the_store_or_its_backup(self):
        directory = tempfile.mkdtemp()
        db = os.path.join(directory, "store.sqlite")
        key = pathlib.Path(directory) / "identity.key"
        store = Store(db)
        private, printed = open_identity(store, key)
        store.write("the-owner", "stands", b"{}", 1)
        copy = os.path.join(directory, "copy.sqlite")
        store.backup(copy)
        store.close()

        raw = private.private_bytes_raw()
        written = base64.b64encode(raw)
        for path in (db, copy):
            body = pathlib.Path(path).read_bytes()
            self.assertNotIn(raw, body, f"the raw private key is inside {path}")
            self.assertNotIn(written, body, f"the written private key is inside {path}")
            # The fingerprint IS in there, and that is the point: it names
            # without proving, so the memory may travel where the key may not.
            self.assertIn(printed.encode("ascii"), body)

    def test_the_key_file_is_not_swept_up_by_a_backup_glob(self):
        # `store.sqlite*` is what an operator's backup line looks like. The
        # key must not be caught by it, which is why it is not named after
        # the database.
        directory = tempfile.mkdtemp()
        db = os.path.join(directory, "store.sqlite")
        store = Store(db)
        # The default is what is being measured, so an override from the
        # environment would make this case answer a different question.
        named = os.environ.pop("MAER_SYNC_KEY", None)
        try:
            path = key_path(db)
            self.assertFalse(str(path).startswith(db),
                             f"{path} would be caught by a glob on {db}")
            self.assertEqual(path.parent, pathlib.Path(directory))
        finally:
            if named is not None:
                os.environ["MAER_SYNC_KEY"] = named
            store.close()


if __name__ == "__main__":
    unittest.main()
