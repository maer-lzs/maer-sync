# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""That the licence is here, and that every file says which one it is.

This repository was split out of the app's and arrived without a licence at
all, while twenty-two of its files carried a header naming one. Nobody
noticed, because nothing measured it: the app has a fence for this and the
service did not, which is exactly why the two could drift apart.

Every header makes three promises, and all three are checked here: a
copyright holder, an SPDX identifier, and additional terms in a file of that
name. A header promising terms that are not there is the fault this fence was
built for.

**Rule 7 runs through the whole file.** A copy that cannot answer a question
abstains with its reason; a copy that CAN answer and finds nothing goes red.
Both halves are needed: abstaining everywhere hides real faults, and asking
everywhere turns an image or an unpacked archive red for carrying what it was
meant to carry. The first version of this fence got that wrong in its own
first run, in the very class it was written against.

Exemptions carry their reason at the entry, not in a list somewhere else. An
exemption whose reason has to be reconstructed is an exemption nobody reviews.
"""
import pathlib
import unittest

from tests import gitquery

ROOT = pathlib.Path(__file__).resolve().parent.parent

#: Both licence files. A repository whose headers name a licence, with no
#: licence text in it, says "all rights reserved" to anyone reading it as law,
#: and the opposite in every header.
LICENCE_FILES = ("LICENSE", "LICENSE-ADDITIONAL-TERMS")

#: Where source lives. Named positively, like the contract fence next door: a
#: negative list would quietly read `.venv` once somebody installs one.
#: Seit dem Rust-Zweig liegt Quelltext auch in `src`, und der Zaun muss ihn
#: sehen: sonst faellt genau das Neue durch, und zwar still.
# `vektoren` ist dabei, seit die Erzeuger dort liegen: ein Erzeuger IST
# Quelltext, und eine Vektordatei ohne Erzeuger ist eine Behauptung. Die
# JSON-Dateien daneben werden nicht eingesammelt (die Sammlung nimmt nur
# .py, .rs, .sh und Dockerfile) und tragen ihre Ausnahme mit Grund.
# `core`, `service` und `abdeckung` gingen am 24.09.2026 mit der Python-Fassung.
# `abruf` since 25.11.2026: the fetch program of the catalog index, a package
# of its own so its crates never enter the tree of the service.
SOURCE_DIRS = ("tests", "src", "werkzeug", "vektoren", "abruf")

#: Files at the root that are source too, each named on purpose.
ROOT_FILES = ("Dockerfile", "compose.yaml", "Cargo.toml", "abruf/Cargo.toml")

#: Exempt, each with the reason at the entry — not in a list elsewhere.
EXEMPT = {
    "werkzeug/haken/commit-msg": "a git hook, whose name git fixes without a suffix; it carries the header, "
                                 "the collection only knows suffixes",
    "werkzeug/haken/pre-commit": "a git hook, whose name git fixes without a suffix; it carries the header, "
                                 "the collection only knows suffixes",
    "werkzeug/haken/pre-push": "a git hook, whose name git fixes without a suffix; it carries the header, "
                               "the collection only knows suffixes",
    "werkzeug/haken/crates.txt": "a list the hooks read; it carries the header as comment lines",
    "tests/role-names.txt": "a list the hooks and the name fence read; it carries the header as comment lines",
    "tests/role-names-allowed.txt": "a list the name fence reads; it carries the header as comment lines",
    ".gitignore": "a list of patterns, and comments in it would be read as patterns",
    "README.md": "prose about the service; the LICENSE file covers the repository",
    "CONTRACT.md": "byte-identical in both repositories with a fence on each side, "
                   "so a header here is a change to the app's repository too",
    "Cargo.lock": "cargo writes this file; a header would be rewritten away, and the "
                  "pins it holds are checked by the fence on Cargo.toml instead",
    "abruf/Cargo.lock": "cargo writes this file; a header would be rewritten away. It starts as a copy of "
                        "the service's Cargo.lock, so the shared crates have the same versions",
    "abruf/FREMDTEILE.md": "a table of the third-party crates of the fetch program; the LICENSE file covers it",
    "abruf/regale.json": "the theme shelves the fetch program reads, kept equal to the copy in the vectors; "
                         "JSON has no comment to put a header in",
    "LICENSE": "it is the licence",
    "LICENSE-ADDITIONAL-TERMS": "it is the additional terms",
    "vektoren/README.md": "prose about the vectors next to them; the LICENSE file covers it",
    "werkzeug/README.md": "prose about the build harness; the LICENSE file covers it",
    "vektoren/abdruecke.json": "numbers, and JSON has no comment to put a header in; "
                               "where they come from is stated in the README beside them",
    "vektoren/rahmen.json": "numbers, and JSON has no comment to put a header in; "
                            "where they come from is stated in the README beside them",
    "vektoren/schreiben.json": "numbers, and JSON has no comment to put a header in; "
                               "where they come from is stated in the README beside them",
    "vektoren/zeichen.json": "numbers, and JSON has no comment to put a header in; "
                             "where they come from is stated in the README beside them",
    "vektoren/sitzungen.json": "numbers, and JSON has no comment to put a header in; "
                               "where they come from is stated in the README beside them",
    "vektoren/rueckkehr.json": "numbers, and JSON has no comment to put a header in; "
                               "where they come from is stated in the README beside them",
    "vektoren/kopf-und-schluessel.json": "numbers, and JSON has no comment to put a header in; "
                                          "where they come from is stated in the README beside them",
    "vektoren/protokoll.json": "which line each refusal leaves, measured off the real "
                              "application, and JSON has no comment to put a header in; "
                              "its generator is stated in the README beside it",
    "vektoren/befehle.json": "exit codes and sentences of the operator commands, measured "
                            "off the real CLI, and JSON has no comment to put a header in; "
                            "its generator is stated in the README beside it",
    "vektoren/abweisung.json": "what a close code becomes on the wire, measured off a "
                              "running uvicorn, and JSON has no comment to put a header "
                              "in; its generator and its one condition are stated in the "
                              "README beside it",
    "vektoren/horcher.json": "close codes and deliveries measured off the running socket, "
                             "and JSON has no comment to put a header in; how it was "
                             "produced is stated in the README beside it",
    "vektoren/wege.json": "answers measured off the running application, and JSON has no "
                          "comment to put a header in; how it was produced, and what it "
                          "pins, is stated in the README beside it",
    "vektoren/faelle-v6.json": "expected answers of contract version 6, written from the contract text by a second author "
        "and not from this code; JSON has no comment to put a header in, the LICENSE file covers it",
    "vektoren/sign-in-v6.json": "the derivation vectors of contract version 6, delivered with the contract and computed twice "
        "independently, not measured off this service; JSON has no comment to put a header in, the LICENSE file covers it",
    "vektoren/noise-nk.json": "somebody else's published numbers, copied verbatim; a "
                              "header would alter the bytes whose whole value is that "
                              "nobody touched them, and JSON has no comments to put one in",
}

#: Exempt by folder, each with the reason at the entry: a folder of recorded
#: answers holds dozens of files of one kind, and listing each would hide the
#: one reason they share.
EXEMPT_UNDER = {
    "abruf/lizenzen/": "the licence files of the third-party crates of the fetch program, copied unchanged "
                       "from each published crate; a header would alter another author's licence text",
    "vektoren/katalog/": "answers of the catalog sources, recorded and cut to one record, and the rows "
                         "expected from them; JSON, RDF and CSV have no comment to put a header in, a "
                         "header would alter recorded bytes, and where they come from is stated in "
                         "vektoren/README.md and in each answer's _quelle",
}

SPDX = "SPDX-License-Identifier"
COPYRIGHT = "Copyright (C)"
TERMS = "LICENSE-ADDITIONAL-TERMS"
HEAD_LINES = 10


def _sources():
    """Every file this repository calls its own, measured rather than listed."""
    found = []
    for directory in SOURCE_DIRS:
        d = ROOT / directory
        if not d.is_dir():
            continue
        # `*.py` UND `*.rs`: die Rust-Seite traegt denselben Kopf in ihrer
        # Kommentarform. Wer hier nur eine Endung sammelt, prueft die andere
        # nie und merkt es nicht.
        # Endungen UND der Dockerfile-Name: wer hier nur sammelt, was er kennt,
        # prueft das Neue nie und merkt es nicht.
        for muster in ("*.py", "*.rs", "*.sh", "Dockerfile"):
            found.extend(sorted(p for p in d.rglob(muster)
                                if "__pycache__" not in p.parts and "ziel" not in p.parts))
    for name in ROOT_FILES:
        p = ROOT / name
        if p.is_file():
            found.append(p)
    return sorted(set(found))


def _claims_a_licence():
    """Whether this copy can be asked for a licence file at all.

    Rule 7, and the reason it is a function rather than an assumption: a
    checkout and an image both carry the sources AND the licence, so both can
    be asked. An unpacked fragment that carries neither cannot — and calling
    that a failure would make the fence red for a copy that never promised
    anything.

    The question is answered by the copy's OWN content, not by a proxy: if a
    file here carries a licence header, this copy makes a claim about a
    licence, and then the licence has to be here. Asking `README.md` instead
    would be a stand-in for the thing itself.
    """
    for p in _sources():
        try:
            kopf = p.read_text(encoding="utf-8").splitlines()[:HEAD_LINES]
        except OSError:
            continue
        if any(SPDX in zeile for zeile in kopf):
            return True
    return False


class TheLicenceIsHere(unittest.TestCase):

    def setUp(self):
        if not _claims_a_licence():
            raise unittest.SkipTest(
                "no file here carries a licence header, so this copy makes no "
                "claim a licence file could answer for")

    def test_the_licence_files_exist_and_are_not_empty(self):
        for name in LICENCE_FILES:
            p = ROOT / name
            self.assertTrue(p.is_file(), f"{name} is missing, while the headers here name it")
            # Size, not readability: a file truncated to nothing is perfectly
            # readable and says nothing.
            self.assertGreater(p.stat().st_size, 0, f"{name} exists but is empty")

    def test_the_licence_is_the_one_the_headers_name(self):
        # A LICENSE file carrying some other licence would leave every header
        # lying, and nothing else would notice.
        text = (ROOT / "LICENSE").read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", text)
        self.assertIn("Version 3", text)

    def test_the_additional_terms_are_terms_under_that_licence(self):
        # The fourth header line points here by name. If this file ever stops
        # being terms under the GPL, every header points at something else.
        text = (ROOT / TERMS).read_text(encoding="utf-8")
        self.assertIn("GNU GPL Version 3, Section 7", text)
        # Section 10 forbids further restrictions; the terms say so themselves,
        # and that sentence is what keeps them compatible.
        self.assertIn("may\nnot be removed", text)


class EveryFileNamesItsLicence(unittest.TestCase):

    def test_something_was_read_at_all(self):
        # Rule 7's second half, first and on its own. A fence over an empty
        # list passes every other assertion here while measuring nothing.
        self.assertGreater(len(_sources()), 0,
                           "the header check found no files — it measured nothing, "
                           "so its silence is not a result")

    def test_every_source_file_carries_a_complete_header(self):
        ohne = []
        for p in _sources():
            rel = p.relative_to(ROOT).as_posix()
            if rel in EXEMPT:
                continue
            # Unreadable is a finding, not a skip: this copy CAN read its own
            # files, so a failure to is a fault, not an unanswerable question.
            try:
                kopf = p.read_text(encoding="utf-8").splitlines()[:HEAD_LINES]
            except OSError as fehler:
                ohne.append(f"{rel}: could not be read ({fehler.strerror})")
                continue
            for was, marke in (("an SPDX identifier", SPDX),
                               ("a copyright holder", COPYRIGHT),
                               ("the additional terms", TERMS)):
                if not any(marke in zeile for zeile in kopf):
                    ohne.append(f"{rel}: its header is missing {was}")
        self.assertEqual([], ohne, "incomplete licence headers: " + "; ".join(ohne))

    def test_every_exemption_names_a_file_that_is_here(self):
        # The other direction, and the quiet one: an exemption for a file that
        # no longer exists stops exempting anything and starts being a note
        # nobody can check. It also hides a rename.
        #
        # Only a working copy can be asked. An image carries the sources
        # without the repository's own files — `.gitignore` is not there, and
        # its absence says nothing about the exemption.
        if not gitquery.is_a_checkout():
            raise unittest.SkipTest("not a checkout, so the repository's own files are not here")
        verschwunden = sorted(name for name in EXEMPT if not (ROOT / name).exists())
        self.assertEqual([], verschwunden,
                         "exempted files that are not here any more: " + ", ".join(verschwunden))

    def test_nothing_in_the_repository_falls_out_silently(self):
        """The hole this case closes, found by the reviewer and not by me.

        `_sources()` gathers `*.py` and three files at the root. Everything
        else inside a source directory is neither checked NOR exempt: it
        simply is not seen, so it carries no header and nobody says why.
        `tests/vectors/noise-nk.json` had been sitting in exactly that gap
        since it was added, and the fence stayed green over it.

        That is the fence's own doctrine turned against itself: an exemption
        carries its reason at the entry, and an unseen file is an exemption
        with no entry at all. So every tracked file in a source directory
        must be one or the other, and a new kind of file has to be decided
        about rather than slipping past.

        **The limit, and it is worth typing out because it surprised the
        reviewer:** this asks git what is TRACKED. A file that has merely
        been created is not seen at all, and a probe that only writes one
        stays green until it is added. That is the right boundary for a
        fence about what the repository publishes, and it means the case
        speaks at `git add` and not before.
        """
        if not gitquery.is_a_checkout():
            raise unittest.SkipTest("not a checkout, so what is tracked cannot be asked")
        gathered = {p.relative_to(ROOT).as_posix() for p in _sources()}
        # Ueber das GANZE Repo, nicht nur ueber die Quellverzeichnisse. Die
        # frueheste Fassung fragte nur innerhalb von SOURCE_DIRS, und genau
        # daneben ist der Rust-Zweig entstanden: Cargo.toml, Cargo.lock, zwei
        # Verzeichnisse und vier Vektordateien lagen ausserhalb und fielen
        # still durch, waehrend der Zaun gruen blieb. Dieselbe Klasse wie die,
        # gegen die er gebaut wurde, nur eine Ebene weiter aussen.
        unseen = sorted(
            rel for rel in gitquery.tracked()
            if rel not in gathered
            and rel not in EXEMPT
            and not any(rel.startswith(prefix) for prefix in EXEMPT_UNDER)
            and "__pycache__" not in rel)
        self.assertEqual([], unseen,
                         "tracked files in a source directory that are neither checked "
                         "nor exempt, so nothing says why they carry no header: "
                         + ", ".join(unseen))

    def test_every_exemption_carries_a_reason(self):
        ohne_grund = sorted(name for name, grund in {**EXEMPT, **EXEMPT_UNDER}.items() if not grund.strip())
        self.assertEqual([], ohne_grund,
                         "exemptions without a reason at the entry: " + ", ".join(ohne_grund))
