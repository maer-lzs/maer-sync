# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The fetch container is hardened as the service is.

Both containers of `compose.yaml` run code of this repository against
answers from outside; the fetch program reads answers of public sources.
The keys that keep a process from gaining privileges and from forking
without end stand at the service and stand at the fetch program too.
"""
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
KEYS = ("no-new-privileges:true", "pids_limit:")


def service_block(name):
    """The lines of one service, comments left out."""
    lines = (ROOT / "compose.yaml").read_text(encoding="utf-8").splitlines()
    start = lines.index(f"  {name}:")
    block = []
    for line in lines[start + 1:]:
        if line and not line.startswith("   ") and not line.lstrip().startswith("#"):
            break
        if not line.lstrip().startswith("#"):
            block.append(line)
    return "\n".join(block)


class ComposeTests(unittest.TestCase):
    def test_the_service_carries_the_keys(self):
        # First: without it the case below could pass on a reader that
        # finds nothing anywhere.
        block = service_block("maer-sync")
        for k in KEYS:
            self.assertIn(k, block)

    def test_the_fetch_container_is_hardened_as_the_service(self):
        block = service_block("maer-sync-abruf")
        self.assertIn("mem_limit:", block)
        missing = [k for k in KEYS if k not in block]
        self.assertEqual(missing, [])


if __name__ == "__main__":
    unittest.main()
