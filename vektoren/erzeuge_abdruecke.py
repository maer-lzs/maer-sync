# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/abdruecke.json` aus `core/identity.py`.

    python3 vektoren/erzeuge_abdruecke.py > vektoren/abdruecke.json

**Warum diese Datei einen Erzeuger bekommt** (Entscheid 19.09.2026): sie lag
seit Scheibe 1 als Zahlen ohne Skript daneben, und eine Vektordatei ohne
Erzeuger ist eine Behauptung. Sie trägt vier der Fälle, auf denen die
Rust-Seite steht.

**Der Abdruck wird hier ZWEIMAL gerechnet**, und das ist der eigentliche Wert
dieser Datei. Einmal über `core.identity.fingerprint`, und einmal daneben aus
`hashlib.sha256` und der veröffentlichten Crockford-Tabelle, ohne `identity`
zu rufen und mit einer anderen Rechnung: die Nachrechnung schiebt kein
Bitfenster über eine grosse Zahl, sondern hängt die Bits als Text aneinander
und schneidet ihn in Fünferstücke. Zwei Wege, die sich nur im Ergebnis treffen
dürfen. Weichen sie ab, hält der Erzeuger an, statt eine Zahl zu drucken.

Ohne diese zweite Rechnung bewiese die Datei nur, dass unsere Python-Fassung
mit unserer Rust-Fassung übereinstimmt. Mit ihr steht dahinter ein Verfahren,
das ein Dritter nachrechnen kann: SHA-256 und eine veröffentlichte Tabelle.

**Die Ausgabe ist byte-stabil**: feste Schlüssel, kein Zeitstempel, nichts
Zufälliges. Sie endet ohne Zeilenumbruch, weil die eingecheckte Datei so
aussieht; `tests/test_erzeuger.py` hält beide byte-gleich gegeneinander.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.identity import ALPHABET, FINGERPRINT_BYTES, GROUP, fingerprint

#: Die vier Schlüssel, und warum gerade diese: zwei Ränder (alles null, alles
#: gesetzt), einer, bei dem jedes Byte gleich ist, ohne Rand zu sein, und
#: einer, bei dem jedes Byte verschieden ist. Ein Abdruck, der Bytes
#: vertauscht oder eine Gruppe doppelt nimmt, fällt an den letzten beiden auf
#: und an den ersten beiden nicht.
SCHLUESSEL = [
    ("null", bytes(32)),
    ("sieben", bytes([0x07]) * 32),
    ("aufsteigend", bytes(range(32))),
    ("ff", bytes([0xFF]) * 32),
]


def nachgerechnet(roh):
    """Derselbe Abdruck, aus der veröffentlichten Tabelle und anders gerechnet."""
    digest = hashlib.sha256(roh).digest()[:FINGERPRINT_BYTES]
    bits = "".join(format(b, "08b") for b in digest)
    zeichen = "".join(ALPHABET[int(bits[i:i + 5], 2)] for i in range(0, len(bits), 5))
    return "-".join(zeichen[i:i + GROUP] for i in range(0, len(zeichen), GROUP))


faelle = []
for name, roh in SCHLUESSEL:
    aus_identity = fingerprint(roh)
    aus_tabelle = nachgerechnet(roh)
    if aus_identity != aus_tabelle:
        sys.exit(f"ABBRUCH: die zwei Rechnungen weichen bei {name} ab: "
                 f"{aus_identity} gegen {aus_tabelle}. Es wird nichts gedruckt.")
    faelle.append({"name": name, "key_hex": roh.hex(), "fingerprint": aus_identity})

if len(faelle) != len(SCHLUESSEL):
    sys.exit("ABBRUCH: nicht jeder Schluessel hat einen Fall ergeben.")

sys.stdout.write(json.dumps({
    "quelle": "core/identity.py, gegengerechnet mit einer unabhaengigen Crockford-Umsetzung",
    "verfahren": "SHA-256 des rohen 32-Byte-Schluessels, erste 15 Byte, "
                 "Crockford-base32, Gruppen von 4",
    "faelle": faelle},
    indent=2, ensure_ascii=True))
