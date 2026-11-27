#!/bin/bash
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# test-server.sh: the witnesses of the gate on the server (server-tor.sh).
# Builds a bare repository that holds the gate as its pre-receive hook and
# the patterns beside it, as Forgejo will hold them, and pushes to it from a
# clone without hooks, as a foreign clone would. Each case names the reason
# it expects: a refusal for another reason counts as a failure. The last
# cases hold rules.py and the gate to the same verdicts on the same lines.
# Prints one line per case; exits non-zero when any case goes the wrong way
# or could not be run.
set -u
HAKEN="$(cd "$(dirname "$0")" && pwd)"
SOURCE="$(cd "$HAKEN/../.." && pwd)"
GIT="$(command -v git)" || { echo "not measured: no git program"; exit 2; }
command -v python3 >/dev/null || { echo "not measured: no python3"; exit 2; }
[ -s "$HAKEN/server-tor.sh" ] || { echo "not measured: no server-tor.sh"; exit 2; }
[ -s "$HAKEN/muster.txt" ] || { echo "not measured: no muster.txt"; exit 2; }
AUTHOR_NAME=lzs
AUTHOR_MAIL=352619651+maer-lzs@users.noreply.github.com

WORK="$(mktemp -d "${TMPDIR:-/tmp}/tor-zeugen.XXXXXX")" && [ -d "$WORK" ] \
    || { echo "not measured: no temporary directory"; exit 2; }
trap 'rm -rf "$WORK"' EXIT
ORIGIN="$WORK/origin.git"
REPO="$WORK/repo"
"$GIT" init -q --bare "$ORIGIN"
"$GIT" init -q -b dev "$REPO"
mkdir "$WORK/no-hooks"
cd "$REPO" || exit 2
"$GIT" config user.name "$AUTHOR_NAME"
"$GIT" config user.email "$AUTHOR_MAIL"
"$GIT" config core.hooksPath "$WORK/no-hooks"
"$GIT" remote add origin "$ORIGIN"
echo "witness repository of the server gate" > .wegwerf
mkdir -p tests werkzeug/haken src docs
cp "$SOURCE/tests/role-names.txt" "$SOURCE/tests/role-names-allowed.txt" tests/
cp "$HAKEN/crates.txt" "$HAKEN/muster.txt" werkzeug/haken/
echo "base" > docs/base.md
"$GIT" add -A && "$GIT" commit -q -m "Base of the witness repository"
# The base goes up before the gate is in place, and so does a branch that a
# case deletes later.
"$GIT" branch -q main && "$GIT" branch -q gone
"$GIT" push -q origin dev main gone || { echo "not measured: the base cannot be pushed"; exit 2; }
cp "$HAKEN/server-tor.sh" "$ORIGIN/hooks/pre-receive"
chmod 755 "$ORIGIN/hooks/pre-receive"
cp "$HAKEN/muster.txt" "$ORIGIN/hooks/pre-receive.muster"

# Values built at run time, so this file carries none of the forms it tests.
TOKVAL="$(printf 'q%.0s' $(seq 20))"
HEX40="$(printf 'e%.0s' $(seq 40))"
HEX64="$(printf 'd%.0s' $(seq 64))"
L_TOKEN="$(printf 'api_key = "%s"' "$TOKVAL")"
L_HEX="$(printf 'const X: &str = "%s";' "$HEX40")"
L_AUTH="$(printf '%s: Bearer %s' Authorization "$TOKVAL")"
L_DIGEST="$(printf 'pub const DIGEST: &str = "%s";' "$HEX64")"
L_LOCK="$(printf 'checksum = "%s"' "$HEX64")"
L_SHORT="$(printf 'token = "%s"' short)"
HEX64_UPPER="$(printf 'D%.0s' $(seq 64))"
L_DIGEST_UPPER="$(printf 'pub const DIGEST: &str = "%s";' "$HEX64_UPPER")"
L_LOCK_UPPER="$(printf 'checksum = "%s"' "$HEX64_UPPER")"

# What rules.py says of the head commit of the witness clone.
rules_verdict() {
    if python3 -I "$HAKEN/rules.py" commit HEAD > /dev/null 2>"$WORK/rules"; then echo accepted; else echo refused; fi
}

failures=0
cases=0
verdict() {
    local want="$1" got="$2" name="$3"
    cases=$((cases + 1))
    if [ "$got" = "$want" ]; then
        echo "ok   $want  $name"
    else
        echo "FAIL want $want, got $got: $name"; sed 's/^/     /' "$WORK/out" | head -4
        failures=$((failures + 1))
    fi
}

# What a push came to: accepted when the gate spoke and let it through,
# refused:<reason> when it refused, unknown otherwise (no word of the gate).
# The gate writes "tor: checked" when it lets a push through and
# "tor: refused: <reason>: <detail>" for each refusal.
push_result() {
    "$GIT" push --no-verify "$@" > "$WORK/out" 2>&1
    local rc=$?
    if [ $rc -eq 0 ] && grep -q "tor: checked" "$WORK/out"; then
        echo accepted
    elif [ $rc -ne 0 ] && grep -q "tor: refused" "$WORK/out"; then
        echo "refused:$(sed -n 's/.*tor: refused: \([a-z -]*\).*/\1/p' "$WORK/out" | head -1 | sed 's/ *$//')"
    else
        echo "unknown(rc $rc)"
    fi
}

