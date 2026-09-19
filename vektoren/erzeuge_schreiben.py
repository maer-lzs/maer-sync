# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/schreiben.json` aus den fünf reinen Modulen der Scheibe 2.

    python3 vektoren/erzeuge_schreiben.py > vektoren/schreiben.json

`core/versions.py`, `core/admission.py`, `core/formats.py`, `core/writes.py`
und `core/paths.py`. Jeder Fall wird gerufen, und **jedes Urteil kommt mit
seinem SATZ** in die Datei: ein Urteil ohne Satz wäre genau die stille
Abweisung, gegen die diese Tore gebaut sind, und ein Zeuge, der nur den Status
vergleicht, liesse eine falsche Begründung durch.

**Was der Erzeuger über die einzelnen Fälle hinaus misst**, weil sonst eine
ganze Gruppe still verkümmern könnte:

1. **Kein Urteil ist stumm.** Ein leerer Satz hält ihn an.
2. **`rotate` erfindet nichts und verliert nichts:** was fällt und was bleibt,
   ergibt zusammen wieder die Eingabe, in derselben Reihenfolge.
3. **Jeder Fehlerfall wird gerufen.** Dass er knallt, ist gemessen; einer, der
   durchginge, wandert nicht als Zeile in die Datei.
4. **Alle drei Status kommen unter den `decide`-Fällen vor.** Eine Tafel ohne
   412 prüft die Vorbedingung nicht mehr, und genau sie steht zwischen zwei
   Geräten und einem verlorenen Schrieb.
5. **Es gibt mindestens einen Fall, in dem ZWEI Tore zugleich verletzt sind.**
   Ohne ihn wäre die Reihenfolge der Tore frei, und keine Zeile würde reissen,
   wenn sie sich ändert.

Die Ausgabe ist byte-stabil: feste Zahlen, keine Uhr, kein Pfad aus der
Umgebung. Sie endet ohne Zeilenumbruch, wie die eingecheckte Datei.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.admission import DEFAULT_SHRINK_FLOOR, admit
from core.formats import serve_verdict, write_verdict
from core.paths import VERSION, parse_document_path, parse_listing_path, valid_scope
from core.versions import rotate
from core.writes import REFUSED, STALE, WRITTEN, decide

# ------------------------------------------------------------------- rotate

DREHUNGEN = [
    ([], 1),
    (["a"], 1),
    (["a", "b", "c"], 1),
    (["a", "b", "c"], 2),
    (["a", "b", "c"], 3),
    (["a", "b", "c"], 9),
    (["a", "b", "c", "d", "e", "f", "g", "h"], 3),
]
DREHUNGEN_FEHLER = [(["a"], 0), (["a"], -1)]

# -------------------------------------------------------------------- admit
#: (stored_bytes, incoming_bytes, declared_shrink). Die Ränder: der erste
#: Stand, ein leerer Speicher, der bekannte Verlustfall mit und ohne Ansage,
#: genau auf dem Boden, knapp darüber, Wachstum, und drei Rundungen.
GROESSEN = [
    (None, 0, False),
    (None, 100, False),
    (0, 50, False),
    (100, 0, False),
    (100, 0, True),
    (100, 40, False),
    (100, 40, True),
    (100, 50, False),
    (100, 51, False),
    (100, 100, False),
    (100, 250, False),
    (3, 1, False),
    (7, 3, False),
    (1000, 499, False),
]
GROESSEN_FEHLER = [(None, -1), (-5, 1)]

# ------------------------------------------------------- Fassungen und Gestalt
SCHREIBER = [(None, 1), (1, 1), (1, 2), (2, 1), (5, 9), (9, 5)]
GESTALTEN = [(None, 1), (1, 1), (1, 2), (2, 1), (3, 3)]

