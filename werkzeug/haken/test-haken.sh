#!/bin/bash
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# test-haken.sh: the witnesses of the commit and push hooks. Builds a
# throwaway repository with a bare origin, wires the hooks of this tree into
# it, and for every rule makes one case the hooks must refuse and one they
# must let through. Prints one line per case; exits non-zero when any case
# goes the wrong way or could not be run.
set -u
HAKEN="$(cd "$(dirname "$0")" && pwd)"
SOURCE="$(cd "$HAKEN/../.." && pwd)"
PUSH="$SOURCE/werkzeug/push.sh"
GIT="$(command -v git)" || { echo "not measured: no git program"; exit 2; }
command -v python3 >/dev/null || { echo "not measured: no python3"; exit 2; }
AUTHOR_NAME=lzs
AUTHOR_MAIL=352619651+maer-lzs@users.noreply.github.com

# A name the hooks refuse, read from the one list.
ROLE="$(grep -v '^#' "$SOURCE/tests/role-names.txt" | grep -m1 .)" || { echo "not measured: no list of names"; exit 2; }
ROLE_CAPITAL="$(echo "${ROLE:0:1}" | tr '[:lower:]' '[:upper:]')${ROLE:1}"
ROLE_UPPER="$(echo "$ROLE" | tr '[:lower:]' '[:upper:]')"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/haken-zeugen.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
ORIGIN="$WORK/origin.git"
REPO="$WORK/repo"
"$GIT" init -q --bare "$ORIGIN"
"$GIT" init -q -b dev "$REPO"
cd "$REPO" || exit 2
"$GIT" config user.name "$AUTHOR_NAME"
"$GIT" config user.email "$AUTHOR_MAIL"
"$GIT" remote add origin "$ORIGIN"
"$GIT" remote add elsewhere "$ORIGIN"
# The witness repository lies in a temporary path on purpose and carries the
# throwaway mark, so only the rule about such trees decides there.
echo "witness repository of the hooks" > .wegwerf
mkdir -p tests werkzeug/haken src docs
cp "$SOURCE/tests/role-names.txt" "$SOURCE/tests/role-names-allowed.txt" tests/
cp "$HAKEN/crates.txt" werkzeug/haken/
printf '[[package]]\nname = "serde"\nversion = "1.0.0"\n' > Cargo.lock
echo "base" > docs/base.md
# Build output and the log of a failed run stay out of the tree, as in this repository.
printf 'ziel/\n' > .gitignore
"$GIT" add -A && "$GIT" commit -q -m "Base of the witness repository" --no-verify
"$GIT" push -q origin dev --no-verify
"$GIT" branch -q main && "$GIT" push -q origin main --no-verify
"$GIT" config core.hooksPath "$HAKEN"

failures=0
cases=0
verdict() {
    local want="$1" got="$2" name="$3"
    cases=$((cases + 1))
    if [ "$got" = "$want" ]; then
        echo "ok   $want  $name"
    else
        echo "FAIL want $want, got $got: $name"; sed 's/^/     /' "$WORK/err" 2>/dev/null | head -3
        failures=$((failures + 1))
    fi
}

# expect <refuse|pass> <name> <file> <content> <message>: append, stage, commit.
expect() {
    local want="$1" name="$2" file="$3" content="$4" message="$5" got
    mkdir -p "$(dirname "$file")"
    printf '%s\n' "$content" >> "$file"
    "$GIT" add "$file"
    if "$GIT" commit -q -m "$message" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
    verdict "$want" "$got" "$name"
    "$GIT" reset -q --hard origin/dev
}

