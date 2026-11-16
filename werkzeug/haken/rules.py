#!/usr/bin/env python3
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
"""The rules every commit of this repository keeps, in one place.

`rules.py msg <file>` reads a commit message, `rules.py staged` the staged
change, never the working tree. Each prints what breaks a rule, one line
each, and exits 1; a clean change exits 0. A rule that cannot be checked
refuses with its reason: an unchecked commit is not a clean one.

English is an approximation: umlauts and a list of German function words,
two hits make a German line.
"""
import os
import re
import subprocess
import sys
import tomllib

AUTHORS = {"lzs <352619651+maer-lzs@users.noreply.github.com>"}
TEMP_ROOTS = ("/private/tmp/", "/tmp/", "/private/var/folders/", "/var/folders/", "/var/tmp/")

# Files whose added lines are not read for values, names, dashes and
# language, each with its reason.
CONTENT_EXEMPT = {
    "tests/role-names.txt": "it is the list of names",
    "tests/role-names-allowed.txt": "it holds the phrases in which a name is allowed",
    "werkzeug/haken/test-haken.sh": "the witnesses carry each forbidden form on purpose",
    "werkzeug/haken/rules.py": "it spells out the patterns it refuses",
}


def git(*args):
    out = subprocess.run(["git", *args], capture_output=True, text=True)
    return out.returncode, out.stdout


def top():
    rc, out = git("rev-parse", "--show-toplevel")
    if rc != 0:
        raise SystemExit("rules.py: not inside a repository")
    return out.strip()


def read_list(path, what):
    try:
        lines = open(path, encoding="utf-8").read().splitlines()
    except OSError:
        raise SystemExit(f"rules.py: the {what} is missing: {path}")
    entries = [line.split(" # ", 1)[0].strip() for line in lines
               if line.strip() and not line.startswith("#")]
    if not entries:
        raise SystemExit(f"rules.py: the {what} is empty: {path}")
    return entries


ROOT = top()
NAMES = read_list(os.path.join(ROOT, "tests", "role-names.txt"), "list of names")
ALLOWED_PHRASES = read_list(os.path.join(ROOT, "tests", "role-names-allowed.txt"), "list of allowed phrases")
ROLES = re.compile(r"\b(" + "|".join(map(re.escape, NAMES)) + r")\b", re.I)
TRAILER = re.compile(r"^(co-authored-by|generated with|signed-off-by: claude)", re.I | re.M)
CHANNEL_CODE = re.compile(r"(^|[^A-Za-z])[A-Z]{1,4}[0-9]{0,3}-[A-Za-z0-9]{1,6}(-[A-Za-z0-9]{1,6})*([^A-Za-z0-9]|$)")
HASH = re.compile(r"\b(?=[0-9a-f]*[0-9])(?=[0-9a-f]*[a-f])[0-9a-f]{7,40}\b")
DECISION = re.compile(r"\bentscheid", re.I)
DASH = re.compile(r"[—–]")
GERMAN_WORDS = {
    "und", "nicht", "ist", "ein", "eine", "einen", "einem", "einer", "mit", "für", "fuer", "auf", "von",
    "zu", "dem", "des", "wird", "werden", "sind", "noch", "nur", "schon", "auch", "oder", "aber", "wenn",
    "weil", "dass", "zum", "zur", "bei", "nach", "aus", "wie", "sich", "mehr", "kein", "keine", "jetzt",
    "hier", "dort", "immer", "der", "das", "zeile", "sitzung", "steht", "geht", "bleibt",
}
WORD = re.compile(r"[A-Za-zÄÖÜäöüß]+")
UMLAUT = re.compile(r"[ÄÖÜäöüß]")
QUOTED = re.compile(r"\"[^\"]*\"|„[^“]*“|“[^”]*”")
HEADER = [
    re.compile(r"^(//!?|#) Mär — an audio player for self-hosted open ecosystems$"),
    re.compile(r"^(//!?|#) Copyright \(C\) \d{4} .+$"),
    re.compile(r"^(//!?|#) SPDX-License-Identifier: .+$"),
    re.compile(r"^(//!?|#) Additional terms apply — see LICENSE-ADDITIONAL-TERMS$"),
]
ACCESS = [
    re.compile(r"(?i)(token|secret|password|passwd|api[_-]?key|apikey|key)\w*\s*(:\s*&?'?\w*\s*)?[:=]\s*[\"'][^\"'\s]{16,}[\"']"),
    re.compile(r"[\"'][0-9A-Fa-f]{32,}[\"']"),
    re.compile(r"(?i)authorization:\s*(token|bearer|basic)\s+\S{8,}"),
]


