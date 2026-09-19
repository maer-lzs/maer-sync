# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/kopf-und-schluessel.json` aus `core/frames.py` und
`core/identity.py`.

    python3 vektoren/erzeuge_kopf_und_schluessel.py > vektoren/kopf-und-schluessel.json

Drei Stücke in einer Datei, weil sie denselben Anlass haben: sie standen bei
der Zählung der 317 ohne Gegenpart da und sind rein. Das Lesen eines
Antrags-Kopfs, das Lesen eines Antwort-Kopfs, und das Urteil über einen
Dienst-Schlüssel.

Jeder Fall wird gerufen. Die Fehlerfälle werden ebenfalls gerufen: dass sie
abgewiesen werden, ist gemessen, und einer, der in Wahrheit durchginge, hält
den Erzeuger an, statt als Zeile in die Datei zu wandern.

**Die zwei abweisenden Urteile werden zusätzlich geprüft**, und zwar auf das,
wofür sie da sind: ihr Satz muss den Befehl nennen, mit dem ein Betreiber
weiterkommt. Eine Abweisung, die ein Terminal erreicht, ohne zu sagen, was zu
tun ist, ist der Fehler, gegen den dieses Stück geschrieben wurde. Ohne diese
Prüfung könnte der Satz verkümmern und die Tafel bliebe zu sich selbst
stimmig.

**Die Abdrücke in den `decide`-Fällen sind erfunden und nicht gerechnet.**
`decide` sieht nur Text; welcher Abdruck zu welchem Schlüssel gehört,
entscheidet sich eine Ebene tiefer und steht in `abdruecke.json`. Hier sind
sie mit Absicht so gewählt, dass ein Mensch sie im Satz wiederfindet.

Die Ausgabe ist byte-stabil und endet ohne Zeilenumbruch, wie die
eingecheckte Datei.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.frames import NotAFrame, reply_of, request_of
from core.identity import ADOPT, MAKE_NEW, decide

ANTRAEGE = [
    ("gewoehnlich", {"method": "GET", "target": "/v1/doc/stands"}),
    ("mit Frage", {"method": "GET", "target": "/v1/doc/stands?scope=a&x=1"}),
    ("leere Frage", {"method": "GET", "target": "/v1/doc/stands?"}),
    ("Kopfzeilen klein", {"method": "PUT", "target": "/v1/doc/x",
                          "headers": {"If-Match": "3", "X-MAER-FORMAT": "1"}}),
    ("POST", {"method": "POST", "target": "/v1/token"}),
]

ANTRAEGE_FEHLER = [
    ("keine Methode", {"target": "/x"}),
    ("fremde Methode", {"method": "DELETE", "target": "/x"}),
    ("Ziel ohne Schraegstrich", {"method": "GET", "target": "x"}),
    ("Ziel ist Zahl", {"method": "GET", "target": 1}),
    ("Kopfzeilen keine Karte", {"method": "GET", "target": "/x", "headers": []}),
    ("Kopfzeile kein Paar", {"method": "GET", "target": "/x", "headers": {"a": 1}}),
]

ANTWORTEN = [
    ("einfach", {"status": 200}),
    ("mit Kopfzeilen", {"status": 412, "headers": {"ETag": "3"}}),
    ("Rand unten", {"status": 100}),
    ("Rand oben", {"status": 599}),
]

ANTWORTEN_FEHLER = [
    ("kein Status", {}),
    ("Status als Text", {"status": "200"}),
    # In Python IST True eine Zahl. Der Fall steht deshalb hier und nicht
    # unter den Selbstverstaendlichkeiten.
    ("Status als Wahrheitswert", {"status": True}),
    ("zu klein", {"status": 99}),
    ("zu gross", {"status": 600}),
]

#: Erfundene Abdrücke, absichtlich wiedererkennbar. Der eine ist der, der
#: hier liegt, der andere der, den der Speicher erwartet.
HIER = "AAAA-BBBB-CCCC-DDDD-EEEE-FFFF"
ERWARTET = "ZZZZ-YYYY-XXXX-WWWW-VVVV-TTTT"

STAENDE = [
    (None, None),
    (HIER, None),
    (HIER, HIER),
    (None, ERWARTET),
    (HIER, ERWARTET),
]


def gerufen_und_abgewiesen(funktion, name, kopf):
    try:
        funktion(kopf)
    except NotAFrame:
        return
    sys.exit(f"ABBRUCH: {name} sollte abgewiesen werden und ging durch. "
             "Es wird nichts gedruckt.")


antraege = []
for name, kopf in ANTRAEGE:
    methode, pfad, frage, kopfzeilen = request_of(kopf)
    antraege.append({"name": name, "head": kopf, "method": methode, "path": pfad,
                     "query": frage.decode("ascii"), "headers": kopfzeilen})

antraege_fehler = []
for name, kopf in ANTRAEGE_FEHLER:
    gerufen_und_abgewiesen(request_of, name, kopf)
    antraege_fehler.append({"name": name, "head": kopf})

antworten = []
for name, kopf in ANTWORTEN:
    status, kopfzeilen = reply_of(kopf)
    antworten.append({"name": name, "head": kopf, "status": status,
                      "headers": kopfzeilen})

antworten_fehler = []
for name, kopf in ANTWORTEN_FEHLER:
    gerufen_und_abgewiesen(reply_of, name, kopf)
    antworten_fehler.append({"name": name, "head": kopf})

staende = []
for gefunden, gemerkt in STAENDE:
    urteil = decide(gefunden, gemerkt)
    if not urteil.reason.strip():
        sys.exit(f"ABBRUCH: der Stand {urteil.name} urteilt stumm.")
    if not urteil.may_serve and ADOPT not in urteil.reason and MAKE_NEW not in urteil.reason:
        sys.exit(f"ABBRUCH: der abweisende Stand {urteil.name} nennt keinen Befehl. "
                 "Eine Abweisung darf kein Terminal erreichen, ohne zu sagen, was zu "
                 "tun ist; es wird nichts gedruckt.")
    staende.append({"found": gefunden, "remembered": gemerkt, "name": urteil.name,
                    "may_serve": urteil.may_serve, "reason": urteil.reason})

if len({s["name"] for s in staende}) != len(STAENDE):
    sys.exit("ABBRUCH: zwei Staende tragen denselben Namen, dann fehlt ein Fall.")
if sum(1 for s in staende if not s["may_serve"]) != 2:
    sys.exit("ABBRUCH: es sind nicht mehr genau zwei abweisende Staende. Das ist ein "
             "Befund und keine neue Tafel.")

sys.stdout.write(json.dumps({
    "_was": "Die drei Stuecke, die bei der Zaehlung ohne Gegenpart dastanden, und rein "
            "sind.",
    "request_of": antraege,
    "request_of_fehler": antraege_fehler,
    "reply_of": antworten,
    "reply_of_fehler": antworten_fehler,
    "ADOPT": ADOPT,
    "MAKE_NEW": MAKE_NEW,
    "decide": staende},
    indent=2, ensure_ascii=True))
