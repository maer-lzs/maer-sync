# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/sitzungen.json`, indem `core/sessions.py` gefahren wird.

    python3 vektoren/erzeuge_sitzungen.py > vektoren/sitzungen.json

Die Fälle sind Ränder, weil die Mitte jeder Regel uninteressant ist und ein
Verschieben um eins am Rand wohnt: genau auf der Frist, eine Sekunde davor,
genau an der Decke.

**Was dieser Erzeuger über die Fälle hinaus misst**, weil ihm sonst eine
ganze Klasse wegbrechen könnte, ohne dass die Datei kleiner aussieht:

1. **Alle drei Stände kommen vor.** Eine Tafel ohne `old` prüft die
   Altersgrenze nicht mehr.
2. **Mindestens vier Fälle, in denen BEIDES zutrifft** (zu alt UND still).
   Das sind die, die die Reihenfolge der Antwort festhalten; ohne sie wäre
   `standing` frei, das Falsche zuerst zu nennen, und der Grund, der in eine
   Protokollzeile reist, wäre der unehrliche.
3. **`give_way` erfindet nichts und verliert nichts**: jeder genannte Name
   stammt aus der Liste, die hineingegangen ist.
4. **Die kleinen Grenzen sind wirklich klein.** Sie stehen in der Datei, damit
   ein Fall mit drei Zeilen eine volle Decke zeigen kann. Wären sie aus
   Versehen die ausgelieferten, zeigte kein Fall mehr eine volle Decke, und
   alle blieben grün.

Die kleinen Grenzen gelten für `give_way`, `sweep` und `room_for_one`; die
`standing`-Fälle fahren mit den AUSGELIEFERTEN Zahlen, weil dort genau diese
Ränder gemeint sind.

Die Ausgabe ist byte-stabil: feste Zahlen, keine Uhr. Sie endet ohne
Zeilenumbruch, wie die eingecheckte Datei.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.sessions import (CAPACITY, IDLE, IDLE_SECONDS, LIVE, MAX_AGE_SECONDS, OLD,
                           UNPROVEN_IDLE_SECONDS, Limits, give_way, room_for_one,
                           standing, sweep)

BEIDES = "beides zugleich: zu alt UND still, das Alter muss sprechen"

#: (created_at, last_used_at, now, proven, Vermerk oder None)
STAENDE = [
    (0, 0, 10, True, None),
    (0, 0, 10, False, None),
    (0, 0, UNPROVEN_IDLE_SECONDS, False, None),
    (0, 0, UNPROVEN_IDLE_SECONDS - 1, False, None),
    (0, 0, IDLE_SECONDS, True, None),
    (0, 0, IDLE_SECONDS - 1, True, None),
    (0, 3500, IDLE_SECONDS, True, None),
    (0, MAX_AGE_SECONDS, MAX_AGE_SECONDS, True, None),
    (0, MAX_AGE_SECONDS - 1, MAX_AGE_SECONDS - 1, True, None),
    (0, MAX_AGE_SECONDS - 1, MAX_AGE_SECONDS, False, None),
    (100, 100, 200, False, None),
    (100, 190, 200, False, None),
    (0, 0, MAX_AGE_SECONDS, True, BEIDES),
    (0, 0, 90000, True, BEIDES),
    (0, 0, MAX_AGE_SECONDS, False, BEIDES),
    (0, 0, 100000, False, BEIDES),
]

#: Klein genug, dass drei Zeilen eine volle Decke sind.
KLEIN = Limits(idle_seconds=100, unproven_idle_seconds=10, max_age_seconds=1000,
               capacity=3)

#: (Name, [(id, last_used_at, proven)])
WEICHEN = [
    ("leer", []),
    ("unter der Decke", [("a", 10, True), ("b", 20, True)]),
    ("genau voll, unbewiesene zuerst",
     [("a", 10, True), ("b", 20, False), ("c", 30, True)]),
    ("genau voll, alle bewiesen",
     [("a", 30, True), ("b", 10, True), ("c", 20, True)]),
    ("darueber",
     [("a", 10, True), ("b", 20, False), ("c", 30, True), ("d", 40, False)]),
    ("alle unbewiesen",
     [("a", 30, False), ("b", 10, False), ("c", 20, False)]),
]