# ------------------------------------------------------------------- decide
#: Jeder Eintrag ist ein voller Schrieb. Die letzten drei verletzen zwei Tore
#: zugleich und halten damit die REIHENFOLGE fest.
SCHRIEBE = [
    dict(stored_bytes=None, stored_format=None, stored_etag=None,
         incoming_bytes=10, writer_format=1),
    dict(stored_bytes=None, stored_format=None, stored_etag=None,
         incoming_bytes=10, writer_format=1, if_match="3"),
    dict(stored_bytes=100, stored_format=1, stored_etag="3",
         incoming_bytes=120, writer_format=1),
    dict(stored_bytes=100, stored_format=1, stored_etag="3",
         incoming_bytes=120, writer_format=1, if_match="2"),
    dict(stored_bytes=100, stored_format=1, stored_etag="3",
         incoming_bytes=120, writer_format=1, if_match="3"),
    dict(stored_bytes=100, stored_format=2, stored_etag="3",
         incoming_bytes=120, writer_format=1, if_match="3"),
    dict(stored_bytes=100, stored_format=1, stored_etag="3",
         incoming_bytes=0, writer_format=1, if_match="3"),
    dict(stored_bytes=100, stored_format=1, stored_etag="3",
         incoming_bytes=0, writer_format=1, if_match="3", declared_shrink=True),
    dict(stored_bytes=100, stored_format=1, stored_etag="3",
         incoming_bytes=10, writer_format=1, if_match="3"),
    dict(stored_bytes=100, stored_format=2, stored_etag="3",
         incoming_bytes=1, writer_format=1, if_match="3"),
    dict(stored_bytes=100, stored_format=2, stored_etag="3",
         incoming_bytes=1, writer_format=1, if_match="2"),
]

# -------------------------------------------------------------------- Pfade
PFADE = [
    "/v1/doc/stands",
    "/v1/doc/stands/",
    "v1/doc/stands",
    "/v1/doc/Stands",
    "/v1/doc/",
    "/v1/doc",
    "/v2/doc/stands",
    "/v1/docs/stands",
    "/v1/doc/a",
    "/v1/doc/" + "a" * 32,
    "/v1/doc/" + "a" * 33,
    "/v1/doc/1stands",
    "/v1/doc/st_ands",
    "/v1/doc/st-ands",
]

BEREICHE = ["", None, "abc", "A.b_c:d-e", "a" * 64, "a" * 65, "a/b", "a b"]


def satz(urteil, was):
    if not urteil.reason.strip():
        sys.exit(f"ABBRUCH: {was} urteilt stumm. Es wird nichts gedruckt.")
    return urteil.reason


def muss_scheitern(ruf, was):
    try:
        ruf()
    except ValueError:
        return
    sys.exit(f"ABBRUCH: {was} sollte scheitern und ging durch. Es wird nichts gedruckt.")


drehungen = []
for ids, keep in DREHUNGEN:
    behalten, gefallen = rotate(ids, keep=keep)
    if gefallen + behalten != list(ids):
        sys.exit(f"ABBRUCH: rotate({ids}, {keep}) erfindet oder verliert Fassungen.")
    drehungen.append({"ids": list(ids), "keep": keep,
                      "kept": behalten, "dropped": gefallen})

drehungen_fehler = []
for ids, keep in DREHUNGEN_FEHLER:
    muss_scheitern(lambda i=ids, k=keep: rotate(i, keep=k), f"rotate({ids}, {keep})")
    drehungen_fehler.append({"ids": list(ids), "keep": keep})

groessen = []
for gespeichert, eingehend, angesagt in GROESSEN:
    u = admit(stored_bytes=gespeichert, incoming_bytes=eingehend,
              declared_shrink=angesagt)
    groessen.append({"stored_bytes": gespeichert, "incoming_bytes": eingehend,
                     "declared_shrink": angesagt,
                     "verdict": {"accepted": u.accepted,
                                 "reason": satz(u, f"admit({gespeichert} -> {eingehend})")}})

groessen_fehler = []
for gespeichert, eingehend in GROESSEN_FEHLER:
    muss_scheitern(lambda s=gespeichert, e=eingehend:
                   admit(stored_bytes=s, incoming_bytes=e),
                   f"admit({gespeichert} -> {eingehend})")
    groessen_fehler.append({"stored_bytes": gespeichert, "incoming_bytes": eingehend})

