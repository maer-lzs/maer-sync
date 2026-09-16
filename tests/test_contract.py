"""The fence on the contract, service side.

It reads CONTRACT.md and holds THIS repository's code against it. It cannot
see the app repository, and says so rather than pretending: what it buys is
that a wire change touching only this side goes red here at the next run,
instead of going quiet until a device stops syncing.
"""
import pathlib
import re
import unittest

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


def sources():
    return "\n".join(p.read_text(encoding="utf-8")
                     for p in sorted(ROOT.rglob("*.py"))
                     if "tests" not in p.parts)


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
