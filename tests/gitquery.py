# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""What this copy of the repository can be asked about itself, and when it cannot.

Two fences ask git what is tracked here, and both had the same hole: a
missing git REPOSITORY makes the command answer with a return code, which
was handled, while a missing git PROGRAM makes it RAISE, which was not. The
second came back as an ERROR and looked like a broken witness instead of an
unanswerable question.

That is not a corner: the service runs its own suite inside an image that
carries the sources and no git at all. There the two fences reported
failures that were really abstentions, and a suite whose failures are
declared normal is a licence to look away.

Both callers now go through here, so the next one cannot get it wrong on its
own. Skipping and not returning an empty list is the point: an empty answer
would claim that nothing is tracked, and that is a measurement nobody made.
"""
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


def tracked(pattern=None):
    """The paths git tracks here, or a skip when this copy cannot answer."""
    command = ["git", "ls-files"]
    if pattern is not None:
        command.append(pattern)
    try:
        out = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    except OSError:
        raise unittest.SkipTest("no git program here, so the repository cannot be asked")
    if out.returncode != 0:
        raise unittest.SkipTest("not a git checkout")
    return out.stdout.split()


def is_a_checkout():
    """Whether the question can be put at all, without asking it."""
    try:
        tracked()
    except unittest.SkipTest:
        return False
    return True