note() {
    local result="$1" rev="${2:-HEAD}" tree="${3:-}"
    [ -n "$tree" ] || tree="$("$GIT" rev-parse "$rev^{tree}")"
    "$GIT" notes --ref=lauf add -f -m "kind=final result=$result cases=1 tree=$tree" "$rev"
}

add_line() {
    mkdir -p "$(dirname "$1")"
    printf '%s\n' "$2" >> "$1"
    "$GIT" add -A
    "$GIT" commit -q -m "${3:-Witness case}"
}

fresh() {
    "$GIT" fetch -q origin
    "$GIT" checkout -q -B "${1:-dev}" "origin/${1:-dev}"
}

# S1 first: a clean commit with the note of a green run goes through, and the
# gate said so. When it does not, every refusal below could be a broken gate.
fresh
add_line docs/clean.md "a clean line" "Add a clean line"
note green
verdict accepted "$(push_result origin dev refs/notes/lauf)" "S1 a clean commit with the note of a green run"
# ALL_CASES=1 runs the rest anyway, to show each case red against an empty
# gate (named limit, for the red commit only).
if [ "$failures" -ne 0 ] && [ "${ALL_CASES:-}" != 1 ]; then
    echo "the clean case does not pass the gate; the other cases are not run"
    echo "cases: $cases, failures: $failures"
    exit 1
fi

fresh; add_line docs/s2.txt "$L_TOKEN"; note green
verdict "refused:an access value" "$(push_result origin dev refs/notes/lauf)" "S2 an access value in an added line"

fresh; add_line src/contract.rs "$L_DIGEST" "Hold the contract to its digest"; note green
verdict accepted "$(push_result origin dev refs/notes/lauf)" "S3 the digest line in src/contract.rs"
fresh; add_line Cargo.lock "$L_LOCK" "Pin a crate"; note green
verdict accepted "$(push_result origin dev refs/notes/lauf)" "S3 a checksum line in Cargo.lock"

# The two exceptions are exact: the same lines in capitals are access values
# for the gate and for rules.py alike.
fresh; add_line src/contract.rs "$L_DIGEST_UPPER" "Hold the contract to its digest"; note green
verdict "refused:an access value" "$(push_result origin dev refs/notes/lauf)" "S3 the digest line in capitals"
verdict refused "$(rules_verdict)" "S3 rules.py refuses the digest line in capitals"
fresh; add_line Cargo.lock "$L_LOCK_UPPER" "Pin a crate"; note green
verdict "refused:an access value" "$(push_result origin dev refs/notes/lauf)" "S3 a checksum line in capitals"
verdict refused "$(rules_verdict)" "S3 rules.py refuses a checksum line in capitals"

fresh; add_line src/other.rs "$L_DIGEST"; note green
verdict "refused:an access value" "$(push_result origin dev refs/notes/lauf)" "S4 the digest line in another file"

fresh; add_line docs/s5.md "a line"
"$GIT" commit -q --amend --no-edit --author="somebody <somebody@example.org>"
note green
verdict "refused:the author" "$(push_result origin dev refs/notes/lauf)" "S5 another author"

fresh; add_line docs/s6.md "a line" "$(printf 'Add a line\n\nCo-Authored-By: Somebody <somebody@example.org>')"
note green
verdict "refused:a trailer" "$(push_result origin dev refs/notes/lauf)" "S6 an attribution trailer in the message"

fresh
verdict "refused:deletes dev" "$(push_result origin :dev)" "S7 the deletion of dev"
fresh; "$GIT" commit -q --amend -m "A rewritten head"; note green
verdict "refused:not a fast-forward" "$(push_result --force origin dev refs/notes/lauf)" "S7 a push to dev that is not a fast-forward"
verdict accepted "$(push_result origin :gone)" "S7 the deletion of another branch"
fresh main; add_line docs/release.md "a release line" "Cut a release"
verdict accepted "$(push_result origin main)" "S7 a clean commit on main without a note (the protection of the branch is a setting of the server)"
"$GIT" tag -a -m "A release" v0.0.1 main
verdict accepted "$(push_result origin v0.0.1)" "S7 a tag (the protection of tags is a setting of the server)"

fresh; add_line docs/s8.md "a line without a note"
verdict "refused:no note of a green run" "$(push_result origin dev)" "S8 a head without a note"
fresh; add_line docs/s8.md "a line with a red note"; note red
verdict "refused:no note of a green run" "$(push_result origin dev refs/notes/lauf)" "S8 a head with the note of a red run"
fresh; add_line docs/s8.md "a line whose note names another tree"; note green HEAD "$("$GIT" rev-parse HEAD^^{tree})"
verdict "refused:no note of a green run" "$(push_result origin dev refs/notes/lauf)" "S8 a head whose green note names another tree"

