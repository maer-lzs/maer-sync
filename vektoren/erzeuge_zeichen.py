# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/zeichen.json`, indem `core/authorization.py` gefahren wird.

    python3 vektoren/erzeuge_zeichen.py > vektoren/zeichen.json

Abdrücke, Kopfzeilen und die Zerlegung der gespeicherten Form. Jeder Fall wird
gerufen; nichts hier behauptet, was `authorization` tut.

**Das Salz wird eingespeist, nicht gezogen.** `hash_secret` nimmt eines
entgegen, und nur deshalb ist diese Datei byte-stabil. Ein Erzeuger, der das
echte Salz ziehen liesse, schriebe bei jedem Lauf eine andere Datei, und der
Zeuge, der sie byte-gleich hält, wäre dauerhaft rot. Die zwei festen Salze
sind erfunden und stehen als solche da.

**Kein Wert in dieser Datei ist ein Geheimnis.** `gueltiges-zeichen` ist ein
Platzhalter, der nie ausgegeben wurde; das Konto dazu gibt es nicht. Deshalb
steht er im Klartext, und deshalb MUSS er es: maskiert würde aus den drei
Fällen „gültiges Zeichen", „falsches Zeichen" und „kein Zeichen" derselbe
Fall, und genau das ist an dieser Stelle schon einmal passiert.

**Was der Erzeuger über die Fälle hinaus misst:**

1. **Ein richtiges Geheimnis passt und ein falsches nicht**, je gespeicherter
   Form. Ohne die zweite Hälfte wäre ein `secret_matches`, das immer `True`
   sagt, in jeder Zeile grün.
2. **Jede unlesbare Form wird abgewiesen.** Eine, die durchginge, hält den
   Erzeuger an, statt als `"matches": false` in die Datei zu wandern.
3. **Kein zwei Abdrücke sind gleich.** Zwei gleiche Zeilen wären eine.

Die scrypt-Rechnung selbst ist teuer; dieser Erzeuger ruft sie sechsmal und
braucht dafür ein paar Sekunden. Das ist der Preis dafür, dass die Zahlen
gerechnet und nicht abgeschrieben sind.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.authorization import (DENIED, KEY_BYTES, SALT_BYTES, SCRYPT_N, SCRYPT_P,
                                SCRYPT_R, caller_for, hash_secret, secret_matches,
                                token_digest)

#: Erfunden, nie ausgegeben, und zu keinem Konto gehörend.
GUELTIG = "gueltiges-zeichen"
BESITZER = "lzs"

ZEICHEN = [
    "",
    "a",
    "ein-zeichen",
    "0123456789abcdef" * 4,
    "mit Umlaut oeae",
    "  fuehrende Leerzeichen",
]

#: Die Kopfzeilen, und was sie prüfen: Grossschreibung, zusätzliche
#: Leerzeichen, ein fremdes Schema, ein fehlendes Schema, ein leerer Rest und
#: ein zweites Wort dahinter.
KOPFZEILEN = [
    None,
    "",
    f"Bearer {GUELTIG}",
    f"bearer {GUELTIG}",
    f"BEARER {GUELTIG}",
    f"Bearer  {GUELTIG}",
    "Bearer falsches",
    f"Basic {GUELTIG}",
    GUELTIG,
    "Bearer",
    "Bearer   ",
    f"Bearer {GUELTIG} extra",
]

#: (Geheimnis, Salz). Beide Salze sind erfunden und fest, damit die Datei
#: byte-stabil ist.
FORMEN = [
    ("hallo", bytes(range(16))),
    ("zweites Geheimnis", bytes.fromhex("ab" * 16)),
]

UNLESBAR = [
    "",
    "scrypt",
    "scrypt$1$2$3$4",
    "bcrypt$32768$8$1$aabb$ccdd",
    "scrypt$x$8$1$aabb$ccdd",
    "scrypt$32768$8$1$zz$ccdd",
    "scrypt$32768$8$1$aabb$zz",
    "scrypt$32768$8$1$aabb$ccdd$extra",
]


def besitzer_von(digest):
    """Der Nachschlag, den `caller_for` bekommt: genau ein Zeichen ist vergeben."""
    return BESITZER if digest == token_digest(GUELTIG) else None


abdruecke = [{"token": z, "digest": token_digest(z)} for z in ZEICHEN]
if len({a["digest"] for a in abdruecke}) != len(abdruecke):
    sys.exit("ABBRUCH: zwei Zeichen haben denselben Abdruck. Es wird nichts gedruckt.")

rufer = []
for kopf in KOPFZEILEN:
    wer = caller_for(kopf, besitzer_von)
    rufer.append({"header": kopf, "owner": wer.owner, "reason": wer.reason,
                  "authorized": wer.authorized})
if not any(r["authorized"] for r in rufer):
    sys.exit("ABBRUCH: kein einziger Fall ist berechtigt. Dann misst die Tafel nur "
             "Abweisungen, und ein caller_for, das immer nein sagt, waere gruen.")

formen = []
for geheimnis, salz in FORMEN:
    gespeichert = hash_secret(geheimnis, salt=salz)
    passt = secret_matches(gespeichert, geheimnis)
    passt_nicht = secret_matches(gespeichert, geheimnis + "x")
    if not passt or passt_nicht:
        sys.exit(f"ABBRUCH: die gespeicherte Form von {geheimnis!r} urteilt falsch "
                 f"(richtig: {passt}, falsch: {passt_nicht}).")
    formen.append({"secret": geheimnis, "stored": gespeichert,
                   "passt": passt, "passt_nicht": passt_nicht})

unlesbar = []
for gespeichert in UNLESBAR:
    if secret_matches(gespeichert, "hallo"):
        sys.exit(f"ABBRUCH: die unlesbare Form {gespeichert!r} ging durch. "
                 "Es wird nichts gedruckt.")
    unlesbar.append({"stored": gespeichert, "matches": False})

leer = formen[0]["stored"]
if secret_matches(leer, ""):
    sys.exit("ABBRUCH: ein leeres Angebot passte auf eine gespeicherte Form.")

sys.stdout.write(json.dumps({
    "_was": "Scheibe 3, reine Haelfte. Abdruecke, Kopfzeilen und die Zerlegung der "
            "gespeicherten Form, mit der Python-Fassung erzeugt. Die scrypt-Rechnung "
            "selbst fehlt noch, ihre Crate ist ein eigener Entscheid; was hier liegt, "
            "misst alles davor und alle Abweisungen.",
    "DENIED": DENIED,
    "parameter": {"n": SCRYPT_N, "r": SCRYPT_R, "p": SCRYPT_P,
                  "salt_bytes": SALT_BYTES, "key_bytes": KEY_BYTES},
    "token_digest": abdruecke,
    "caller_for": rufer,
    "gespeicherte_formen": formen,
    "unlesbare_formen": unlesbar,
    "leeres_angebot": [{"stored": leer, "offered": "", "matches": False}]},
    indent=2, ensure_ascii=True))
