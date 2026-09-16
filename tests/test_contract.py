# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The fence on the contract, service side.

It reads CONTRACT.md and holds THIS repository's code against it. It cannot
see the app repository, and says so rather than pretending: what it buys is
that a wire change touching only this side goes red here at the next run,
instead of going quiet until a device stops syncing.
"""
import pathlib
import re
import tempfile
import unittest

from tests import gitquery

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "CONTRACT.md"


def declared():
    """The machine readable block of the contract, as name to values."""
    text = CONTRACT.read_text(encoding="utf-8")
    block = re.search(r"<!-- maer-sync-contract\n(.*?)-->", text, re.S)
    assert block, "the contract has no machine readable block"
    out = {}
    for line in block.group(1).strip().splitlines():
        name, _, values = line.partition(":")
        out[name.strip()] = values.split()
    return out


#: The service's own sources, named POSITIVELY.
#:
#: This used to read everything under the root except `tests`, and that was
#: a list of what had occurred to somebody. The first thing to occur to
#: nobody was `.venv`: the setup guide creates one inside the clone in step
#: 3, `uvicorn` brings `click` along, `click` has a field called `choices`,
#: and `choices` is a kind in the contract. From then on the suite was red
#: for everyone who followed the guide, and it pointed at the service.
#:
#: An exclusion list would have fixed that one name and waited for the next:
#: a build directory, a cache, a vendored file. Each lands in the fence
#: silently, with no error and no signal. Naming what the service IS cannot
#: go quiet that way; a new source directory is an addition somebody has to
#: make here on purpose.
SERVICE_DIRS = ("core", "service")


#: The root is a parameter so the rule can be asked somewhere else.
#:
#: Every real caller leaves it alone and reads this repository. The one
#: witness that asks what this function does with a file BESIDE the named
#: directories used to answer that by writing into the source tree, and a
#: hardened image does not allow it: the service runs as an unprivileged
#: user under a root owned by somebody else, and the question came back as
#: an ERROR instead of an answer.
#:
#: Widening the thing under test so it can be tested is worth naming, and
#: it is the shape the app side already uses for the same reason (a runner
#: takes its carrier, defaulted to the real one). The witness pays for it
#: by asserting that its built tree is read at all, so a `sources` that
#: ignored the root could not pass.
def sources(root=ROOT):
    return "\n".join(p.read_text(encoding="utf-8")
                     for directory in SERVICE_DIRS
                     for p in sorted((root / directory).rglob("*.py")))


#: Where the repository itself keeps Python, as top level names.
#:
#: The list above is the only thing that decides what the fence reads, and
#: nothing made it stay complete: a module in a third directory would drop
#: out of every check here without a word. So this asks the repository what
#: it carries instead of holding the list against a second hand copy of
#: itself. Untracked things cannot answer, which is why a `.venv` does not
#: show up here even though it sits in the clone.
#:
#: Where there is no repository to ask, there is no answer, and the witness
#: says so rather than passing. It stays a fence ABOVE the fence for that
#: reason: `sources()` must keep working from a plain unpacked copy. Both
#: ways of being unable to answer live in `gitquery`, because handling only
#: one of them was the first thing this witness got wrong.
def tracked_python_roots():
    return {pathlib.Path(line).parts[0] for line in gitquery.tracked("*.py")}


class ContractTests(unittest.TestCase):
    def test_the_contract_exists_and_names_its_version(self):
        self.assertTrue(CONTRACT.exists(), "CONTRACT.md is the whole point of two repositories")
        self.assertEqual(declared()["version"], ["2"])
        # The version stands twice on purpose: once for a reader, once for
        # this fence. Whoever bumps one and not the other trips here.
        self.assertIn("**Version: 2**", CONTRACT.read_text(encoding="utf-8"))

    def test_every_declared_path_exists_in_this_service(self):
        text = sources()
        for path in declared()["paths"]:
            self.assertIn(path, text, f"the contract promises {path} and the service has no such path")

    def test_every_path_this_service_answers_is_in_the_contract(self):
        """The other direction, and it was missing.

        The case above asks whether every promise is kept. Nothing asked the
        reverse: a path added to the service with no line in the contract
        stayed GREEN here, and the contract quietly stopped describing the
        service. That is the same one way fence that let a music provider
        door and a stock count through on the app side, twice in a week.

        It bites immediately: A4 wants a path that hands out the service's
        static public key, and without this case it could have landed with
        the contract still saying five paths.

        **Two shapes, because the service builds its paths two ways**, and a
        scan for whole literals would have found only the first:

        - compared whole, `path == "/v1/health"` in the application, and
          `scope["path"] != "/v1/changes"` for the socket, which is the same
          comparison against a differently named left side
        - taken apart, `parts[1] != "doc"` in `core/paths.py`, where the
          version is a constant and never appears in the literal

        **The first pattern here was too narrow and found four of the five**,
        because it insisted the left side be spelled `path`. The socket
        compares `scope["path"]`. So the comparison is matched on its RIGHT
        side, which is the part that is actually a path, and the count below
        is what caught it: a scan that finds fewer paths than the contract
        declares has a hole, and saying "nothing undeclared" from inside a
        hole is not a measurement.

        **The limit, said rather than papered over:** a path built a THIRD
        way is not seen here. The two assertions below are what keeps that
        from being silent, and the failure text names the shapes so whoever
        adds one gets told what this looks for.

        **And the other edge, for whoever is puzzled by a red one day:** this
        reads TEXT, so a comparison written inside a comment counts as a
        path. That is the safe direction to be wrong in, because it asks for
        a line in the contract instead of letting a real route pass, but it
        does mean an example in a docstring has to be spelled without the
        comparison.
        """
        text = sources()
        whole = set(re.findall(r'!=\s*"(/v1/[^"]*)"|==\s*"(/v1/[^"]*)"', text))
        whole = {a or b for a, b in whole}
        taken_apart = {f"/v1/{name}" for name in re.findall(r'parts\[1\]\s*!=\s*"(\w+)"', text)}
        answered = whole | taken_apart

        # The hole detector, and it is not decoration: it found the socket
        # path missing from the first version of this case. A scan that sees
        # fewer paths than the contract names cannot honestly report that
        # nothing is undeclared.
        missed = sorted(set(declared()["paths"]) - answered)
        self.assertEqual([], missed,
                         "this scan does not see paths the contract names, so its silence "
                         "about undeclared ones means nothing: " + ", ".join(missed))

        # Rule 7's second half: a scan that finds nothing passes every
        # assertion after it while measuring nothing.
        self.assertGreater(len(answered), 0,
                           "no path was found in the service at all, so this case measured "
                           "nothing. It looks for `path == \"/v1/...\"` and for "
                           "`parts[1] != \"...\"`; a path built another way needs a third "
                           "pattern here.")

        undeclared = sorted(answered - set(declared()["paths"]))
        self.assertEqual([], undeclared,
                         "paths the service answers that the contract does not name: "
                         + ", ".join(undeclared)
                         + ". Adding one is a contract change, so it raises the version in "
                           "BOTH repositories.")

    def test_every_declared_header_is_read_here(self):
        text = sources().lower()
        for header in declared()["headers"]:
            self.assertIn(header.lower(), text,
                          f"the contract promises the header {header} and nothing here reads it")

    def test_every_declared_status_is_answered_here(self):
        text = sources()
        for status in declared()["status"]:
            self.assertRegex(text, rf"\b{status}\b",
                             f"the contract promises {status} and this service never answers it")

    def test_the_socket_message_is_the_one_in_the_contract(self):
        # The shape was agreed by two implementations happening to match,
        # never written down, and a renamed field would have broken nothing
        # loudly: the client would read nothing, fall back to polling, and
        # the takeover would only get slower. Written down now, and held.
        text = sources()
        for field in ('"kind"', '"scope"', '"version"'):
            self.assertIn(field, text,
                          f"the contract promises the socket field {field}")
        self.assertIn('{"kind": "<kind>", "scope": "<scope>", "version": "<etag>"}',
                      CONTRACT.read_text(encoding="utf-8"))

    def test_a_file_outside_the_service_cannot_redden_the_fence(self):
        # Its OWN question and not the neighbour's: whether a stray file is
        # read at all. Asserting the kinds here too would make it tear
        # whenever the test beside it tears, and a green run would then say
        # nothing about what was actually asked.
        #
        # Asked on a tree this witness builds, not on the checkout. It used
        # to write its probe into the source root, which the service's own
        # hardened image forbids, and there the question came back as an
        # error rather than an answer. A built tree is also the sharper
        # place to ask: nothing here depends on what the repository happens
        # to contain today.
        with tempfile.TemporaryDirectory() as raw:
            root = pathlib.Path(raw)
            for directory in SERVICE_DIRS:
                (root / directory).mkdir()
            service_marker = "the_service_marker_4b1c"
            (root / SERVICE_DIRS[0] / "part.py").write_text(f"{service_marker} = 1\n",
                                                            encoding="utf-8")
            stray = root / ".not-the-service-probe"
            stray.mkdir()
            marker = "not_the_service_marker_9f3a"
            (stray / "intruder.py").write_text(f"{marker} = 1\n", encoding="utf-8")

            text = sources(root)
            # The precondition, and without it the rest measures nothing: a
            # reader that returned "" for this tree would pass the real
            # assertion below while reading no service at all.
            self.assertIn(service_marker, text,
                          "the built service was not read, so the question was never put")
            self.assertNotIn(marker, text,
                             "a file outside core/ and service/ reached the fence")

    def test_the_positive_list_still_names_the_whole_service(self):
        # The other direction of the same fix, and the quiet one. Naming the
        # service positively cannot go red on a stray file any more, but it
        # can go SILENT: a new source directory would simply never be read,
        # and every check here would stay green while measuring less.
        #
        # Both ways round on purpose. A name in the list with no tracked
        # source is the same hazard from the other end, because `rglob` on a
        # directory that is not there returns nothing without complaining.
        known = set(SERVICE_DIRS) | {"tests"}
        tracked = tracked_python_roots()
        named_but_empty = sorted(known - tracked)
        tracked_but_unfenced = sorted(tracked - known)
        self.assertEqual((named_but_empty, tracked_but_unfenced), ([], []),
                         "the fence and the repository have drifted: "
                         f"named here with no tracked source {named_but_empty}, "
                         f"tracked as source and read by nothing {tracked_but_unfenced}")

    def test_the_kinds_are_NOT_known_to_the_service(self):
        # The counter direction, and it is the one that matters: the service
        # checks the FORM of a kind and never a list of names. A fence that
        # only checked presence would happily accept a service that had
        # learned the six names, which is exactly the coupling this whole
        # split is meant to avoid.
        text = sources()
        for kind in declared()["kinds"]:
            self.assertNotIn(f'"{kind}"', text,
                             f"the service names the kind {kind}; it must not know any")


if __name__ == "__main__":
    unittest.main()