schreiber = []
for gespeichert, schreibt in SCHREIBER:
    u = write_verdict(stored_format=gespeichert, writer_format=schreibt)
    schreiber.append({"stored_format": gespeichert, "writer_format": schreibt,
                      "verdict": {"accepted": u.accepted,
                                  "reason": satz(u, f"write_verdict({gespeichert}, {schreibt})")}})

gestalten = []
for laden, dienst in GESTALTEN:
    u = serve_verdict(store_schema=laden, service_schema=dienst)
    gestalten.append({"store_schema": laden, "service_schema": dienst,
                      "verdict": {"accepted": u.accepted,
                                  "reason": satz(u, f"serve_verdict({laden}, {dienst})")}})

schriebe = []
for eingaben in SCHRIEBE:
    u = decide(**eingaben)
    schriebe.append({"eingaben": eingaben, "status": u.status,
                     "reason": satz(u, f"decide({eingaben})"),
                     "accepted": u.accepted})

gesehen = {s["status"] for s in schriebe}
fehlend = {STALE, REFUSED, WRITTEN} - gesehen
if fehlend:
    sys.exit(f"ABBRUCH: diese Status kommen in keinem Schrieb vor: {sorted(fehlend)}.")

#: Die Ordnung der Tore steht nur fest, solange es Faelle gibt, die ZWEI Tore
#: zugleich verletzen. Gezaehlt wird das hier, damit die Zusicherung nicht an
#: einem Kommentar haengt.
zweifach = 0
for eingaben in SCHRIEBE:
    verletzt = 0
    if eingaben.get("stored_etag") is not None \
            and eingaben.get("if_match") != eingaben["stored_etag"]:
        verletzt += 1
    if eingaben.get("stored_format") is not None \
            and eingaben["writer_format"] < eingaben["stored_format"]:
        verletzt += 1
    if eingaben.get("stored_bytes") and not eingaben.get("declared_shrink") \
            and eingaben["incoming_bytes"] < eingaben["stored_bytes"] * DEFAULT_SHRINK_FLOOR:
        verletzt += 1
    if verletzt >= 2:
        zweifach += 1
if zweifach < 1:
    sys.exit("ABBRUCH: kein Schrieb verletzt zwei Tore zugleich. Dann ist die Ordnung "
             "der Tore frei und keine Zeile wuerde reissen, wenn sie sich aendert.")

pfade = [{"pfad": p, "document": parse_document_path(p),
          "listing": parse_listing_path(p)} for p in PFADE]
if not any(p["document"] for p in pfade) or not any(p["listing"] for p in pfade):
    sys.exit("ABBRUCH: kein Pfad wird erkannt. Eine Tafel, die nur Abweisungen kennt, "
             "bliebe gruen, wenn nichts mehr erkannt wird.")

bereiche = [{"scope": b, "gueltig": valid_scope(b)} for b in BEREICHE]
if not any(b["gueltig"] for b in bereiche) or all(b["gueltig"] for b in bereiche):
    sys.exit("ABBRUCH: die Bereiche urteilen einstimmig, dann messen sie nichts.")

sys.stdout.write(json.dumps({
    "_was": "Die reine Haelfte von Scheibe 2, erzeugt mit der Python-Fassung. Jeder "
            "Fall traegt die Eingaben und das erwartete Urteil MIT SEINEM SATZ: ein "
            "Urteil ohne Satz waere genau die stille Abweisung, gegen die diese Tore "
            "gebaut sind.",
    "rotate": drehungen,
    "rotate_fehler": drehungen_fehler,
    "admit": groessen,
    "admit_fehler": groessen_fehler,
    "write_verdict": schreiber,
    "serve_verdict": gestalten,
    "decide": schriebe,
    "status": {"STALE": STALE, "REFUSED": REFUSED, "WRITTEN": WRITTEN},
    "paths": pfade,
    "scopes": bereiche,
    "konstanten": {"DEFAULT_SHRINK_FLOOR": DEFAULT_SHRINK_FLOOR, "VERSION": VERSION}},
    indent=2, ensure_ascii=True))
