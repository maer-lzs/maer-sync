# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Jede Vektordatei, die einen Erzeuger hat, wird neu gezogen und gegen die
eingecheckte gehalten.

**Eine Vektordatei ohne Erzeuger ist eine Behauptung.** Sie sagt, der Dienst
antworte so, und niemand kann das nachrechnen: wer wissen will, ob sie noch
stimmt, müsste sie von Hand gegen die Anwendung halten, und genau das tut
niemand. Mit Erzeuger ist die Frage eine Zeile.

Dieser Zeuge fällt in zwei Richtungen, und beide sind der Zweck:

- **Der Dienst hat sich geändert**, und die Datei hält einen Stand fest, den es
  nicht mehr gibt. Dann ist die Datei neu zu ziehen, und der Unterschied ist zu
  lesen, bevor das geschieht: er ist die Liste dessen, was sich am Verhalten
  geändert hat.
- **Der Erzeuger hat sich geändert** und zieht jetzt etwas anderes. Dann ist zu
  prüfen, ob er noch dasselbe misst.

Die Fallzahl wird eigens geprüft. Ein Erzeuger, der stillschweigend weniger
Fälle zieht, erzeugte sonst eine kleinere Datei, die weiter zu sich selbst
passt, und der Zaun bliebe grün, während die Abdeckung schrumpft.
"""

import json
import os
import subprocess
import sys
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HIER)

#: Erzeuger und ihre Datei, und wie viele Fälle darin mindestens stehen müssen.
#: Die Zahlen sind der Stand bei der Aufnahme; sie dürfen wachsen, nie fallen.
ERZEUGER = [
    ("vektoren/erzeuge_wege.py", "vektoren/wege.json", {"faelle": 38, "tunnel": 11}),
    ("vektoren/erzeuge_horcher.py", "vektoren/horcher.json", {"faelle": 12}),
    ("vektoren/erzeuge_befehle.py", "vektoren/befehle.json", {"faelle": 16}),
    ("vektoren/erzeuge_protokoll.py", "vektoren/protokoll.json", {"faelle": 11}),
    # Die zwei Zuordnungen auf Fall-Ebene sind ebenfalls erzeugte Dateien, und
    # sie standen bis Teil 5 ausserhalb dieses Zauns. Eine von Hand nachgezogene
    # `108-faelle.json` haette dann von ihrem Erzeuger abweichen koennen, ohne
    # dass etwas reisst: der Zaun in `test_abdeckung.py` haelt die Datei gegen
    # den Quelltext, nie gegen das Skript, das sie schreibt.
    ("abdeckung/zuordnung.py", "abdeckung/108-faelle.json", {"faelle": 108}),
    ("abdeckung/zuordnung-126.py", "abdeckung/126-faelle.json", {"faelle": 126}),
    # Die sieben Vektordateien der Scheiben 1 bis 3, die bis zum 19.09.2026 ohne
    # Erzeuger dalagen. Sie ziehen aus `core/` und fassen keinen Speicher an.
    ("vektoren/erzeuge_abdruecke.py", "vektoren/abdruecke.json", {"faelle": 4}),
    ("vektoren/erzeuge_rahmen.py", "vektoren/rahmen.json",
     {"gueltig": 4, "ungueltig": 4}),
    ("vektoren/erzeuge_rueckkehr.py", "vektoren/rueckkehr.json", {"faelle": 10}),
    ("vektoren/erzeuge_kopf_und_schluessel.py", "vektoren/kopf-und-schluessel.json",
     {"request_of": 5, "request_of_fehler": 6, "reply_of": 4, "reply_of_fehler": 5,
      "decide": 5}),
    ("vektoren/erzeuge_sitzungen.py", "vektoren/sitzungen.json",
     {"standing": 16, "give_way": 6, "sweep": 4, "room_for_one": 3}),
    ("vektoren/erzeuge_zeichen.py", "vektoren/zeichen.json",
     {"token_digest": 6, "caller_for": 12, "gespeicherte_formen": 2,
      "unlesbare_formen": 8}),
    ("vektoren/erzeuge_schreiben.py", "vektoren/schreiben.json",
     {"rotate": 7, "admit": 14, "write_verdict": 6, "serve_verdict": 5, "decide": 11,
      "paths": 14, "scopes": 8}),
]

#: Die Erzeuger, die einen SPEICHER anfassen: sie legen ein Konto an und lassen
#: sich ein Zeichen geben, und genau dieses Zeichen darf nie in ihrer Datei
#: landen.
#:
#: Die uebrigen stehen hier NICHT, und der Grund ist kein Nachlassen: sie ziehen
#: nur aus `core/`, es gibt bei ihnen kein Zeichen, das leaken koennte, und ihre
#: Dateien tragen mit Absicht lange Hex-Ketten (Schluessel, Abdruecke, Rahmen).
#: Die Kettenprobe wuerde dort auf Material anschlagen, das dort hingehoert, und
#: eine Probe, die am falschen Gegenstand anschlaegt, wird abgeschaltet statt
#: gelesen. Was an ihrer Stelle gemessen wird, steht eine Klasse tiefer:
#: `test_kein_erzeuger_ohne_speicher_kann_an_ein_zeichen_kommen`.
ZIEHT_EIN_ZEICHEN = [
    "vektoren/wege.json",
    "vektoren/horcher.json",
    "vektoren/befehle.json",
    "vektoren/protokoll.json",
]

#: Die Erzeuger, die keinen Speicher anfassen duerfen. Fuer sie gilt die
#: staerkere Aussage: nicht „es ist kein Zeichen drin", sondern „es kann keines
#: geben".
OHNE_SPEICHER = [
    "vektoren/erzeuge_abdruecke.py",
    "vektoren/erzeuge_rahmen.py",
    "vektoren/erzeuge_rueckkehr.py",
    "vektoren/erzeuge_kopf_und_schluessel.py",
    "vektoren/erzeuge_sitzungen.py",
    "vektoren/erzeuge_zeichen.py",
    "vektoren/erzeuge_schreiben.py",
]

#: Vektordateien, die KEINEN Erzeuger haben, mit dem Grund dafür. Die Liste
#: darf nicht wachsen, ohne dass jemand einen Grund hinschreibt, und genau
#: das prüft `test_jede_vektordatei_hat_einen_erzeuger_oder_einen_grund`.
OHNE_ERZEUGER = {
    "vektoren/noise-nk.json":
        "veroeffentlichte Vektoren, unveraendert von aussen. Sie nachzuziehen hiesse, "
        "sie durch unsere eigene Rechnung zu ersetzen, und damit waere genau das weg, "
        "was sie wertvoll macht: dass jemand Dritter sagt, was richtig ist.",
}


#: Erzeuger, die einen SERVER brauchen, also eine venv mit uvicorn. Sie stehen
#: getrennt, weil ihre Bedingung eine andere ist: ohne venv kann diese Kopie die
#: Frage nicht stellen, und dann wird ausgewiesen uebersprungen (Regel 7), nie
#: bestanden.
MIT_SERVER = [
    ("vektoren/erzeuge_abweisung.py", "vektoren/abweisung.json", {"faelle": 4}),
]


def venv_python():
    """Das Python der venv aus der README, oder nichts."""
    weg = os.path.join(REPO, ".venv", "bin", "python")
    return weg if os.access(weg, os.X_OK) else None


class JederErzeugerZiehtSeineDatei(unittest.TestCase):
    def test_die_datei_kommt_byte_gleich_wieder_heraus(self):
        for skript, datei, _ in ERZEUGER:
            with self.subTest(erzeuger=skript):
                gezogen = subprocess.run(
                    [sys.executable, os.path.join(REPO, skript)],
                    cwd=REPO, capture_output=True, timeout=300)
                self.assertEqual(0, gezogen.returncode,
                                 f"{skript} lief nicht durch: "
                                 f"{gezogen.stderr.decode('utf-8', 'replace')[-500:]}")
                with open(os.path.join(REPO, datei), "rb") as f:
                    eingecheckt = f.read()
                self.assertEqual(
                    eingecheckt, gezogen.stdout,
                    f"{datei} ist nicht mehr das, was {skript} zieht. Entweder hat "
                    f"sich der Dienst geändert, dann ist die Datei neu zu ziehen "
                    f"und der Unterschied zu lesen, oder der Erzeuger misst etwas "
                    f"anderes. Neu ziehen: python3 {skript} > {datei}")

    def test_kein_erzeuger_zieht_stillschweigend_weniger_faelle(self):
        for skript, datei, mindestens in ERZEUGER:
            with self.subTest(erzeuger=skript):
                with open(os.path.join(REPO, datei), encoding="utf-8") as f:
                    gelesen = json.load(f)
                for abschnitt, zahl in mindestens.items():
                    self.assertIn(abschnitt, gelesen, f"{datei} hat keinen Abschnitt {abschnitt}")
                    self.assertGreaterEqual(
                        len(gelesen[abschnitt]), zahl,
                        f"{datei}: der Abschnitt {abschnitt} ist von {zahl} auf "
                        f"{len(gelesen[abschnitt])} geschrumpft")

    def test_das_echte_zeichen_steht_in_keiner_gezogenen_datei(self):
        """Der Erzeuger legt ein Konto an und lässt sich ein Zeichen geben. Das
        darf nie in der Datei landen, auch nicht abgekürzt."""
        import re
        gepruefte = 0
        for _, datei, _ in ERZEUGER:
            if datei not in ZIEHT_EIN_ZEICHEN:
                continue
            gepruefte += 1
            with self.subTest(datei=datei):
                with open(os.path.join(REPO, datei), encoding="utf-8") as f:
                    text = f.read()
                self.assertEqual([], re.findall(r"[0-9a-f]{32}", text),
                                 f"{datei} trägt eine Kette, die wie ein Zeichen aussieht")
        # Ohne diese Zeile könnte ZIEHT_EIN_ZEICHEN leerlaufen und die Probe wäre
        # grün, ohne eine Datei angesehen zu haben.
        self.assertEqual(len(ZIEHT_EIN_ZEICHEN), gepruefte,
                         "nicht jede Datei mit Speicher wurde geprüft")

    def test_kein_erzeuger_ohne_speicher_kann_an_ein_zeichen_kommen(self):
        """Die stärkere Aussage für die sieben, die keinen Speicher anfassen.

        Bei ihnen lautet die Frage nicht „steht ein Zeichen in der Datei",
        sondern „kann es überhaupt eines geben". Ein Zeichen entsteht in
        `service/store.py`; wer den Dienst nicht importiert, hat keines, und
        dann ist die Kettenprobe auf ihrer Datei eine Probe am falschen
        Gegenstand, die auf Schlüsselmaterial anschlägt.
        """
        import re
        for skript in OHNE_SPEICHER:
            with self.subTest(erzeuger=skript):
                with open(os.path.join(REPO, skript), encoding="utf-8") as f:
                    quelle = f.read()
                treffer = re.findall(r"^\s*(?:from|import)\s+(service[.\w]*)",
                                     quelle, re.M)
                self.assertEqual([], treffer,
                                 f"{skript} zieht aus dem Dienst ({treffer}); dann kann "
                                 "ein Zeichen entstehen und die Zusicherung hier ist weg")


class JedeVektordateiIstErfasst(unittest.TestCase):
    """Der Zaun ueber den Zaeunen: eine Datei, die in keiner Liste steht.

    **Warum es ihn gibt.** Bis zum 19.09.2026 lagen sieben Vektordateien ohne
    Erzeuger da, und keine Probe hat das je gesagt; es stand als Satz in der
    README und wurde von dort aus alt. Eine Liste von Hand vergisst, eine
    Schleife ueber das Verzeichnis kann es nicht.
    """

    def vektordateien(self):
        ordner = os.path.join(REPO, "vektoren")
        return {f"vektoren/{n}" for n in os.listdir(ordner) if n.endswith(".json")}

    def test_jede_vektordatei_hat_einen_erzeuger_oder_einen_grund(self):
        gezogen = {datei for _, datei, _ in ERZEUGER + MIT_SERVER}
        erfasst = gezogen | set(OHNE_ERZEUGER)
        verwaist = sorted(self.vektordateien() - erfasst)
        self.assertEqual([], verwaist,
                         "diese Vektordateien zieht kein Erzeuger, und es steht auch "
                         "kein Grund dabei: " + ", ".join(verwaist))
        # Die andere Richtung: ein Grund fuer eine Datei, die es nicht mehr gibt,
        # ist eine Ausnahme, die niemand mehr braucht und die stehen bleibt.
        tot = sorted(set(OHNE_ERZEUGER) - self.vektordateien())
        self.assertEqual([], tot, "Grund ohne Datei: " + ", ".join(tot))

    def test_jeder_grund_ist_einer(self):
        duenn = sorted(d for d, g in OHNE_ERZEUGER.items() if len(g) < 60)
        self.assertEqual([], duenn,
                         "ein Grund, der nichts erklaert, ist eine Ausnahme mit "
                         "Etikett: " + ", ".join(duenn))

    def test_jede_vektordatei_steht_in_der_readme_tabelle(self):
        """Die Tabelle erklaert, wie stark jede Datei ist. Fehlt eine Zeile,
        liest man die Abstufung und haelt sie fuer vollzaehlig."""
        import re
        with open(os.path.join(REPO, "vektoren", "README.md"), encoding="utf-8") as f:
            tabelle = set(re.findall(r"^\| `([a-z0-9._-]+\.json)`", f.read(), re.M))
        fehlend = sorted(n.split("/")[-1] for n in self.vektordateien()
                         if n.split("/")[-1] not in tabelle)
        self.assertEqual([], fehlend,
                         "diese Dateien haben keine Zeile in der Staerke-Tabelle: "
                         + ", ".join(fehlend))


class DieErzeugerMitServer(unittest.TestCase):
    """Dieselbe Frage, aber sie braucht eine venv mit uvicorn.

    **Ohne venv wird UEBERSPRUNGEN, nicht bestanden.** Eine leere Antwort
    behauptet eine Messung, die niemand gemacht hat; genau dafuer steht Regel 7.
    Wer sie fahren will: `python3 -m venv .venv` und die Anleitung in der README.
    """

    def test_die_datei_kommt_byte_gleich_wieder_heraus(self):
        py = venv_python()
        if py is None:
            self.skipTest("keine .venv im Repo, also kein Server, also keine Messung "
                          "moeglich; siehe README")
        for skript, datei, _ in MIT_SERVER:
            with self.subTest(erzeuger=skript):
                gezogen = subprocess.run(
                    [sys.executable, os.path.join(REPO, skript), "--python", py],
                    cwd=REPO, capture_output=True, timeout=300)
                self.assertEqual(0, gezogen.returncode,
                                 f"{skript} lief nicht durch: "
                                 f"{gezogen.stderr.decode('utf-8', 'replace')[-500:]}")
                with open(os.path.join(REPO, datei), "rb") as f:
                    eingecheckt = f.read()
                self.assertEqual(eingecheckt, gezogen.stdout,
                                 f"{datei} ist nicht mehr das, was {skript} zieht")

    def test_kein_erzeuger_zieht_stillschweigend_weniger_faelle(self):
        """Diese Haelfte braucht KEINEN Server: sie liest die eingecheckte Datei."""
        for skript, datei, mindestens in MIT_SERVER:
            with self.subTest(erzeuger=skript):
                with open(os.path.join(REPO, datei), encoding="utf-8") as f:
                    gelesen = json.load(f)
                for abschnitt, zahl in mindestens.items():
                    self.assertGreaterEqual(
                        len(gelesen[abschnitt]), zahl,
                        f"{datei}: {abschnitt} ist von {zahl} auf "
                        f"{len(gelesen[abschnitt])} geschrumpft")

    def test_der_erzeuger_haelt_an_statt_leer_zu_messen(self):
        """Fail-closed: ohne brauchbares Python bricht er ab, statt eine Datei zu
        schreiben, die zu sich selbst passt und nichts sagt."""
        for skript, _, _ in MIT_SERVER:
            with self.subTest(erzeuger=skript):
                gab = subprocess.run(
                    [sys.executable, os.path.join(REPO, skript),
                     "--python", "/gibt/es/nicht"],
                    cwd=REPO, capture_output=True, timeout=60)
                self.assertNotEqual(0, gab.returncode, "er haette anhalten muessen")
                self.assertEqual(b"", gab.stdout, "er hat trotzdem etwas geschrieben")


if __name__ == "__main__":
    unittest.main()
