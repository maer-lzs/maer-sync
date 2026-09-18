# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/befehle.json`, indem die ECHTE Python-CLI gefahren wird.

    python3 vektoren/erzeuge_befehle.py > vektoren/befehle.json

Der Betreiber sieht vom Dienst zuerst diese Befehle, und was er sieht, ist ein
**Ausgangscode und ein Satz**. Beides gehoert gemessen, nicht nachgebaut: ein
Ausgang, der sich um eins verschiebt, macht aus einer geglueckten Sicherung
eine gescheiterte, und der Zeitplan des Betreibers alarmiert auf die falsche.

**Was maskiert wird, und warum genau das.** Abdruecke und Pfade wechseln je
Lauf; sie stehen als `<abdruck>` und `<pfad>` da, damit die Datei byte-stabil
bleibt. Der Ausgang und der Satz drumherum bleiben roh, denn sie sind der
Gegenstand.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""

import json, os, re, subprocess, sys, tempfile

WURZEL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

ABDRUCK = re.compile(r"\b[0-9A-HJKMNP-TV-Z]{4}(?:-[0-9A-HJKMNP-TV-Z]{4}){5}\b")


def saeubern(text, tmp):
    text = text.replace(tmp, "<pfad>")
    text = ABDRUCK.sub("<abdruck>", text)
    # Zeilen, die eine Zeitangabe oder eine Groesse tragen, wuerden die Datei
    # unstabil machen; sie werden NICHT geraten, sondern nur dort ersetzt, wo
    # sie als Muster erkennbar sind.
    text = re.sub(r"\b\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}[.,]?\d*\b", "<zeit>", text)
    # Und die Gestalt, die `account-remove` in den Namen seiner Sicherung
    # schreibt: `20260919-004832`. Der erste Anlauf kannte nur die Form mit
    # Bindestrichen, und die Datei war deshalb NICHT byte-stabil, was erst
    # der Vergleich zweier Laeufe zeigte.
    text = re.sub(r"\b\d{8}-\d{6}\b", "<zeitmarke>", text)
    return text.strip()


def fahre(tmp, args, eingabe=None, umgebung=None):
    u = dict(os.environ, MAER_SYNC_DB=os.path.join(tmp, "a.sqlite"))
    u.pop("MAER_SYNC_KEY", None)
    if umgebung:
        u.update(umgebung)
    gab = subprocess.run([sys.executable, "-m", "service"] + args,
                         cwd=WURZEL, env=u, capture_output=True,
                         input=(eingabe.encode() if eingabe is not None else None),
                         timeout=120)
    return (gab.returncode,
            saeubern(gab.stdout.decode("utf-8", "replace"), tmp),
            saeubern(gab.stderr.decode("utf-8", "replace"), tmp))


