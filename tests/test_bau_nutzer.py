# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The build container runs as the caller, not as root.

On 25.11.2026 `werkzeug/bau.sh generate-lockfile` left `abruf/Cargo.lock`
owned by root in a checkout of another user, and the next copy onto it was
refused. A file of another owner in a checkout makes the next tool blind.
"""

import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


class BuildRunsAsCaller(unittest.TestCase):
    def test_the_container_is_started_with_the_callers_ids(self):
        text = (ROOT / "werkzeug" / "bau.sh").read_text(encoding="utf-8")
        run = text[text.index("exec docker run"):]
        self.assertIn('--user "$(id -u):$(id -g)"', run)


if __name__ == "__main__":
    unittest.main()
