#!/bin/bash
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# server-tor.sh: the gate of the server, installed in Forgejo as
# hooks/pre-receive.d/20-regeln of maer-sync.git with the patterns beside it
# as 20-regeln.muster (not executable, so Forgejo does not run it as a hook).
# It reads every push, whatever clone it comes from, and refuses it whole
# when one rule breaks:
#   - an access value in an added line of any new commit, after the patterns
#     of werkzeug/haken/muster.txt, read as rules.py reads them;
#   - an author other than the agreed one;
#   - an attribution trailer in a message;
#   - the deletion of dev, or a push to dev that is not a fast-forward;
#   - a head of a branch other than main without the note of a green run of
#     push.sh for exactly its tree, with at least one case (main and tags
#     are guarded by the protection rules of the server, which only the
#     owner may pass).
# It needs bash, git and the grep and awk of BusyBox, nothing else. It writes
# "tor: checked" when it lets a push through and one line
# "tor: refused: <reason>: <detail>" for each rule broken; a detail never
# repeats the line that broke the rule.
set -u
ZERO=0000000000000000000000000000000000000000
EMPTY_TREE=4b825dc642cb6eb9a060e54bf8d69288fbee4904
AUTHOR="lzs <352619651+maer-lzs@users.noreply.github.com>"
MUSTER="${TOR_MUSTER:-$0.muster}"
TMP="$(mktemp -d)" && [ -d "$TMP" ] || { echo "tor: refused: no temporary directory: nothing is checked" >&2; exit 1; }
trap 'rm -rf "$TMP"' EXIT
refused=0
refuse() {
    echo "tor: refused: $1: $2" >&2
    refused=1
}

# The patterns. A gate without them checks nothing, so it lets nothing in.
[ -s "$MUSTER" ] || { echo "tor: refused: no patterns: $MUSTER is missing" >&2; exit 1; }
: > "$TMP/access"; : > "$TMP/except"; : > "$TMP/exempt"
while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in ''|'#'*) continue ;; esac
    kind="${line%% *}"; rest="${line#* }"
    case "$kind" in
        access) printf '%s\n' "$rest" >> "$TMP/access" ;;
        except) printf '%s\n' "$rest" >> "$TMP/except" ;;
        exempt) printf '%s\n' "${rest%% *}" >> "$TMP/exempt" ;;
        *) echo "tor: refused: no patterns: a line of unknown kind in $MUSTER" >&2; exit 1 ;;
    esac
done < "$MUSTER"
[ -s "$TMP/access" ] || { echo "tor: refused: no patterns: $MUSTER holds no access pattern" >&2; exit 1; }

cat > "$TMP/updates"

# The notes of the runs: as this push leaves them, or as the server holds them.
notes=""
while read -r old new ref; do
    [ "$ref" = refs/notes/lauf ] && [ "$new" != "$ZERO" ] && notes="$new"
done < "$TMP/updates"
[ -n "$notes" ] || notes="$(git rev-parse -q --verify refs/notes/lauf)"
[ -n "$notes" ] && git ls-tree -r --name-only "$notes" > "$TMP/notes"

note_of() {
    [ -s "$TMP/notes" ] || return 1
    local path
    while IFS= read -r path; do
        if [ "${path//\//}" = "$1" ]; then
            git cat-file blob "$notes:$path"
            return
        fi
    done < "$TMP/notes"
    return 1
}

short() { git rev-parse --short "$1"; }

# The kind of a file as rules.py reads it: its ending, or its name for a
# Dockerfile.
kind_of() {
    local name="${1##*/}" bare
    [ "$name" = Dockerfile ] && { echo Dockerfile; return; }
    bare="${name#"${name%%[!.]*}"}"
    case "$bare" in *.*) echo ".${bare##*.}" ;; *) echo "" ;; esac
}

# The added lines of one path of a commit against its first parent; a renamed
# or copied file of the same kind is read against its source.
check_path() {
    local commit="$1" base="$2" path="$3" source="$4" paths entry pattern
    grep -qxF -e "$path" "$TMP/exempt" && return
    if [ -n "$source" ]; then set -- "$source" "$path"; else set -- "$path"; fi
    git diff -M -U0 --no-color --no-ext-diff "$base" "$commit" -- "$@" \
        | awk '/^@@/ {h = 1; next} h && /^\+/ {print substr($0, 2)}' > "$TMP/lines"
    while IFS= read -r entry; do
        paths="${entry%% *}"; pattern="${entry#* }"
        case ",$paths," in *",$path,"*)
            grep -vxE -e "$pattern" "$TMP/lines" > "$TMP/kept"
            mv "$TMP/kept" "$TMP/lines" ;;
        esac
    done < "$TMP/except"
    if grep -qiE -f "$TMP/access" "$TMP/lines"; then
        refuse "an access value" "$(short "$commit") $path"
    fi
}

check_commit() {
    local commit="$1" author base status path source
    author="$(git log -1 --format='%an <%ae>' "$commit")"
    [ "$author" = "$AUTHOR" ] || refuse "the author" "$(short "$commit") $author"
    if git log -1 --format=%B "$commit" | grep -qiE '^(co-authored-by|generated with|signed-off-by: claude)'; then
        refuse "a trailer" "$(short "$commit")"
    fi
    base="$(git rev-parse -q --verify "$commit^")" || base="$EMPTY_TREE"
    git diff --name-status -z -M --diff-filter=ACMRT "$base" "$commit" > "$TMP/names-$commit"
    while IFS= read -r -d '' status; do
        source=""
        case "$status" in
            R*|C*)
                IFS= read -r -d '' source; IFS= read -r -d '' path
                [ "$(kind_of "$source")" = "$(kind_of "$path")" ] || source="" ;;
            *) IFS= read -r -d '' path ;;
        esac
        check_path "$commit" "$base" "$path" "$source"
    done < "$TMP/names-$commit"
}

has_green_note() {
    local text tree
    text="$(note_of "$1")" || return 1
    tree="$(git rev-parse "$1^{tree}")"
    printf '%s\n' "$text" | grep -qE "(^| )kind=final( |$)" \
        && printf '%s\n' "$text" | grep -qE "(^| )result=green( |$)" \
        && printf '%s\n' "$text" | grep -qE "(^| )cases=[1-9][0-9]*( |$)" \
        && printf '%s\n' "$text" | grep -qE "(^| )tree=$tree( |$)"
}

count=0
while read -r old new ref; do
    count=$((count + 1))
    case "$ref" in
        refs/heads/*) ;;
        *) continue ;;
    esac
    branch="${ref#refs/heads/}"
    if [ "$new" = "$ZERO" ]; then
        [ "$branch" = dev ] && refuse "deletes dev" "$ref"
        continue
    fi
    if [ "$branch" = dev ] && [ "$old" != "$ZERO" ] && ! git merge-base --is-ancestor "$old" "$new"; then
        refuse "not a fast-forward" "$ref"
    fi
    for commit in $(git rev-list "$new" --not --all); do
        check_commit "$commit"
    done
    if [ "$branch" != main ] && ! has_green_note "$new"; then
        refuse "no note of a green run" "$ref at $(short "$new")"
    fi
done < "$TMP/updates"

[ "$refused" -eq 0 ] || exit 1
echo "tor: checked $count refs" >&2
exit 0