def without_allowed(text):
    for phrase in ALLOWED_PHRASES:
        text = text.replace(phrase, " ")
    return text


def is_german(text):
    hits = 0
    for word in WORD.findall(QUOTED.sub(" ", text)):
        lower = word.lower()
        if lower == "mär":
            continue
        if lower in GERMAN_WORDS or UMLAUT.search(word):
            hits += 1
    return hits >= 2


def check_message(text):
    body = "\n".join(line for line in text.split("\n") if not line.startswith("#"))
    found = []
    if ROLES.search(without_allowed(body)):
        found.append("the message names a role or a person")
    if TRAILER.search(body):
        found.append("the message carries an attribution trailer")
    if CHANNEL_CODE.search(body) or HASH.search(body):
        found.append("the message carries a channel code or a hash")
    if DECISION.search(body):
        found.append("the message cites a decision")
    if DASH.search(body):
        found.append("the message carries a dash")
    if any(is_german(line) for line in body.split("\n")):
        found.append("the message is not English")
    return found


def added_lines():
    rc, diff = git("diff", "--cached", "-U0", "--no-color", "--no-ext-diff")
    if rc != 0:
        raise SystemExit("rules.py: the staged change cannot be read")
    path = None
    for line in diff.split("\n"):
        if line.startswith("+++ "):
            path = line[6:] if line.startswith("+++ b/") else None
        elif line.startswith("+") and path is not None:
            yield path, line[1:]


def comment_of(path, line):
    stripped = line.strip()
    if path.endswith(".rs") and stripped.startswith(("//", "/*", "*")):
        return stripped
    if (path.endswith((".py", ".sh", ".toml")) or os.path.basename(path) in ("Dockerfile",)) \
            and stripped.startswith("#") and not stripped.startswith("#!"):
        return stripped
    return None


def staged_file(path):
    rc, out = git("show", f":{path}")
    return out if rc == 0 else None


def crate_names(lock_text, toml_text):
    names = set()
    if lock_text:
        names |= set(re.findall(r'^name = "([^"]+)"', lock_text, re.M))
    if toml_text:
        data = tomllib.loads(toml_text)
        for table in ("dependencies", "dev-dependencies", "build-dependencies"):
            names |= set(data.get(table, {}).keys())
        own = data.get("package", {}).get("name")
        names.discard(own)
    return names


def check_staged():
    found = []
    rc, ident = git("var", "GIT_AUTHOR_IDENT")
    author = re.sub(r"\s+\d+\s+[+-]\d{4}\s*$", "", ident.strip())
    if author not in AUTHORS:
        found.append(f"the author is not the agreed one: {author}")
    real = os.path.realpath(ROOT) + "/"
    if real.startswith(TEMP_ROOTS) or "/scratchpad/" in real:
        if git("cat-file", "-e", ":.wegwerf")[0] != 0:
            found.append(f"the working tree lies in a temporary path without .wegwerf: {ROOT}")
    files = set()
    for path, line in added_lines():
        files.add(path)
        if path in CONTENT_EXEMPT:
            continue
        where = f"{path}: {line.strip()[:80]}"
        if any(pattern.search(line) for pattern in ACCESS):
            found.append(f"an access value: {where}")
        if path.endswith(".md") and DASH.search(line):
            found.append(f"a dash in a new markdown line: {where}")
        comment = comment_of(path, line)
        if comment is not None and not any(h.match(comment) for h in HEADER):
            if ROLES.search(without_allowed(comment)):
                found.append(f"a comment names a role or a person: {where}")
            if DASH.search(comment):
                found.append(f"a comment carries a dash: {where}")
            if is_german(comment):
                found.append(f"a comment is not English: {where}")
    if files & {"Cargo.lock", "Cargo.toml"}:
        approved = set(read_list(os.path.join(ROOT, "werkzeug", "haken", "crates.txt"), "list of crates"))
        unknown = crate_names(staged_file("Cargo.lock"), staged_file("Cargo.toml")) - approved
        for name in sorted(unknown):
            found.append(f"a crate without approval: {name}")
    return found


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "msg":
        found = check_message(open(sys.argv[2], encoding="utf-8").read())
    elif len(sys.argv) >= 2 and sys.argv[1] == "staged":
        found = check_staged()
    else:
        print("usage: rules.py msg <file> | rules.py staged", file=sys.stderr)
        return 2
    for item in found:
        print(f"refused: {item}", file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
