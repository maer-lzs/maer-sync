#!/bin/sh
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# Erst verfolgen, dann messen, dann erst committen.
#
# WARUM ES DAS GIBT: zweimal an einem Tag habe ich committet, waehrend die
# Python-Suite rot war, weil Messung und Commit in EINER Befehlskette liefen
# und die Kette am Ergebnis nicht anhielt. Beide Male war der Befund echt
# (ein neues Quellverzeichnis, das kein Zaun kannte), und beide Male lag der
# kaputte Stand kurz auf dem Zweig.
#
# Dazu die aeltere Lehre, die hier gleich mit eingebaut ist: ein Zaun, der
# fragt, was VERFOLGT ist, misst vor dem `git add` einen anderen Stand. Also
# erst `git add -A`, dann messen.
#
# Ausgang 0 heisst: es darf committet werden. Sonst nicht.
set -eu
cd "$(dirname "$0")/.."

echo "== erst verfolgen =="
git add -A

echo "== Zaeune des Repos (Python) =="
# Seit dem 24.09.2026 ist hier kein Dienst mehr in Python. Was bleibt, sind
# die Zaeune, die das REPO pruefen (Lizenzkoepfe, was verfolgt ist): sie fragen
# `git`, und das gibt es auf dem Wirt, nicht im Bau-Abbild der Rust-Suite.
#
# In eine Datei, nicht durch eine Pipe: hinter einer Pipe ist der Ausgang der
# des LETZTEN Gliedes, also der von `tail`, und der ist immer 0. Genau diese
# Verwechslung hat die zwei roten Commits verursacht, die es zu verhindern
# gilt.
PROT=$(mktemp)
if python3 -m unittest discover -s tests -t . > "$PROT" 2>&1; then
  tail -3 "$PROT"
else
  tail -20 "$PROT"
  rm -f "$PROT"
  echo "HALT: ein Zaun des Repos ist rot. Nicht committen." >&2
  exit 1
fi
rm -f "$PROT"

echo "== Rust =="
# The fetch program (abruf/) is a package of its own; its witnesses run here
# too. Each package runs on its own, its exit code is read, and it has to
# have measured something: counted together, a package that did not compile
# left no result line, the other one counted, and the gate said green.
RPROT=$(mktemp)
gp=0; gf=0; gi=0
for paket in dienst abruf; do
  if [ "$paket" = abruf ]; then set -- --manifest-path abruf/Cargo.toml; else set --; fi
  if ! werkzeug/bau.sh test --offline "$@" > "$RPROT" 2>&1; then
    tail -20 "$RPROT"
    rm -f "$RPROT"
    echo "HALT: die Rust-Zeugen von $paket sind rot oder uebersetzen nicht. Nicht committen." >&2
    exit 1
  fi
  set -- $(grep -E '^test result' "$RPROT" | awk -F'[ ;]+' '{p+=$4; f+=$6; i+=$8} END {print p+0, f+0, i+0}')
  if [ "$1" -eq 0 ] || [ "$2" -ne 0 ]; then
    tail -20 "$RPROT"
    rm -f "$RPROT"
    echo "HALT: $paket hat nichts gemessen oder ist rot ($1 gruen, $2 rot). Nicht committen." >&2
    exit 1
  fi
  gp=$((gp + $1)); gf=$((gf + $2)); gi=$((gi + $3))
done
rm -f "$RPROT"
printf "  grün: %d  rot: %d  übersprungen: %d\n" "$gp" "$gf" "$gi"

echo "== Baum =="
git diff --cached --stat Cargo.toml Cargo.lock | tail -1
echo "frei zum Committen."