fresh
add_line docs/s9.md "first" "Add the first line"
add_line docs/s9.txt "$L_TOKEN" "Add the second line"
add_line docs/s9.md "third" "Add the third line"
note green
verdict "refused:an access value" "$(push_result origin dev refs/notes/lauf)" "S9 the middle of three commits breaks a rule"

# S10: rules.py and the gate read the same lines alike. Each line goes up on
# a branch of its own, with a green note, and rules.py reads the same commit.
same() {
    local path="$1" line="$2" name="$3" branch="case-$cases"
    fresh
    "$GIT" checkout -q -b "$branch"
    add_line "$path" "$line" "Add a witness line"
    note green
    local gate clone
    gate="$(push_result origin "$branch" refs/notes/lauf)"
    gate="${gate%%:*}"
    if python3 -I "$HAKEN/rules.py" commit HEAD > /dev/null 2>"$WORK/rules"; then clone=accepted; else clone=refused; fi
    verdict "$clone" "$gate" "S10 rules.py $clone, the gate alike: $name"
    "$GIT" checkout -q dev
}
same docs/a.txt "$L_TOKEN" "a named value in quotes"
same docs/b.rs "$L_HEX" "forty hex digits in quotes"
same docs/c.txt "$L_AUTH" "an authorization header"
same src/contract.rs "$L_DIGEST" "the digest line in its file"
same src/d.rs "$L_DIGEST" "the digest line elsewhere"
same Cargo.lock "$L_LOCK" "a checksum in the lock file"
same docs/e.toml "$L_LOCK" "a checksum elsewhere"
same docs/f.txt "$L_SHORT" "a short value"
same werkzeug/haken/test-haken.sh "$L_TOKEN" "a value in a file the patterns exempt"

# S11: both read their patterns from the one file. A line no pattern knows
# goes through both; once the file knows it, both refuse it.
LEVER="ZEUGE123456X"
same docs/g.txt "$LEVER" "a line no pattern knows"
echo "access zeuge[0-9]{6}x" >> werkzeug/haken/muster.txt
"$GIT" add -A && "$GIT" commit -q -m "Teach the patterns a witness form"
note green
"$GIT" push -q --no-verify origin dev refs/notes/lauf > "$WORK/out" 2>&1 \
    || { echo "not measured: the lever of S11 cannot be pushed"; sed 's/^/     /' "$WORK/out" | head -4; exit 2; }
echo "access zeuge[0-9]{6}x" >> "$ORIGIN/hooks/pre-receive.muster"
fresh; "$GIT" checkout -q -b lever
add_line docs/h.txt "$LEVER" "Add a witness line"
note green
gate="$(push_result origin lever refs/notes/lauf)"
if python3 -I "$HAKEN/rules.py" commit HEAD > /dev/null 2>"$WORK/rules"; then clone=accepted; else clone=refused; fi
verdict refused "$clone" "S11 rules.py reads the pattern the file was taught"
verdict "refused:an access value" "$gate" "S11 the gate reads the pattern the file was taught"

# S12: without whole patterns the gate lets nothing in and rules.py reads
# nothing, each with a sentence that says why. The same push goes through
# once the file is whole again, so the refusals came from the file.
INSTALLED="$ORIGIN/hooks/pre-receive.muster"
TRACKED="werkzeug/haken/muster.txt"
cp "$INSTALLED" "$WORK/installed.kept"
cp "$TRACKED" "$WORK/tracked.kept"
fresh; add_line docs/s12.md "a clean line" "Add a clean line"; note green
# rules.py refused for the named reason, or something else.
rules_reason() {
    if python3 -I "$HAKEN/rules.py" commit HEAD > /dev/null 2>"$WORK/out"; then echo accepted
    elif grep -q "$1" "$WORK/out"; then echo refused
    else echo "other"; fi
}
: > "$INSTALLED"
verdict "refused:no patterns" "$(push_result origin dev refs/notes/lauf)" "S12 the gate with an empty file of patterns"
rm -f "$INSTALLED"
verdict "refused:no patterns" "$(push_result origin dev refs/notes/lauf)" "S12 the gate without a file of patterns"
{ cat "$WORK/installed.kept"; echo "allow everything"; } > "$INSTALLED"
verdict "refused:no patterns" "$(push_result origin dev refs/notes/lauf)" "S12 the gate with a line of unknown kind"
: > "$TRACKED"
verdict refused "$(rules_reason "holds no access pattern")" "S12 rules.py with an empty file of patterns"
rm -f "$TRACKED"
verdict refused "$(rules_reason "list of patterns is missing")" "S12 rules.py without a file of patterns"
{ cat "$WORK/tracked.kept"; echo "allow everything"; } > "$TRACKED"
verdict refused "$(rules_reason "unknown kind")" "S12 rules.py with a line of unknown kind"
cp "$WORK/installed.kept" "$INSTALLED"
cp "$WORK/tracked.kept" "$TRACKED"
verdict accepted "$(push_result origin dev refs/notes/lauf)" "S12 the gate takes the same push once the file is whole"
verdict accepted "$(rules_verdict)" "S12 rules.py takes the same commit once the file is whole"

echo "cases: $cases, failures: $failures"
[ "$failures" -eq 0 ]
