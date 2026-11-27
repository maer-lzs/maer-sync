# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The gate before a commit says green only for a measured run.

`werkzeug/vor-dem-commit.sh` runs the witnesses of two packages, the service
and the fetch program. It counted the result lines of both together: when
one package did not compile, its line was missing, the other one counted,
and the gate said green. Each case here runs a copy of the gate in a
throwaway checkout, with a stand-in for `werkzeug/bau.sh` that answers per
package.
"""
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent

STAND_IN = """#!/bin/sh
case "$*" in *--manifest-path*) r="$GATE_FETCH" ;; *) r="$GATE_SERVICE" ;; esac
case "$r" in
  ok*) echo "test result: ok. ${r#ok } passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"; exit 0 ;;
  broken) echo "error[E0425]: cannot find value"; exit 101 ;;
  silent) exit 0 ;;
esac
exit 2
"""

ONE_FENCE = """import unittest


class One(unittest.TestCase):
    def test_one(self):
        pass
"""


def run_gate(service, fetch):
    with tempfile.TemporaryDirectory() as d:
        d = pathlib.Path(d)
        (d / "werkzeug").mkdir()
        shutil.copy(ROOT / "werkzeug" / "vor-dem-commit.sh", d / "werkzeug" / "vor-dem-commit.sh")
        (d / "werkzeug" / "bau.sh").write_text(STAND_IN, encoding="utf-8")
        (d / "werkzeug" / "bau.sh").chmod(0o755)
        (d / "tests").mkdir()
        (d / "tests" / "__init__.py").write_text("", encoding="utf-8")
        (d / "tests" / "test_one.py").write_text(ONE_FENCE, encoding="utf-8")
        for f in ("Cargo.toml", "Cargo.lock"):
            (d / f).write_text("", encoding="utf-8")
        subprocess.run(["git", "init", "-q", str(d)], check=True)
        env = dict(os.environ, GATE_SERVICE=service, GATE_FETCH=fetch)
        p = subprocess.run(["sh", str(d / "werkzeug" / "vor-dem-commit.sh")], cwd=d, env=env,
                           capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr


class GateTests(unittest.TestCase):
    def test_a_measured_run_passes(self):
        # First, and it has to pass: otherwise every case below fails for
        # the wrong reason.
        rc, out = run_gate("ok 3", "ok 2")
        self.assertEqual(rc, 0, out)
        self.assertIn("grün: 5", out)

    def test_a_service_that_does_not_compile_stops_the_gate(self):
        rc, out = run_gate("broken", "ok 2")
        self.assertEqual(rc, 1, out)

    def test_a_fetch_program_that_does_not_compile_stops_the_gate(self):
        rc, out = run_gate("ok 3", "broken")
        self.assertEqual(rc, 1, out)

    def test_a_package_without_a_result_line_stops_the_gate(self):
        rc, out = run_gate("silent", "ok 2")
        self.assertEqual(rc, 1, out)


if __name__ == "__main__":
    unittest.main()
