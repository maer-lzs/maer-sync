# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""No vector carries the favourite list of an archive.org account.

The answers under `vektoren/katalog/` are real answers of archive.org, cut
to one title. Their `collection` field also named the favourite lists of
single accounts (`fav-` and the name of the account): names of people that
no reader needs and no expectation reads. The vectors keep the collections
of the source only.
"""
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


class VectorTests(unittest.TestCase):
    def test_no_vector_carries_a_favourite_list(self):
        files = sorted(p for p in (ROOT / "vektoren").rglob("*") if p.is_file())
        # A fence that reads nothing says nothing.
        self.assertGreater(len(files), 100, f"only {len(files)} vector files were read")
        found = [str(p.relative_to(ROOT)) for p in files
                 if '"fav-' in p.read_text(encoding="utf-8", errors="replace")]
        # The files only, never the names they carry.
        self.assertEqual(found, [], f"{len(found)} vector files name a favourite list")


if __name__ == "__main__":
    unittest.main()
