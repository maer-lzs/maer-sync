"""The fence on what must never be committed here.

It measures the TRACKED FILES, not the intention written in `.gitignore`.
An ignore file is a statement about the future; the list of tracked files
is what actually happened, and the two came apart on the very first branch
of this repository: no ignore file existed and 25 compiled files rode along
before anyone looked.
"""
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


def tracked():
    out = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True)
    if out.returncode != 0:
        raise unittest.SkipTest("not a git checkout")
    return out.stdout.split()


class RepoHygieneTests(unittest.TestCase):
    def test_no_store_is_tracked(self):
        # The one that is not merely untidy. The store holds scrypt hashes
        # of account secrets and digests of live tokens; committing one
        # publishes them for as long as the history lives.
        offenders = [p for p in tracked() if p.endswith((".sqlite", ".sqlite-wal", ".sqlite-shm"))]
        self.assertEqual(offenders, [], f"a store is tracked: {offenders}")

    def test_no_bytecode_is_tracked(self):
        offenders = [p for p in tracked() if p.endswith(".pyc") or "__pycache__" in p]
        self.assertEqual(offenders, [], f"compiled files are tracked: {offenders}")

    def test_the_ignore_file_carries_the_patterns(self):
        # The intention BESIDE the measurement, not instead of it: without
        # it every fresh run puts the files back and the test above turns
        # into a chore instead of a gate.
        text = (ROOT / ".gitignore").read_text(encoding="utf-8")
        for pattern in ("*.sqlite", "__pycache__/", "*.pyc", ".venv/"):
            self.assertIn(pattern, text, f".gitignore does not hold {pattern}")


if __name__ == "__main__":
    unittest.main()
