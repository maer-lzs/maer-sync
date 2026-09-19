# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/rahmen.json`, indem `core/frames.py` gefahren wird.

    python3 vektoren/erzeuge_rahmen.py > vektoren/rahmen.json

**Nichts hier behauptet, was ein Rahmen ist.** Jeder gültige Fall wird
gepackt, und die Bytes, die dabei herauskommen, stehen in der Datei. Jeder
ungültige wird gelesen, und dass es dabei knallt, ist gemessen und nicht
angenommen: ein Fall, der in Wahrheit durchginge, hält den Erzeuger an,
statt als `"erwartet": "NotAFrame"` in die Datei zu wandern.

Dazu fährt jeder gültige Fall den Rundlauf: gepackt, wieder gelesen, und Kopf
und Körper müssen dieselben sein. Ohne das könnte `pack` seit Monaten etwas
schreiben, das nur es selbst wieder liest.

**Die zwei Schreibweisen mit dem Umlaut** entstehen hier ebenfalls und werden
nicht abgeschrieben. Die entwertete schreibt `core/frames.py` selbst; die rohe
rechnet dieser Erzeuger daneben aus, weil sie von `MaerSyncFrame.swift` und
`src/frames.rs` kommt und auf diesem Wirt kein Swift und kein Rust liegt. Was
er dabei NICHT tut, ist behaupten, dass beide dasselbe bedeuten: er liest
beide zurück und vergleicht die Bedeutung gegeneinander. Sind sie ungleich,
hält er an.

**Diese Datei ist die schwächste der Vektordateien**, und das steht so in der
README daneben: das Rahmenformat ist unser eigenes, die Zahlen halten den
heutigen Stand fest. Der Erzeuger ändert daran nichts, er macht sie
nachziehbar.

Die Ausgabe ist byte-stabil und endet ohne Zeilenumbruch, weil die
eingecheckte Datei so aussieht. `ensure_ascii` steht hier auf `False`, sonst
stünde der Umlaut in der Vektordatei entwertet da und der Fall, um den es
geht, wäre von aussen nicht mehr zu sehen.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.frames import LENGTH_BYTES, NotAFrame, pack, unpack

#: Kopf und Körper je Fall. Die vier decken: kein Körper, ein gewöhnlicher,
#: eine Antwort statt eines Antrags, und ein Körper mit einem Nullbyte und
#: einem Byte über 0x7F, also genau das, was einen Körper, der durch Text
#: ginge, zerreissen würde.
GUELTIG = [
    ("leerer_koerper", {"method": "GET", "target": "/v1/health"}, b""),
    ("kleiner_koerper", {"method": "PUT", "target": "/v1/docs/x",
                         "headers": {"If-Match": "3"}}, b"hallo"),
    ("antwort", {"status": 412, "headers": {"Content-Type": "application/json"}},
     b'{"a":1}'),
    ("koerper_mit_nullbyte", {"method": "POST", "target": "/v1/token"},
     bytes([0x00, 0x01, 0x02, 0xFF])),
]

#: Vier Formen von „kein Rahmen", und sie müssen alle denselben Fehler geben.
#: Ein Leser, der hier zwischen Fehlern unterscheidet, sagt einem Fremden,
#: welche seiner Bytes ihn wie weit gebracht haben.
UNGUELTIG = [
    ("zu_kurz", bytes([0x00])),
    ("kopf_abgeschnitten", bytes([0x00, 0x10]) + b"ab"),
    ("kopf_kein_json", bytes([0x00, 0x03]) + b"{{{"),
    ("leer", b""),
]

#: Der Kopf mit dem Umlaut, und der Körper dazu mit rohen Bytes.
UMLAUT = {"method": "GET", "target": "/v1/docs/hörbuch"}
UMLAUT_KOERPER = bytes([0x00, 0xFF]) + b"daten"


def kopflaenge(rahmen):
    return int.from_bytes(rahmen[:LENGTH_BYTES], "big")


gueltig = []
for name, kopf, koerper in GUELTIG:
    rahmen = pack(kopf, koerper)
    zurueck_kopf, zurueck_koerper = unpack(rahmen)
    if zurueck_kopf != kopf or zurueck_koerper != koerper:
        sys.exit(f"ABBRUCH: der Rundlauf von {name} kommt nicht heil zurueck.")
    gueltig.append({"name": name, "head": kopf, "body_hex": koerper.hex(),
                    "frame_hex": rahmen.hex(), "head_len": kopflaenge(rahmen)})

ungueltig = []
for name, bytes_ in UNGUELTIG:
    try:
        unpack(bytes_)
    except NotAFrame:
        ungueltig.append({"name": name, "frame_hex": bytes_.hex(),
                          "erwartet": "NotAFrame"})
        continue
    sys.exit(f"ABBRUCH: {name} sollte kein Rahmen sein und wurde gelesen. "
             "Es wird nichts gedruckt.")

# Die zwei Schreibweisen. Die entwertete kommt aus `pack`, die rohe daneben.
entwertet = pack(UMLAUT, UMLAUT_KOERPER)
roh_kopf = json.dumps(UMLAUT, separators=(",", ":"), sort_keys=True,
                      ensure_ascii=False).encode("utf-8")
roh = len(roh_kopf).to_bytes(LENGTH_BYTES, "big") + roh_kopf + UMLAUT_KOERPER

if entwertet == roh:
    sys.exit("ABBRUCH: die zwei Schreibweisen sind dieselbe, dann misst der Fall nichts.")
for name, rahmen in (("entwertet", entwertet), ("roh", roh)):
    kopf, koerper = unpack(rahmen)
    if kopf != UMLAUT or koerper != UMLAUT_KOERPER:
        sys.exit(f"ABBRUCH: die Schreibweise {name} bedeutet nicht dasselbe.")

if len(gueltig) != len(GUELTIG) or len(ungueltig) != len(UNGUELTIG):
    sys.exit("ABBRUCH: nicht jeder Fall hat eine Zeile ergeben.")

sys.stdout.write(json.dumps({
    "quelle": "core/frames.py",
    "format": "zwei Byte Kopflaenge big endian, JSON-Kopf (sortierte Schluessel, "
              "ohne Leerzeichen), dann roher Koerper",
    "gueltig": gueltig,
    "ungueltig": ungueltig,
    "gleiche_bedeutung": {
        "warum": "Der Kopf ist ein JSON-Objekt in UTF-8 mit sortierten Schluesseln. "
                 "Nicht-ASCII darf ROH oder ENTWERTET geschrieben werden, gelesen wird "
                 "die Bedeutung; der Vertrag verlangt keine Byte-Kanonik, und die drei "
                 "Umsetzungen schreiben heute schon verschieden (Python entwertet, Swift "
                 "und Rust roh). Ein Leser, der nur eine Schreibweise annimmt, ist der "
                 "Befund, nicht der Umlaut.",
        "bedeutung": UMLAUT,
        "body_hex": UMLAUT_KOERPER.hex(),
        "schreibweisen": [
            {"name": "entwertet", "frame_hex": entwertet.hex(),
             "head_len": kopflaenge(entwertet),
             "geschrieben_von": "core/frames.py (json.dumps, ensure_ascii Vorgabe)"},
            {"name": "roh", "frame_hex": roh.hex(), "head_len": kopflaenge(roh),
             "geschrieben_von": "MaerSyncFrame.swift (JSONSerialization) und "
                                "src/frames.rs (serde_json)"},
        ]}},
    indent=2, ensure_ascii=False))
