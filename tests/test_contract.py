"""The fence on the contract, service side.

It reads CONTRACT.md and holds THIS repository's code against it. It cannot
see the app repository, and says so rather than pretending: what it buys is
that a wire change touching only this side goes red here at the next run,
instead of going quiet until a device stops syncing.
"""
import pathlib
import re
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


def sources():
    return "\n".join(p.read_text(encoding="utf-8")
                     for directory in SERVICE_DIRS
                     for p in sorted((ROOT / directory).rglob("*.py")))


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
        stray = ROOT / ".not-the-service-probe"
        stray.mkdir(exist_ok=True)
        marker = "not_the_service_marker_9f3a"
        try:
            (stray / "intruder.py").write_text(f"{marker} = 1\n", encoding="utf-8")
            self.assertNotIn(marker, sources(),
                             "a file outside core/ and service/ reached the fence")
        finally:
            (stray / "intruder.py").unlink(missing_ok=True)
            stray.rmdir()

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
