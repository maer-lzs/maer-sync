#!/bin/bash
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# push.sh <branch>: the one command for every push. Measures the commit the
# branch names now, runs the Rust witnesses in the pinned image, the fences
# of the repository and the witnesses of the hooks and of the gate on the
# server in this checkout, which
# has to be exactly that commit, writes the note of the run, and pushes that commit
# by its hash, so a later commit on the branch cannot ride along.
# PUSH_RUN replaces the run and PUSH_AFTER_RUN runs a command after it; both
# exist for the witnesses of the hooks (named limit).
set -u
branch="${1:-}"
[ -n "$branch" ] || { echo "usage: werkzeug/push.sh <branch>" >&2; exit 2; }
[ "$branch" != main ] || { echo "push.sh: main moves only through a release" >&2; exit 1; }
root="$(git rev-parse --show-toplevel)" || exit 2
cd "$root" || exit 2
sha="$(git rev-parse --verify -q "refs/heads/$branch^{commit}")" || { echo "push.sh: no branch $branch" >&2; exit 1; }
[ "${#sha}" -eq 40 ] || { echo "push.sh: no full hash for $branch" >&2; exit 1; }
tree="$(git rev-parse "$sha^{tree}")"
# The run measures this checkout, so it has to be exactly the commit: HEAD
# at the branch, nothing changed, nothing untracked. Ignored build output
# under ziel/ stays as a cache.
[ "$(git rev-parse HEAD)" = "$sha" ] || { echo "push.sh: HEAD is not $branch; switch to it first" >&2; exit 1; }
[ -z "$(git status --porcelain --untracked-files=all)" ] \
    || { echo "push.sh: the tree has changes that are not committed; nothing is run or pushed" >&2; exit 1; }
# The hooks are a setting of the clone and do not travel with it: a clone
# without them pushes nothing, and every commit the push carries meets the
# rules of the hooks here, also one made where they were not set.
hooks="$(git config --get core.hooksPath)"
[ -n "$hooks" ] || { echo "push.sh: this clone has no hooks; set them: git config core.hooksPath werkzeug/haken" >&2; exit 1; }
case "$hooks" in /*) ;; *) hooks="$root/$hooks";; esac
[ -x "$hooks/pre-commit" ] && [ -x "$hooks/pre-push" ] && [ -f "$hooks/rules.py" ] \
    || { echo "push.sh: the hooks at $hooks are not whole" >&2; exit 1; }
at="$(git ls-remote origin "refs/heads/$branch" | cut -f1)"
if [ -n "$at" ] && git cat-file -e "$at^{commit}" 2>/dev/null; then range=("$at..$sha"); else range=("$sha" --not --remotes=origin); fi
for commit in $(git rev-list "${range[@]}"); do
    why="$(python3 -I "$hooks/rules.py" commit "$commit" 2>&1)" \
        || { echo "push.sh: commit $(git rev-parse --short "$commit") breaks the rules of the hooks: $why" >&2; exit 1; }
done
log="$(mktemp)"
trap 'rm -f "$log" "$log.gate"' EXIT

if [ -n "${PUSH_RUN:-}" ]; then
    bash -c "$PUSH_RUN" > "$log" 2>&1; rc=$?
    cases="$(sed -n 's/^cases=\([0-9][0-9]*\)$/\1/p' "$log" | tail -1)"
else
    run="$root"
    rc=0
    ( cd "$run" && werkzeug/bau.sh test --locked ) > "$log" 2>&1 || rc=1
    # The fetch program is a package of its own with its own lock file.
    ( cd "$run" && werkzeug/bau.sh test --locked --manifest-path abruf/Cargo.toml ) >> "$log" 2>&1 || rc=1
    rust="$(grep -E '^test result: ok\.' "$log" | sed -E 's/.* ok\. ([0-9]+) passed.*/\1/' | awk '{s+=$1} END {if (NR) print s}')"
    grep -qE '^test result: FAILED' "$log" && rc=1
    ( cd "$run" && python3 -m unittest discover -s tests -t . ) >> "$log" 2>&1 || rc=1
    fences="$(sed -n 's/^Ran \([0-9][0-9]*\) tests\{0,1\} in .*/\1/p' "$log" | tail -1)"
    ( cd "$run" && bash werkzeug/haken/test-haken.sh ) >> "$log" 2>&1 || rc=1
    hook_cases="$(sed -n 's/^cases: \([0-9][0-9]*\), failures: 0$/\1/p' "$log" | tail -1)"
    # The witnesses of the gate on the server end with a line of the same
    # form, so they are counted from their own log.
    ( cd "$run" && bash werkzeug/haken/test-server.sh ) > "$log.gate" 2>&1 || rc=1
    cat "$log.gate" >> "$log"
    gate_cases="$(sed -n 's/^cases: \([0-9][0-9]*\), failures: 0$/\1/p' "$log.gate" | tail -1)"
    if [ -n "${rust:-}" ] && [ -n "${fences:-}" ] && [ -n "${hook_cases:-}" ] && [ -n "${gate_cases:-}" ]; then
        cases=$((rust + fences + hook_cases + gate_cases))
    else
        cases=""; rc=1
    fi
fi
tail -5 "$log"
if [ "$rc" -ne 0 ] || [ -z "${cases:-}" ] || [ "$cases" -eq 0 ]; then
    mkdir -p "$root/ziel" && cp "$log" "$root/ziel/push-last-run.log"
    echo "push.sh: the full log of the run is in ziel/push-last-run.log" >&2
    echo "push.sh: the run is not green at $(git rev-parse --short "$sha") (rc $rc, cases ${cases:-none}); nothing is pushed" >&2
    exit 1
fi
git notes --ref=lauf add -f -m "kind=final result=green cases=$cases tree=$tree" "$sha" || exit 1
[ -n "${PUSH_AFTER_RUN:-}" ] && bash -c "$PUSH_AFTER_RUN"
LAUF_PUSH=1 git push origin "$sha:refs/heads/$branch" refs/notes/lauf || exit 1
remote="$(git ls-remote origin "refs/heads/$branch" | cut -f1)"
[ "$remote" = "$sha" ] || { echo "push.sh: origin holds $remote for $branch, not $sha" >&2; exit 1; }
echo "pushed $(git rev-parse --short "$sha") to $branch with $cases cases"
