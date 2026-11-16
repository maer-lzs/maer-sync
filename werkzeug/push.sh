#!/bin/bash
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# push.sh <branch>: the one command for every push. Measures the commit the
# branch names now, runs the Rust witnesses in the pinned image, the fences
# of the repository and the witnesses of the hooks against a clean checkout
# of exactly that commit, writes the note of the run, and pushes that commit
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
log="$(mktemp)"
trap 'rm -f "$log"' EXIT

if [ -n "${PUSH_RUN:-}" ]; then
    bash -c "$PUSH_RUN" > "$log" 2>&1; rc=$?
    cases="$(sed -n 's/^cases=\([0-9][0-9]*\)$/\1/p' "$log" | tail -1)"
else
    run="$root/ziel/push-run"
    git worktree remove --force "$run" >/dev/null 2>&1
    git worktree add -q --detach "$run" "$sha" || { echo "push.sh: no clean checkout" >&2; exit 1; }
    rc=0
    ( cd "$run" && werkzeug/bau.sh test --locked ) > "$log" 2>&1 || rc=1
    rust="$(grep -E '^test result: ok\.' "$log" | sed -E 's/.* ok\. ([0-9]+) passed.*/\1/' | awk '{s+=$1} END {if (NR) print s}')"
    grep -qE '^test result: FAILED' "$log" && rc=1
    ( cd "$run" && python3 -m unittest discover -s tests -t . ) >> "$log" 2>&1 || rc=1
    fences="$(sed -n 's/^Ran \([0-9][0-9]*\) tests\{0,1\} in .*/\1/p' "$log" | tail -1)"
    ( cd "$run" && bash werkzeug/haken/test-haken.sh ) >> "$log" 2>&1 || rc=1
    hooks="$(sed -n 's/^cases: \([0-9][0-9]*\), failures: 0$/\1/p' "$log" | tail -1)"
    git worktree remove --force "$run" >/dev/null 2>&1
    if [ -n "${rust:-}" ] && [ -n "${fences:-}" ] && [ -n "${hooks:-}" ]; then
        cases=$((rust + fences + hooks))
    else
        cases=""; rc=1
    fi
fi
tail -5 "$log"
if [ "$rc" -ne 0 ] || [ -z "${cases:-}" ] || [ "$cases" -eq 0 ]; then
    echo "push.sh: the run is not green at $(git rev-parse --short "$sha") (rc $rc, cases ${cases:-none}); nothing is pushed" >&2
    exit 1
fi
git notes --ref=lauf add -f -m "kind=final result=green cases=$cases tree=$tree" "$sha" || exit 1
[ -n "${PUSH_AFTER_RUN:-}" ] && bash -c "$PUSH_AFTER_RUN"
LAUF_PUSH=1 git push origin "$sha:refs/heads/$branch" refs/notes/lauf || exit 1
remote="$(git ls-remote origin "refs/heads/$branch" | cut -f1)"
[ "$remote" = "$sha" ] || { echo "push.sh: origin holds $remote for $branch, not $sha" >&2; exit 1; }
echo "pushed $(git rev-parse --short "$sha") to $branch with $cases cases"
