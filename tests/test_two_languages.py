# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
"""That the documentation stands in English and German, page for page.

The README and every page under docs/ exist twice, `name.md` and
`name.de.md`, with the same outline: the same headings at the same levels in
the same order. A section added to one language and not to the other is the
fault this fence is for; it would not show anywhere else, because each page
reads well on its own.

The house rule for published prose rides along: no dash as punctuation in
these pages, neither the long nor the short one.
"""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent

DASHES = ("—", "–")


def _outline(path):
    """The heading levels and nothing else, outside fenced and indented code."""
    levels, fenced = [], False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("```"):
            fenced = not fenced
            continue
        if fenced or line.startswith("    ") or line.startswith("\t"):
            continue
        m = re.match(r"(#{1,6}) \S", line)
        if m:
            levels.append(len(m.group(1)))
    return levels


def _pairs():
    pairs = [(ROOT / "README.md", ROOT / "README.de.md")]
    docs = ROOT / "docs"
    if docs.is_dir():
        for en in sorted(docs.glob("*.md")):
            if not en.name.endswith(".de.md"):
                pairs.append((en, en.with_name(en.stem + ".de.md")))
    return pairs


class TwoLanguages(unittest.TestCase):

    def test_the_readme_points_to_the_pages_in_docs(self):
        docs = ROOT / "docs"
        self.assertTrue(docs.is_dir(), "the technical pages belong in docs/")
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        for en in sorted(docs.glob("*.md")):
            if not en.name.endswith(".de.md"):
                self.assertIn(f"docs/{en.name}", readme, f"the README does not lead to docs/{en.name}")

    def test_every_page_has_its_german_twin_with_the_same_outline(self):
        for en, de in _pairs():
            with self.subTest(page=en.name):
                self.assertTrue(de.is_file(), f"{de.relative_to(ROOT)} is missing")
                self.assertEqual(_outline(en), _outline(de),
                                 f"{en.relative_to(ROOT)} and {de.relative_to(ROOT)} differ in their headings")

    def test_every_german_page_has_its_english_original(self):
        docs = ROOT / "docs"
        for de in sorted(docs.glob("*.de.md")) if docs.is_dir() else []:
            with self.subTest(page=de.name):
                self.assertTrue(de.with_name(de.name[:-len(".de.md")] + ".md").is_file(),
                                f"{de.relative_to(ROOT)} has no English page")

    def test_no_dash_as_punctuation(self):
        for en, de in _pairs():
            for page in (en, de):
                if not page.is_file():
                    continue
                for n, line in enumerate(page.read_text(encoding="utf-8").splitlines(), 1):
                    with self.subTest(page=page.name, line=n):
                        self.assertFalse(any(d in line for d in DASHES),
                                         f"{page.relative_to(ROOT)}:{n} carries a dash")


if __name__ == "__main__":
    unittest.main()