# --- The commit message
expect refuse "M1 message names a role"            docs/a.md "one" "The $ROLE session adds a line"
expect refuse "M2 message names a role in capitals" docs/a.md "two" "The $ROLE_UPPER review adds a line"
expect refuse "M2 message names a role inside a compound" docs/a.md "three" "A $ROLE-built row"
expect refuse "M3 message carries a trailer"        docs/b.md "one" $'Adds a line\n\nCo-Authored-By: Someone <a@b.c>'
expect refuse "M3 message says generated with"      docs/b.md "two" $'Adds a line\n\nGenerated with a tool'
expect refuse "M4 message names a channel code"     docs/b.md "three" "Builds K-EQ-13 in the sheet"
expect refuse "M4 message names a hash"             docs/b.md "four" "Follows the order as in a2caecfd"
expect refuse "M5 message cites a decision"         docs/c.md "one" "Entscheid A is built"
expect refuse "M4 message names a work-order code"  docs/b.md "five" "Builds the reader of K3a-4"
expect refuse "M4 message names a short order code" docs/b.md "six" "Takes in K3c and its fences"
expect refuse "M5 message says decided"             docs/c.md "seven" "As decided, the catalog keeps version 6"
expect refuse "M5 message names a decision"         docs/c.md "eight" "Keeps version 6 after the decision of the operator"
expect refuse "M4 message names a code in brackets"  docs/b.md "seven" "Reads the licence as the app does (D4)"
expect refuse "M4 message names a decision code"    docs/b.md "eight" "Keeps the rule of D4 in the reader"
expect pass   "M8 message names protocols and hashes" docs/c.md "ten" "Speaks H2 and IPv6, reads UTF-8, hashes with SHA256"
expect pass   "M8 message names a file format"      docs/c.md "nine" "Reads MP3 and FLAC titles, decides the order"
expect refuse "M6 message carries a dash"           docs/c.md "two" "The store — one file"
expect refuse "M7 message is German"                docs/c.md "three" "Die Sitzung wird nicht mehr gehalten"
expect pass   "M8 English message"                  docs/c.md "four" "Keeps the session for one hour"
expect pass   "M8 message names the app with its umlaut" docs/c.md "five" "Mär keeps the store"
expect pass   "M8 message quotes a German sentence" docs/c.md "six" 'The reply said "Die Sitzung ist abgelaufen" before'