def main():
    faelle = []
    lauf = [0]

    def neuer_lauf():
        lauf[0] += 1

    def fall(name, args, eingabe=None, tmp=None, umgebung=None, vorher=None,
             vorbereitung=None):
        code, aus, fehler = fahre(tmp, args, eingabe, umgebung)
        # Auch der BEFEHL wird maskiert, nicht nur die Ausgabe: er traegt den
        # Pfad des Wegwerf-Verzeichnisses, und der wechselt je Lauf. Beim
        # ersten Versuch stand er roh in der Datei, und die war damit nicht
        # byte-stabil, ohne dass es auffiel.
        eintrag = {"name": name, "befehl": [a.replace(tmp, "<pfad>") for a in args],
                   "erwartet": {"ausgang": code}}
        if eingabe is not None:
            # **Maskiert wird nur ein Geheimnis, das eines IST.** Der leere
            # Fall bleibt leer: ihn als `<geheimnis>` zu schreiben machte aus
            # »leeres Geheimnis« beim Abspielen »gueltiges Geheimnis«, und der
            # Fall pruefte dann das Gegenteil dessen, wofuer er dasteht.
            # Gefangen hat es der Gutfall, der die Tafel gegen die Fassung
            # faehrt, aus der sie stammt.
            eintrag["auf_der_eingabe"] = "<geheimnis>" if eingabe else ""
        if aus:
            eintrag["erwartet"]["ausgabe"] = aus
        if fehler:
            eintrag["erwartet"]["fehlerausgabe"] = fehler
            # **Wer den Satz geschrieben hat, gehoert dazu.** Bei einem
            # fehlenden Argument antwortet `argparse`, nicht der Dienst, und
            # sein Wortlaut nennt den Python-Aufruf. Eine zweite Umsetzung
            # kann und soll das nicht nachbauen; sie muss denselben AUSGANG
            # treffen. Ohne diese Marke muesste ein Zeuge den Unterschied
            # stillschweigend uebergehen, und stillschweigend ist die Gestalt,
            # in der Pruefungen verschwinden.
            if fehler.startswith("usage:"):
                eintrag["erwartet"]["satz_von"] = "argparse"
        # Dieselbe Regel fuer die Rueckkehr-Probe: die zwei Anfaenge unten
        # sind UNSERE Saetze, was dahinter steht, schreibt der
        # Datenbanktreiber. Zwei Treiber schreiben zwei Saetze
        # (»unable to open database file« gegen »unable to open database:
        # <pfad>«), und das ist kein Unterschied im Verhalten, sondern einer
        # in der Herkunft des Textes. Verglichen wird deshalb bis
        # einschliesslich unseres Doppelpunkts, und diese Zeile ist der Grund.
        for unserer in ("the copy did not get written:", "the copy did not open as a store:"):
            if unserer in aus:
                eintrag["erwartet"]["satz_bis"] = unserer
        eintrag["lauf"] = lauf[0]
        if vorbereitung:
            eintrag["vorbereitung"] = vorbereitung
        if vorher:
            eintrag["vorbedingung"] = vorher
        faelle.append(eintrag)

    # --- key: der erste Befehl, den ein Betreiber ueberhaupt fahren kann ---
    neuer_lauf()
    with tempfile.TemporaryDirectory() as tmp:
        fall("key auf leerem Speicher macht keinen", ["key"], tmp=tmp,
             vorher="kein Speicher, kein Schluessel")
    neuer_lauf()
    with tempfile.TemporaryDirectory() as tmp:
        fall("key --new legt einen an", ["key", "--new"], tmp=tmp,
             vorher="kein Speicher, kein Schluessel")
        fall("key zeigt ihn danach", ["key"], tmp=tmp,
             vorher="der Schluessel aus dem Lauf davor liegt")
        fall("key --new UND --adopt zugleich", ["key", "--new", "--adopt"], tmp=tmp)

    # --- account ---
    neuer_lauf()
    with tempfile.TemporaryDirectory() as tmp:
        fall("account ohne Namen", ["account"], tmp=tmp)
        fall("account legt eines an", ["account", "anna"],
             eingabe="ein hinreichend langes geheimnis", tmp=tmp)
        fall("account zweimal derselbe Name", ["account", "anna"],
             eingabe="ein hinreichend langes geheimnis", tmp=tmp)
        fall("account mit leerem Geheimnis", ["account", "bodo"], eingabe="", tmp=tmp)
        fall("account-remove nimmt es weg", ["account-remove", "anna"], tmp=tmp)
        fall("account-remove auf eines, das es nicht gibt",
             ["account-remove", "niemand"], tmp=tmp)

    def lege_ein_dokument(tmp):
        """Ein Schriftstueck in den Speicher, ueber die Bibliothek.

        Die CLI hat keinen Befehl dafuer, und ohne Dokument kann die
        Rueckkehr-Probe NICHTS messen: sie vergleicht Groessen je Fassung.
        Sie sagt dann auch genau das (`not measured`) und gibt 1 zurueck,
        statt Erfolg zu behaupten. Genau deshalb braucht die Tafel beide
        Faelle, den leeren und den vollen.
        """
        import sys as _s
        _s.path.insert(0, WURZEL)
        from service.store import Store
        laden = Store(os.path.join(tmp, "a.sqlite"))
        laden.create_account("carla", "ein hinreichend langes geheimnis")
        laden.write("carla", "playback", b'{"a": 1}', 1, scope="g1")
        laden.close()

    # --- backup, der Befehl mit der eingebauten Probe ---
    neuer_lauf()
    with tempfile.TemporaryDirectory() as tmp:
        fall("backup ohne Speicher", ["backup", os.path.join(tmp, "kopie.sqlite")], tmp=tmp,
             vorher="kein Speicher")
        fall("ein Konto anlegen, damit es etwas zu sichern gibt",
             ["account", "anna"], eingabe="ein hinreichend langes geheimnis", tmp=tmp)
        fall("backup mit Speicher", ["backup", os.path.join(tmp, "kopie.sqlite")], tmp=tmp,
             vorher="ein Speicher mit einem Konto")
        fall("backup ohne Ziel", ["backup"], tmp=tmp)
    neuer_lauf()
    with tempfile.TemporaryDirectory() as tmp:
        lege_ein_dokument(tmp)
        fall("backup mit einem Dokument, die Probe kann messen",
             ["backup", os.path.join(tmp, "kopie.sqlite")], tmp=tmp,
             vorbereitung="ein_dokument",
             vorher="ein Speicher mit einem Konto UND einem Schriftstueck")
    neuer_lauf()
    with tempfile.TemporaryDirectory() as tmp:
        fall("backup in ein Verzeichnis, das es nicht gibt",
             ["backup", os.path.join(tmp, "gibtsnicht", "k.sqlite")], tmp=tmp)

    print(json.dumps({
        "was": "Die Betreiber-Befehle: welcher Ausgang und welcher Satz.",
        "wie": "Die echte Python-CLI gefahren (python3 -m service ...), je Fall ein "
               "eigener Speicher; gemessen, nicht nachgebaut.",
        "hinweis_maskierung": "<abdruck>, <pfad>, <zeit> und <geheimnis> stehen fuer "
                              "Werte, die je Lauf wechseln oder nicht in eine Datei "
                              "gehoeren. Ausgang und Satz sind roh.",
        "faelle": faelle}, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
