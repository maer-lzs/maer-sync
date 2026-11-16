# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The fence on names of roles and people in the tracked files.

The list lives in tests/role-names.txt; the hooks read the same file. A
phrase in tests/role-names-allowed.txt may carry a name when it is not a
role or a person, and each such phrase must still occur somewhere.
"""
import pathlib
import re
import unittest

from tests import gitquery

ROOT = pathlib.Path(__file__).resolve().parent.parent
NAMES = ROOT / "tests" / "role-names.txt"
ALLOWED = ROOT / "tests" / "role-names-allowed.txt"
OWN_FILES = {"tests/role-names.txt", "tests/role-names-allowed.txt"}
HEADER = re.compile(r"^\s*(//!?|#)\s*Copyright \(C\) \d{4} .+$")


def read_list(path):
    entries = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        entries.append(line.split(" # ", 1)[0].strip())
    return entries


class RoleNameTests(unittest.TestCase):
    def test_the_list_is_there_and_not_empty(self):
        self.assertTrue(NAMES.is_file(), "tests/role-names.txt is missing")
        self.assertGreater(len(read_list(NAMES)), 0, "the list of names is empty")

    def test_no_tracked_file_names_a_role(self):
        files = [f for f in gitquery.tracked()
                 if f not in OWN_FILES and not f.startswith("LICENSE")]
        self.assertGreater(len(files), 0, "no tracked file was read")
        names = read_list(NAMES)
        allowed = read_list(ALLOWED)
        pattern = re.compile(r"\b(" + "|".join(map(re.escape, names)) + r")\b", re.I)
        found = []
        for name in files:
            try:
                text = (ROOT / name).read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            for number, line in enumerate(text.splitlines(), 1):
                if HEADER.match(line):
                    continue
                for phrase in allowed:
                    line = line.replace(phrase, " ")
                if pattern.search(line):
                    found.append(f"{name}:{number}")
        self.assertEqual(found, [], "names of roles or people in tracked files")

    def test_every_allowed_phrase_still_occurs(self):
        files = [f for f in gitquery.tracked() if f not in OWN_FILES]
        text = ""
        for name in files:
            try:
                text += (ROOT / name).read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
        self.assertGreater(len(text), 0, "no tracked file was read")
        stale = [p for p in read_list(ALLOWED) if p not in text]
        self.assertEqual(stale, [], "allowed phrases without a subject")


if __name__ == "__main__":
    unittest.main()