# --- The index
"$GIT" config user.email someone@example.com
expect refuse "I1 another author"                   docs/d.md "one" "A line by someone else"
"$GIT" config user.email lzs@noreply.codeberg.org
expect refuse "I1 the former address of the author" docs/d.md "two" "A line by the former address"
"$GIT" config user.email "$AUTHOR_MAIL"
expect pass   "I2 the agreed author"                docs/d.md "three" "A line by the agreed author"
expect refuse "I3 a token in a Rust line"           src/a.rs 'let token = "abcdef0123456789abcdef0123";' "A constant in the source"
expect refuse "I3 a hex key in a Rust constant"     src/b.rs 'const KEY: &str = "0123456789abcdef0123456789abcdef";' "A constant in the source"
expect pass   "I3 the digest line of the contract" src/contract.rs 'pub const DIGEST: &str = "109620ba364d1ff50d8e121c7da6c97516e71e97d5ffdf98e8ad6c1e8c5d54f7";' "The contract moves its digest"
expect refuse "I3 the digest line in another file" src/vertrag.rs 'pub const DIGEST: &str = "109620ba364d1ff50d8e121c7da6c97516e71e97d5ffdf98e8ad6c1e8c5d54f7";' "A digest elsewhere"
expect refuse "I3 another constant of the contract file" src/contract.rs 'pub const ANDERS: &str = "109620ba364d1ff50d8e121c7da6c97516e71e97d5ffdf98e8ad6c1e8c5d54f7";' "Another constant"
expect refuse "I3 the digest line with something behind it" src/contract.rs 'pub const DIGEST: &str = "109620ba364d1ff50d8e121c7da6c97516e71e97d5ffdf98e8ad6c1e8c5d54f7"; const KEY: &str = "0123456789abcdef0123456789abcdef";' "The digest and more"
expect refuse "I3 the digest line with a short digest" src/contract.rs 'pub const DIGEST: &str = "109620ba364d1ff50d8e121c7da6c97516e71e97";' "A digest of another length"
expect pass   "I3 a checksum line of the lock file" Cargo.lock 'checksum = "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1e1418fb6c9dc5184b7e84c99"' "A crate is taken in"
expect pass   "I3 a checksum line of the lock file of the fetch program" abruf/Cargo.lock 'checksum = "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1e1418fb6c9dc5184b7e84c99"' "A crate is taken in"
expect refuse "I3 a checksum line in another file" src/lock.rs 'checksum = "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1e1418fb6c9dc5184b7e84c99"' "A checksum elsewhere"
expect refuse "I3 a checksum line with something behind it" Cargo.lock 'checksum = "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1e1418fb6c9dc5184b7e84c99" # "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1e1418fb6c9dc5184b7e84c99"' "A checksum and more"
expect refuse "I3 a checksum of another length" Cargo.lock 'checksum = "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1"' "A checksum of another length"
expect refuse "I3 another key of the lock file with a hash" Cargo.lock 'secret = "1973cfbc1a2daf9cf550e74e1f088c28e7f7d8c1e1418fb6c9dc5184b7e84c99"' "Another key"
expect refuse "I3 an authorization value"           docs/e.md "Authorization: Bearer abcdef0123456789" "A line in the docs"
expect pass   "I4 the name of a key without its value" src/c.rs 'let token_path = std::env::var("MAER_SYNC_KEY");' "A path in the source"
expect refuse "I5 German line comment"              src/d.rs "// Die Sitzung wird nicht mehr gehalten" "A comment in the source"
expect refuse "I5 German doc comment"               src/e.rs "/// Die Sitzung wird nicht mehr gehalten" "A comment in the source"
expect refuse "I5 German module comment"            src/f.rs "//! Die Sitzung wird nicht mehr gehalten" "A comment in the source"
expect pass   "I6 English comment"                  src/g.rs "// The session ends after one hour" "A comment in the source"
expect refuse "I7 comment names a role"             src/h.rs "// $ROLE_CAPITAL asked for this" "A comment in the source"
expect refuse "I8 comment carries a dash"           src/i.rs "// The store — one file" "A comment in the source"
# The header is read from a file of this tree, so the witness carries no name.
expect pass   "I9 licence header of a new file"     src/j.rs "$(head -4 "$SOURCE/src/main.rs")" "A new file with its header"
expect refuse "I10 new markdown line with a dash"  docs/f.md "The store – one file" "A line in the docs"
expect pass   "I10 new markdown line without a dash" docs/f.md "The store, one file" "A line in the docs"
expect pass   "I11 German text in a string literal" src/k.rs 'const REPLY: &str = "Die Sitzung ist abgelaufen";' "A test vector in the source"

# I18: paths with a space or an umlaut are read like any other path.
expect refuse "I18 German comment in a path with a space"  "src/a b.rs" "// Die Sitzung wird nicht mehr gehalten" "A comment in the source"
expect refuse "I18 German comment in a path with an umlaut" "src/märchen.rs" "// Die Sitzung wird nicht mehr gehalten" "A comment in the source"
expect refuse "I18 access value in a path with a space"   "src/c d.rs" 'let token = "abcdef0123456789abcdef0123";' "A constant in the source"
expect pass   "I18 English comment in a path with a space" "src/e f.rs" "// The session ends after one hour" "A comment in the source"

# I19: a pure rename carries no added line; a rename with a new line is read.
mkdir -p src && printf '// Die alte Zeile bleibt stehen\nfn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n' > src/alt.rs
"$GIT" add src/alt.rs && "$GIT" commit -q -m "An older file" --no-verify
"$GIT" mv src/alt.rs src/neu.rs
if "$GIT" commit -q -m "Moves the older file" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict pass "$got" "I19 pure rename of a file with an older German comment"
printf '// Die neue Zeile wird nicht mehr gezeigt\n' >> src/neu.rs && "$GIT" add src/neu.rs
"$GIT" mv src/neu.rs src/weiter.rs
if "$GIT" commit -q -m "Moves the file again and adds a line" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "I19 rename with a new German comment"
"$GIT" reset -q --hard origin/dev

