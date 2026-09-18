# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Spielt `vektoren/befehle.json` gegen ein ERZEUGNIS ab.

    python3 werkzeug/befehlsprobe.py --erzeugnis ziel/.../maer-sync

Die Tafel ist an der Python-CLI gemessen. Hier wird sie gegen eine andere
Umsetzung gefahren, und verglichen wird **der Ausgang immer** und der Satz
dort, wo er unserer ist.

**Was ausdruecklich NICHT verglichen wird, und warum das dasteht.** Bei einem
fehlenden Argument antwortet auf der Python-Seite `argparse`, mit einem
Wortlaut, der den Python-Aufruf nennt. Eine zweite Umsetzung kann das nicht
nachbauen, ohne zu luegen, wie man sie aufruft. Diese Faelle tragen in der
Tafel `satz_von: argparse`; bei ihnen wird nur der Ausgang verglichen, und
dieser Absatz ist der Grund. Ohne die Marke muesste der Zeuge den Unterschied
stillschweigend uebergehen, und stillschweigend ist die Gestalt, in der
Pruefungen verschwinden.

Die Faelle teilen sich Speicher: `account` zweimal ist nur dann ein Fall, wenn
das erste `account` davor lief. Die Tafel nennt deshalb je Fall ihren `lauf`,
und der wird hier nachgestellt.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""

import argparse, collections, json, os, re, subprocess, sys, tempfile

WURZEL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ABDRUCK = re.compile(r"\b[0-9A-HJKMNP-TV-Z]{4}(?:-[0-9A-HJKMNP-TV-Z]{4}){5}\b")


def saeubern(text, tmp):
    text = text.replace(tmp, "<pfad>")
    text = ABDRUCK.sub("<abdruck>", text)
    text = re.sub(r"\b\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}[.,]?\d*\b", "<zeit>", text)
    text = re.sub(r"\b\d{8}-\d{6}\b", "<zeitmarke>", text)
    return text.strip()


def lege_ein_dokument(tmp):
    sys.path.insert(0, WURZEL)
    from service.store import Store
    laden = Store(os.path.join(tmp, "a.sqlite"))
    laden.create_account("carla", "ein hinreichend langes geheimnis")
    laden.write("carla", "playback", b'{"a": 1}', 1, scope="g1")
    laden.close()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--erzeugnis", required=True)
    a = p.parse_args()
    binaer = os.path.abspath(a.erzeugnis)
    if not os.access(binaer, os.X_OK):
        print(f"  ABBRUCH: {binaer} ist nicht ausfuehrbar")
        return 2

    with open(os.path.join(WURZEL, "vektoren", "befehle.json"), encoding="utf-8") as f:
        tafel = json.load(f)

    nach_lauf = collections.OrderedDict()
    for fall in tafel["faelle"]:
        nach_lauf.setdefault(fall["lauf"], []).append(fall)

    gleich, ab, nur_ausgang = 0, [], 0
    for lauf, faelle in nach_lauf.items():
        with tempfile.TemporaryDirectory() as tmp:
            for fall in faelle:
                if fall.get("vorbereitung") == "ein_dokument":
                    lege_ein_dokument(tmp)
                args = [x.replace("<pfad>", tmp) for x in fall["befehl"]]
                u = dict(os.environ, MAER_SYNC_DB=os.path.join(tmp, "a.sqlite"))
                u.pop("MAER_SYNC_KEY", None)
                eingabe = fall.get("auf_der_eingabe")
                if eingabe == "<geheimnis>":
                    eingabe = "ein hinreichend langes geheimnis"
                gab = subprocess.run([binaer] + args, cwd=tmp, env=u,
                                     capture_output=True, timeout=120,
                                     input=(eingabe.encode() if eingabe is not None else None))
                will = fall["erwartet"]
                schlecht = []
                if gab.returncode != will["ausgang"]:
                    schlecht.append(f"Ausgang {gab.returncode} statt {will['ausgang']}")
                if will.get("satz_von") == "argparse":
                    nur_ausgang += 1
                else:
                    bis = will.get("satz_bis")
                    for strom, schluessel in (("stdout", "ausgabe"), ("stderr", "fehlerausgabe")):
                        hat = saeubern(getattr(gab, strom).decode("utf-8", "replace"), tmp)
                        soll = will.get(schluessel, "")
                        if bis and bis in soll:
                            # Bis einschliesslich unseres Doppelpunkts. Was
                            # dahinter steht, schreibt der Datenbanktreiber,
                            # und zwei Treiber schreiben zwei Saetze.
                            hat, soll = hat.split(bis)[0] + bis, soll.split(bis)[0] + bis
                        if hat != soll:
                            schlecht.append(f"{strom}: {hat!r} statt {soll!r}")
                if schlecht:
                    ab.append((fall["name"], schlecht))
                else:
                    gleich += 1

    print(f"\n  gleich: {gleich}   abweichend: {len(ab)}   "
          f"(davon {nur_ausgang} nur am Ausgang gemessen, Satz von argparse)")
    for name, gruende in ab:
        print(f"    ABWEICHUNG  {name}")
        for g in gruende:
            print(f"                {g}")
    return 1 if ab else 0


if __name__ == "__main__":
    sys.exit(main())
