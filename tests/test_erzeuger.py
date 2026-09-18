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
]


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
        for _, datei, _ in ERZEUGER:
            with self.subTest(datei=datei):
                with open(os.path.join(REPO, datei), encoding="utf-8") as f:
                    text = f.read()
                self.assertEqual([], re.findall(r"[0-9a-f]{32}", text),
                                 f"{datei} trägt eine Kette, die wie ein Zeichen aussieht")


if __name__ == "__main__":
    unittest.main()