# lever <status letter> <name>: the staged change really has that status, so a
# case below cannot pass or fail for a change that never happened.
lever() {
    local got
    got="$("$GIT" diff --cached --name-status -M | cut -c1)"
    [ "$got" = "$1" ] || { echo "not measured: $2 staged as '$got', not $1"; failures=$((failures + 1)); }
}

# I20: a file renamed to another ending is read as a new file, since its
# lines meet the rules of the new kind for the first time.
mkdir -p src && printf '// Die Zeile ist hier nur Text\nfn a() {}\n' > src/daten.txt
"$GIT" add src/daten.txt && "$GIT" commit -q -m "A text file" --no-verify
"$GIT" mv src/daten.txt src/geheim.rs
lever R "I20 rename to a new ending"
if "$GIT" commit -q -m "Turns the text into source" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "I20 rename to a new ending carries an old German line into a comment"
"$GIT" reset -q --hard origin/dev
mkdir -p src && printf '// Die Zeile ist hier nur Text\nfn a() {}\n' > src/daten.txt
"$GIT" add src/daten.txt && "$GIT" commit -q -m "A text file" --no-verify
"$GIT" mv src/daten.txt src/notiz.txt
lever R "I20 rename with the same ending"
if "$GIT" commit -q -m "Moves the text file" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict pass "$got" "I20 rename with the same ending stays a rename"
"$GIT" reset -q --hard origin/dev

# I21: a file that becomes a symbolic link is read like any other change.
mkdir -p docs && printf 'plain\n' > docs/wechsel.md
"$GIT" add docs/wechsel.md && "$GIT" commit -q -m "A plain page" --no-verify
rm docs/wechsel.md && ln -s 'The store – one file' docs/wechsel.md && "$GIT" add docs/wechsel.md
lever T "I21 type change to a link with a dash"
if "$GIT" commit -q -m "Turns the page into a link" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "I21 type change to a link whose target carries a dash"
"$GIT" reset -q --hard origin/dev
mkdir -p docs && printf 'plain\n' > docs/wechsel.md
"$GIT" add docs/wechsel.md && "$GIT" commit -q -m "A plain page" --no-verify
rm docs/wechsel.md && ln -s base.md docs/wechsel.md && "$GIT" add docs/wechsel.md
lever T "I21 type change to a plain link"
if "$GIT" commit -q -m "Turns the page into a link" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict pass "$got" "I21 type change to a plain link"
"$GIT" reset -q --hard origin/dev

# I12: a tree in a temporary path without the mark is refused.
"$GIT" rm -q .wegwerf && "$GIT" commit -q -m "Drop the mark" --no-verify
expect refuse "I12 tree in a temporary path without the mark" docs/g.md "one" "A line in a temporary tree"
"$GIT" reset -q --hard origin/dev

# I13 and I14: what goes out is the index, not the working tree.
printf 'A clean line\n' > docs/h.md && "$GIT" add docs/h.md
printf 'let token = "abcdef0123456789abcdef0123";\n' >> docs/h.md
if "$GIT" commit -q -m "A clean line in the index" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict pass "$got" "I13 clean index, access value only in the working tree"
"$GIT" reset -q --hard origin/dev
mkdir -p src && printf 'let token = "abcdef0123456789abcdef0123";\n' > src/l.rs && "$GIT" add src/l.rs || { verdict refuse not-measured "I14 could not stage the file"; }
printf '// A clean line\n' > src/l.rs
if "$GIT" commit -q -m "A line in the source" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "I14 access value in the index, cleaned in the working tree"
"$GIT" reset -q --hard origin/dev

