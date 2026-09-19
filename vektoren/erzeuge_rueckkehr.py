# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/rueckkehr.json`, indem `core/restore.py` gefahren wird.

    python3 vektoren/erzeuge_rueckkehr.py > vektoren/rueckkehr.json

Jeder Fall wird gerufen, und Ausgang, Satz und `passed` sind die Antwort.
Nichts davon steht hier als Erwartung, sonst wäre eine falsche Umsetzung
zusammen mit einer falschen Erwartung still grün.

**Der Erzeuger prüft zwei Dinge über die Fälle selbst**, weil ihm sonst
unbemerkt eine Gruppe wegbrechen könnte:

1. **Alle drei Ausgänge kommen vor.** Eine Tafel, aus der `not_measured`
   verschwunden ist, prüft genau die Klasse nicht mehr, für die dieses Werk
   drei Ausgänge hat statt zwei.
2. **Nur `fits` besteht.** Käme ein `differs` oder `not_measured` mit
   `passed: true` heraus, wäre das kein Fall für die Tafel, sondern ein
   Befund, und der Erzeuger hält an.

Die Ausgabe ist byte-stabil: feste Eingaben, keine Uhr, kein Pfad. Sie endet
ohne Zeilenumbruch, wie die eingecheckte Datei.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.restore import DIFFERS, FITS, NOT_MEASURED, restore_verdict

#: (Name, gesichert, zurückgelesen, Fehler). Die vier Ränder zuerst, in denen
#: NICHT gemessen wurde, danach der Gutfall, danach die Unterschiede.
FAELLE = [
    ("leerer Bestand ist nicht gruen", {}, {}, None),
    ("nichts uebergeben", None, None, None),
    ("read_back fehlt", {"a": 1}, None, None),
    ("Probe gestoppt", {"a": 1}, {"a": 1}, "die Datei war gesperrt"),
    ("passt", {"a": 10, "b": 20}, {"a": 10, "b": 20}, None),
    ("eins fehlt", {"a": 10, "b": 20}, {"a": 10}, None),
    ("zwei fehlen", {"a": 1, "b": 2, "c": 3}, {"a": 1}, None),
    ("eins zuviel", {"a": 10}, {"a": 10, "z": 5}, None),
    ("falsche Groesse", {"a": 10, "b": 20}, {"a": 10, "b": 21}, None),
    ("alles falsch", {"a": 1, "b": 2}, {"a": 9, "b": 8}, None),
]

faelle = []
for name, aus, zurueck, fehler in FAELLE:
    urteil = restore_verdict(aus, zurueck, error=fehler)
    if urteil.passed != (urteil.outcome == FITS):
        sys.exit(f"ABBRUCH: {name} hat Ausgang {urteil.outcome} und passed "
                 f"{urteil.passed}. Nur fits besteht; es wird nichts gedruckt.")
    if not urteil.reason.strip():
        sys.exit(f"ABBRUCH: {name} urteilt stumm.")
    faelle.append({"name": name, "backed_up": aus, "read_back": zurueck,
                   "error": fehler, "outcome": urteil.outcome,
                   "reason": urteil.reason, "passed": urteil.passed})

gesehen = {f["outcome"] for f in faelle}
fehlend = {FITS, DIFFERS, NOT_MEASURED} - gesehen
if fehlend:
    sys.exit(f"ABBRUCH: diese Ausgaenge kommen in keinem Fall vor: {sorted(fehlend)}. "
             "Eine Tafel ohne sie prueft die Klasse nicht mehr, fuer die es sie gibt.")
if len(faelle) != len(FAELLE):
    sys.exit("ABBRUCH: nicht jeder Fall hat eine Zeile ergeben.")

sys.stdout.write(json.dumps({
    "_was": "Das Urteil ueber eine geprobte Rueckkehr. Drei Ausgaenge, und der dritte "
            "ist der Punkt: eine Probe, die nicht laufen konnte, ist KEINE bestandene "
            "Probe. Erzeugt mit der Python-Fassung.",
    "ausgaenge": {"FITS": FITS, "DIFFERS": DIFFERS, "NOT_MEASURED": NOT_MEASURED},
    "faelle": faelle},
    indent=2, ensure_ascii=True))