#: (Name, [(id, created_at, last_used_at, proven)], now)
FEGEN = [
    ("nichts tot", [("a", 0, 90, True), ("b", 0, 95, False)], 100),
    ("unbewiesen verfallen", [("a", 0, 80, False), ("b", 0, 95, True)], 100),
    ("zu alt", [("a", 0, 1000, True)], 1000),
    ("gemischt", [("a", 0, 0, True), ("b", 0, 95, True), ("c", 0, 80, False)], 100),
]

PLATZ = [
    ("tote duerfen keinen lebenden kosten",
     [("alt1", 0, 0, True), ("alt2", 0, 0, True), ("neu", 95, 95, False)], 100),
    ("voll und alles lebt",
     [("a", 90, 90, True), ("b", 92, 95, True), ("c", 94, 96, False)], 100),
    ("platz da", [("a", 90, 95, True)], 100),
]

if not (KLEIN.capacity < CAPACITY and KLEIN.idle_seconds < IDLE_SECONDS
        and KLEIN.max_age_seconds < MAX_AGE_SECONDS):
    sys.exit("ABBRUCH: die kleinen Grenzen sind nicht kleiner als die ausgelieferten. "
             "Dann zeigt kein Fall mehr eine volle Decke und alle blieben gruen.")

staende = []
for erzeugt, benutzt, jetzt, bewiesen, vermerk in STAENDE:
    zeile = {"created_at": erzeugt, "last_used_at": benutzt, "now": jetzt,
             "proven": bewiesen,
             "standing": standing(erzeugt, benutzt, jetzt, bewiesen)}
    if vermerk:
        zeile["_warum"] = vermerk
    staende.append(zeile)

gesehen = {s["standing"] for s in staende}
fehlend = {LIVE, IDLE, OLD} - gesehen
if fehlend:
    sys.exit(f"ABBRUCH: diese Staende kommen in keinem Fall vor: {sorted(fehlend)}.")
if sum(1 for s in staende if "_warum" in s) < 4:
    sys.exit("ABBRUCH: weniger als vier Faelle, in denen beides zutrifft. Genau sie "
             "halten fest, welche Bedingung zuerst spricht.")

weichen = []
for name, sitzungen in WEICHEN:
    aus = give_way([tuple(s) for s in sitzungen], KLEIN)
    namen = {s[0] for s in sitzungen}
    if set(aus) - namen:
        sys.exit(f"ABBRUCH: {name} laesst Namen weichen, die nie da waren: {aus}")
    weichen.append({"name": name,
                    "sessions": [list(s) for s in sitzungen], "give_way": aus})

fegen = []
for name, sitzungen, jetzt in FEGEN:
    aus = sweep([tuple(s) for s in sitzungen], jetzt, KLEIN)
    namen = {s[0] for s in sitzungen}
    if set(aus) - namen:
        sys.exit(f"ABBRUCH: {name} fegt Namen, die nie da waren: {aus}")
    fegen.append({"name": name, "sessions": [list(s) for s in sitzungen],
                  "now": jetzt, "sweep": aus})

platz = []
for name, sitzungen, jetzt in PLATZ:
    aus = room_for_one([tuple(s) for s in sitzungen], jetzt, KLEIN)
    namen = {s[0] for s in sitzungen}
    if set(aus) - namen:
        sys.exit(f"ABBRUCH: {name} raeumt Namen weg, die nie da waren: {aus}")
    platz.append({"name": name, "sessions": [list(s) for s in sitzungen],
                  "now": jetzt, "room_for_one": aus})

sys.stdout.write(json.dumps({
    "_was": "Scheibe 4, reine Haelfte: die Buchfuehrung der Sitzungen. Die Ordnung ist "
            "hier die Entscheidung, nicht die einzelne Zahl, deshalb traegt jeder Fall "
            "die ganze Liste und ihre Antwort.",
    "konstanten": {"IDLE_SECONDS": IDLE_SECONDS,
                   "UNPROVEN_IDLE_SECONDS": UNPROVEN_IDLE_SECONDS,
                   "MAX_AGE_SECONDS": MAX_AGE_SECONDS,
                   "CAPACITY": CAPACITY,
                   "LIVE": LIVE, "IDLE": IDLE, "OLD": OLD},
    "standing": staende,
    "kleine_grenzen": {"idle_seconds": KLEIN.idle_seconds,
                       "unproven_idle_seconds": KLEIN.unproven_idle_seconds,
                       "max_age_seconds": KLEIN.max_age_seconds,
                       "capacity": KLEIN.capacity},
    "give_way": weichen,
    "sweep": fegen,
    "room_for_one": platz},
    indent=2, ensure_ascii=True))
