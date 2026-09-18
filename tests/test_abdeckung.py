# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Der Zaun über die Zuordnung auf Fall-Ebene.

Welcher Rust-Zeuge einen Python-Fall beantwortet, ist ein **Urteil** und keine
Messung; es gibt kein Muster, das es ausrechnet. Genau deshalb steht darüber
ein Zaun, denn eine Handliste ohne Gegenprobe ist die Sorte Dokument, die
niemand mehr nachsieht.

Vier Fragen, und alle vier sperren in BEIDE Richtungen:

1. Jeder Fall der sechs Module kommt genau einmal vor. Die Liste der Fälle
   wird aus dem Quelltext GELESEN, nicht in der Zuordnung geführt.
2. Kein Fall ist erfunden.
3. **Jeder genannte Gegenpart existiert wirklich** (Datei, und wo ein Name
   dabeisteht, der Name). Ohne das könnte die Zuordnung auf Zeugen zeigen, die
   es nie gab, und sähe trotzdem vollständig aus.
4. Jedes „kein Gegenpart" trägt seinen Grund.
"""

import ast
import json
import os
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HIER)
ZUORDNUNG = os.path.join(REPO, "abdeckung", "108-faelle.json")

#: Die sechs Module der Gruppe, die auf Entscheide wartete. Als Liste, weil
#: die Gruppe eine Entscheidung ist; welche FAELLE darin liegen, wird gemessen.
MODULE = ["test_app", "test_cli", "test_session_store",
          "test_session_paths", "test_tunnel", "test_rehearsal"]


def faelle_im_quelltext():
    aus = set()
    for m in MODULE:
        pfad = os.path.join(HIER, f"{m}.py")
        with open(pfad, encoding="utf-8") as f:
            baum = ast.parse(f.read())
        for k in baum.body:
            if not isinstance(k, ast.ClassDef):
                continue
            for f in k.body:
                if isinstance(f, (ast.FunctionDef, ast.AsyncFunctionDef)) \
                        and f.name.startswith("test_"):
                    aus.add(f"{m}.{k.name}.{f.name}")
    return aus


def zuordnung():
    with open(ZUORDNUNG, encoding="utf-8") as f:
        return json.load(f)["faelle"]


class DieZuordnungZeigtAufWirklichkeit(unittest.TestCase):

    def test_jeder_fall_der_sechs_module_kommt_genau_einmal_vor(self):
        im_quelltext = faelle_im_quelltext()
        genannt = [f["fall"] for f in zuordnung()]
        doppelt = sorted({f for f in genannt if genannt.count(f) > 1})
        self.assertEqual([], doppelt, f"doppelt in der Zuordnung: {doppelt}")
        fehlen = sorted(im_quelltext - set(genannt))
        self.assertEqual([], fehlen,
                         f"Faelle ohne Zeile in der Zuordnung: {fehlen}")

    def test_kein_fall_ist_erfunden(self):
        im_quelltext = faelle_im_quelltext()
        erfunden = sorted(f["fall"] for f in zuordnung() if f["fall"] not in im_quelltext)
        self.assertEqual([], erfunden,
                         "die Zuordnung nennt Faelle, die es im Quelltext nicht gibt "
                         "(umbenannt oder entfernt): " + ", ".join(erfunden))

    def test_jeder_genannte_gegenpart_existiert_wirklich(self):
        """Ohne diese Frage koennte die Zuordnung auf Zeugen zeigen, die es nie
        gab, und saehe dabei vollstaendig aus."""
        fehlend = []
        for fall in zuordnung():
            ziel = fall.get("gegenpart", "")
            if not ziel:
                continue
            # Ein Eintrag kann mehrere Stellen nennen; geprueft wird jede, die
            # wie ein Pfad aussieht.
            for stueck in ziel.replace(" und ", " ").split():
                if "/" not in stueck:
                    continue
                datei, _, name = stueck.partition("::")
                pfad = os.path.join(REPO, datei)
                if not os.path.exists(pfad):
                    fehlend.append(f"{fall['fall']}: die Datei {datei} gibt es nicht")
                    continue
                if name:
                    with open(pfad, encoding="utf-8") as f:
                        text = f.read()
                    if f"fn {name}" not in text:
                        fehlend.append(f"{fall['fall']}: {datei} hat kein {name}")
        self.assertEqual([], fehlend, "\n".join(fehlend))

    def test_jede_luecke_traegt_ihren_grund(self):
        ohne = sorted(f["fall"] for f in zuordnung()
                      if f["art"] == "keiner" and len(f.get("grund", "")) < 30)
        self.assertEqual([], ohne,
                         "eine Luecke ohne Grund ist eine, die niemand mehr nachsieht: "
                         + ", ".join(ohne))

    def test_jede_art_ist_eine_der_vier(self):
        erlaubt = {"tafel", "zeuge", "draht", "keiner"}
        fremd = sorted({f["art"] for f in zuordnung()} - erlaubt)
        self.assertEqual([], fremd, f"unbekannte Art: {fremd}")

    def test_wer_einen_gegenpart_hat_nennt_ihn(self):
        """Die andere Richtung: `tafel`, `zeuge` und `draht` ohne Ziel waeren
        eine Zusicherung ohne Gegenstand."""
        leer = sorted(f["fall"] for f in zuordnung()
                      if f["art"] != "keiner" and not f.get("gegenpart"))
        self.assertEqual([], leer, "Gegenpart behauptet, aber keiner genannt: " + ", ".join(leer))


if __name__ == "__main__":
    unittest.main()