# I15 and I16: the crates.
printf '\n[[package]]\nname = "some-unlisted-crate"\nversion = "0.1.0"\n' >> Cargo.lock
"$GIT" add Cargo.lock
if "$GIT" commit -q -m "Adds a dependency" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "I15 a crate that is not on the list"
"$GIT" reset -q --hard origin/dev
# Edited through a copy: in-place editing in sed takes different arguments on BSD and GNU.
sed 's/version = "1.0.0"/version = "1.0.1"/' Cargo.lock > Cargo.lock.new && mv Cargo.lock.new Cargo.lock && "$GIT" add Cargo.lock
if "$GIT" commit -q -m "Moves a dependency to its next version" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict pass "$got" "I16 a listed crate in another version"
"$GIT" reset -q --hard origin/dev

# I17: an existing German comment stays, another line of the file changes.
mkdir -p src && printf '// Die alte Zeile bleibt stehen\nfn a() {}\n' > src/m.rs
"$GIT" add src/m.rs && "$GIT" commit -q -m "An older file" --no-verify
sed 's/fn a() {}/fn a() -> u8 { 1 }/' src/m.rs > src/m.rs.new && mv src/m.rs.new src/m.rs && "$GIT" add src/m.rs
if "$GIT" commit -q -m "Returns one" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict pass "$got" "I17 an unchanged older comment does not refuse"
"$GIT" reset -q --hard origin/dev

# --- The push
# The run is replaced in the witnesses: PUSH_RUN prints the count of cases
# and ends with the result of the run.
green='echo cases=5; exit 0'
"$GIT" commit -q --allow-empty -m "A commit to push"
plain_push() {
    if "$GIT" push -q "$@" >/dev/null 2>"$WORK/err"; then echo pass; else echo refuse; fi
}
verdict refuse "$(plain_push origin dev)" "P1 bare push without the script"
verdict refuse "$(LAUF_PUSH=1 plain_push elsewhere dev)" "P2 push to another remote"
verdict refuse "$(LAUF_PUSH=1 plain_push origin HEAD:refs/heads/main)" "P3 push to main"
"$GIT" tag -a -m "A tag" v9.9.9
verdict refuse "$(LAUF_PUSH=1 plain_push origin v9.9.9)" "P4 push of a tag"
verdict refuse "$(LAUF_PUSH=1 plain_push origin :refs/heads/main)" "P5 deletion of a branch"
"$GIT" push -q origin dev --no-verify
"$GIT" reset -q --hard HEAD~1
"$GIT" commit -q --allow-empty -m "Another history" --no-verify
verdict refuse "$(LAUF_PUSH=1 plain_push --force origin HEAD:refs/heads/dev)" "P6 push that is no fast forward"
"$GIT" fetch -q origin
"$GIT" reset -q --hard origin/dev
"$GIT" commit -q --allow-empty -m "A commit to push"

run_push() {
    if PUSH_RUN="$1" "$PUSH" dev >/dev/null 2>"$WORK/err"; then echo pass; else echo refuse; fi
}
verdict pass   "$(run_push "$green")" "P7 push of dev after a green run"
"$GIT" fetch -q origin
[ "$("$GIT" rev-parse origin/dev 2>/dev/null)" = "$("$GIT" rev-parse dev)" ] && got=pass || got=refuse
verdict pass "$got" "P7 the remote holds the measured commit"
"$GIT" commit -q --allow-empty -m "Another commit to push"
verdict refuse "$(LAUF_PUSH=1 plain_push origin dev)" "P8 push without the note of a run"
verdict refuse "$(run_push 'echo cases=5; exit 1')" "P8 push after a red run"
verdict refuse "$(run_push 'echo cases=0; exit 0')" "P8 push after a run with no cases"
"$GIT" notes --ref=lauf add -f -m "kind=final result=green cases=5 tree=0000000000000000000000000000000000000000" HEAD
verdict refuse "$(LAUF_PUSH=1 plain_push origin dev)" "P8 note for another tree"
"$GIT" reset -q --hard origin/dev
printf 'one\n' > docs/i.md && "$GIT" add docs/i.md && "$GIT" commit -q -m "Die Zeile wird nicht mehr gezeigt" --no-verify
verdict refuse "$(run_push "$green")" "P9 a commit in the range breaks the rules of the message"
"$GIT" reset -q --hard origin/dev

