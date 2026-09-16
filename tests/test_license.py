# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""That the licence is here, and that every file says which one it is.

This repository was split out of the app's and arrived without a licence at
all, while twenty-two of its files carried a header naming one. Nobody
noticed, because nothing measured it: the app has a fence for this and the
service did not, which is exactly why the two could drift apart.

Three things are asserted, and the order matters. The licence file exists and
is not empty. Every source file names a copyright and an SPDX identifier. And
the check read something at all — a fence over an empty list reports success,
which is the failure mode that hides every other one.

**What is deliberately NOT asserted yet:** the fourth header line points at
`LICENSE-ADDITIONAL-TERMS`, and that document's shape is still being decided.
Asserting it here would promise something about a file whose form is not
settled — a fence pointing at a moving target, which is the same fault as a
header pointing at a missing file, only from the other side. The line goes in
when the shape is decided.

Exemptions carry their reason at the entry, not in a list somewhere else. An
exemption whose reason has to be reconstructed is an exemption nobody reviews.
"""
import pathlib
import unittest

from tests import gitquery

ROOT = pathlib.Path(__file__).resolve().parent.parent

#: The licence file itself. It must exist and carry text: a repository whose
#: headers name a licence, with no licence text in it, says "all rights
#: reserved" to anyone reading it as law, and the opposite in every header.
LICENCE_FILE = "LICENSE"

#: Where source lives. Named positively, like the contract fence next door: a
#: negative list would quietly read `.venv` once somebody installs one.
SOURCE_DIRS = ("core", "service", "tests")

#: Files at the root that are source too, each named on purpose.
ROOT_FILES = ("Dockerfile", "compose.yaml", "run-tests.sh")

#: Exempt, each with the reason at the entry — not in a list elsewhere.
EXEMPT = {
    "requirements.txt": "a pinned dependency list, not our source",
    ".gitignore": "a list of patterns, and comments in it would be read as patterns",
    "README.md": "prose about the service; the LICENSE file covers the repository",
    "CONTRACT.md": "byte-identical in both repositories with a fence on each side, "
                   "so a header here is a change to the app's repository too",
}

SPDX = "SPDX-License-Identifier"
COPYRIGHT = "Copyright (C)"
HEAD_LINES = 10


def _sources():
    """Every file this repository calls its own, measured rather than listed."""
    found = []
    for directory in SOURCE_DIRS:
        d = ROOT / directory
        if not d.is_dir():
            continue
        found.extend(sorted(p for p in d.rglob("*.py") if "__pycache__" not in p.parts))
    for name in ROOT_FILES:
        p = ROOT / name
        if p.is_file():
            found.append(p)
    return sorted(set(found))


class TheLicenceIsHere(unittest.TestCase):

    def test_the_licence_file_exists_and_is_not_empty(self):
        p = ROOT / LICENCE_FILE
        self.assertTrue(p.is_file(), f"{LICENCE_FILE} is missing, while every header names a licence")
        # Size, not readability: a file truncated to nothing is perfectly
        # readable and says nothing.
        self.assertGreater(p.stat().st_size, 0, f"{LICENCE_FILE} exists but is empty")

    def test_the_licence_is_the_one_the_headers_name(self):
        # The headers say GPL-3.0-or-later. A LICENSE file carrying some other
        # licence would leave every header lying, and nothing else would notice.
        text = (ROOT / LICENCE_FILE).read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", text)
        self.assertIn("Version 3", text)


class EveryFileNamesItsLicence(unittest.TestCase):

    def test_something_was_read_at_all(self):
        # First, and on its own. A fence over an empty list passes every other
        # assertion in this file while measuring nothing.
        self.assertGreater(len(_sources()), 0,
                           "the header check found no files — it measured nothing, "
                           "so its silence is not a result")

    def test_every_source_file_carries_a_header(self):
        ohne = []
        for p in _sources():
            rel = p.relative_to(ROOT).as_posix()
            if rel in EXEMPT:
                continue
            # Unreadable is a finding, not a skip: the app's version of this
            # fence swallowed the exception and counted the file as fine.
            try:
                kopf = p.read_text(encoding="utf-8").splitlines()[:HEAD_LINES]
            except OSError as fehler:
                ohne.append(f"{rel}: could not be read ({fehler.strerror})")
                continue
            if not any(SPDX in zeile for zeile in kopf):
                ohne.append(f"{rel}: no {SPDX} in its first {HEAD_LINES} lines")
            elif not any(COPYRIGHT in zeile for zeile in kopf):
                ohne.append(f"{rel}: names a licence but no copyright holder")
        self.assertEqual([], ohne, "files with an incomplete licence header: " + "; ".join(ohne))

    def test_every_exemption_names_a_file_that_is_here(self):
        # The other direction, and the quiet one: an exemption for a file that
        # no longer exists stops exempting anything and starts being a note
        # nobody can check. It also hides a rename.
        #
        # Only a working copy can be asked this. An image carries the sources
        # without the repository's own files — `.gitignore` is not there, and
        # its absence says nothing about the exemption. Asking anyway turned
        # this witness red inside the image on its first run: a question this
        # copy cannot put, coming back as a failure instead of an abstention.
        # That is the fault this very repository fixed two commits ago, rebuilt
        # here by the person who found it.
        if not gitquery.is_a_checkout():
            raise unittest.SkipTest("not a checkout, so the repository's own files are not here")
        verschwunden = sorted(name for name in EXEMPT if not (ROOT / name).exists())
        self.assertEqual([], verschwunden,
                         "exempted files that are not here any more: " + ", ".join(verschwunden))

    def test_every_exemption_carries_a_reason(self):
        ohne_grund = sorted(name for name, grund in EXEMPT.items() if not grund.strip())
        self.assertEqual([], ohne_grund,
                         "exemptions without a reason at the entry: " + ", ".join(ohne_grund))