# P10: the branch moves while the hook runs; the hook compares at the end.
"$GIT" commit -q --allow-empty -m "The measured commit"
old="$("$GIT" rev-parse HEAD)"
"$GIT" notes --ref=lauf add -f -m "kind=final result=green cases=5 tree=$("$GIT" rev-parse "$old^{tree}")" "$old"
"$GIT" commit -q --allow-empty -m "A commit after the measurement"
if printf 'refs/heads/dev %s refs/heads/dev %s\n' "$old" "$("$GIT" rev-parse origin/dev)" \
    | LAUF_PUSH=1 "$HAKEN/pre-push" origin "$ORIGIN" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "P10 the local ref no longer points at the pushed commit"
"$GIT" reset -q --hard origin/dev

# P11: the script pushes the measured commit, not what the branch holds later.
"$GIT" commit -q --allow-empty -m "The commit the run measures"
measured="$("$GIT" rev-parse HEAD)"
if PUSH_RUN="$green" PUSH_AFTER_RUN='git commit -q --allow-empty -m "A late commit" --no-verify' \
    "$PUSH" dev >/dev/null 2>"$WORK/err"; then :; fi
"$GIT" fetch -q origin
[ "$("$GIT" rev-parse origin/dev)" = "$measured" ] && got=pass || got=refuse
verdict pass "$got" "P11 the remote holds exactly the measured commit"
"$GIT" reset -q --hard origin/dev

# P13: the run measures the tree as it is committed, so a tree with changes
# that are not committed refuses before any run.
"$GIT" commit -q --allow-empty -m "A commit with a dirty tree behind it"
printf 'not committed\n' >> docs/base.md
verdict refuse "$(run_push "$green")" "P13 push from a tree with uncommitted changes"
printf 'untracked\n' > docs/untracked.md && "$GIT" checkout -q docs/base.md
verdict refuse "$(run_push "$green")" "P13 push from a tree with an untracked file"
rm -f docs/untracked.md
"$GIT" reset -q --hard origin/dev

# P12: the notes only move forward. Without notes on the remote there is
# nothing to move, and the case counts as not measured.
if "$GIT" ls-remote --exit-code origin refs/notes/lauf >/dev/null 2>&1; then :; else
    verdict refuse not-measured "P12 precondition: notes on the remote"
fi
verdict refuse "$(LAUF_PUSH=1 plain_push origin :refs/notes/lauf)" "P12 deletion of the notes"
verdict refuse "$(LAUF_PUSH=1 plain_push --force origin refs/notes/lauf~1:refs/notes/lauf)" "P12 notes moved back"

# --- What the hooks cannot know
mv tests/role-names.txt "$WORK/role-names.txt"
expect refuse "N1 the list of names is missing"     docs/j.md "one" "A line without the list"
mv "$WORK/role-names.txt" tests/role-names.txt
printf 'one\n' > docs/k.md && "$GIT" add docs/k.md
mkdir -p "$WORK/bin" && ln -sf "$GIT" "$WORK/bin/git"
if env PATH="$WORK/bin:/usr/lib/git-core" "$GIT" commit -q -m "A line without python" >/dev/null 2>"$WORK/err"; then got=pass; else got=refuse; fi
verdict refuse "$got" "N2 no python3 for the hooks"
"$GIT" reset -q --hard origin/dev
"$GIT" commit -q --allow-empty -m "A commit with a broken build"
verdict refuse "$(run_push 'echo build failed; exit 101')" "N3 the run did not build"
"$GIT" reset -q --hard origin/dev

echo "cases: $cases, failures: $failures"
[ "$cases" -gt 0 ] && [ "$failures" -eq 0 ]
