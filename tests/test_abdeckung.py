# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Der Zaun über die Zuordnungen auf Fall-Ebene.

Welcher Rust-Zeuge einen Python-Fall beantwortet, ist ein **Urteil** und keine
Messung; es gibt kein Muster, das es ausrechnet. Genau deshalb steht darüber
ein Zaun, denn eine Handliste ohne Gegenprobe ist die Sorte Dokument, die
niemand mehr nachsieht.

Sechs Fragen, und alle sperren in BEIDE Richtungen:

1. Jeder Fall der genannten Module kommt genau einmal vor. Die Liste der Fälle
   wird aus dem Quelltext GELESEN, nicht in der Zuordnung geführt.
2. Kein Fall ist erfunden.
3. **Jeder genannte Gegenpart existiert wirklich** (Datei, und wo ein Name
   dabeisteht, der Name). Ohne das könnte die Zuordnung auf Zeugen zeigen, die
   es nie gab, und sähe trotzdem vollständig aus.
4. Jedes „kein Gegenpart" trägt seinen Grund.
5. Jede Art ist eine der vier.
6. Wer einen Gegenpart behauptet, nennt ihn.

**Der Zaun trägt seit Teil 5 ZWEI Zuordnungen**, und die Liste unten ist die
einzige Stelle, an der eine dritte einzutragen wäre. Sie als Handliste zu
führen wäre der Fehler, den dieser Zaun sonst gerade verhindert: läge sie
nicht hier, ginge eine Zuordnung, die niemand mehr fährt, still mit.
"""

import ast
import json
import os
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HIER)

#: (Datei der Zuordnung, die Module, deren Fälle sie beantwortet).
#: Welche MODULE zu einer Gruppe gehören, ist eine Entscheidung; welche FAELLE
#: darin liegen, wird gemessen.
ZUORDNUNGEN = [
    ("108-faelle.json",
     ["test_app", "test_cli", "test_session_store",
      "test_session_paths", "test_tunnel", "test_rehearsal"]),
    ("126-faelle.json",
     ["test_admission", "test_formats", "test_frames", "test_handshake",
      "test_identity", "test_restore", "test_sessions", "test_versions",
      "test_writes"]),
]


def faelle_im_quelltext(module):
    aus = set()
    for m in module:
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


def zuordnung(datei):
    with open(os.path.join(REPO, "abdeckung", datei), encoding="utf-8") as f:
        return json.load(f)["faelle"]


class DieZuordnungZeigtAufWirklichkeit(unittest.TestCase):

    def test_jeder_fall_der_module_kommt_genau_einmal_vor(self):
        for datei, module in ZUORDNUNGEN:
            with self.subTest(zuordnung=datei):
                im_quelltext = faelle_im_quelltext(module)
                genannt = [f["fall"] for f in zuordnung(datei)]
                doppelt = sorted({f for f in genannt if genannt.count(f) > 1})
                self.assertEqual([], doppelt, f"doppelt in {datei}: {doppelt}")
                fehlen = sorted(im_quelltext - set(genannt))
                self.assertEqual([], fehlen,
                                 f"Faelle ohne Zeile in {datei}: {fehlen}")

    def test_kein_fall_ist_erfunden(self):
        for datei, module in ZUORDNUNGEN:
            with self.subTest(zuordnung=datei):
                im_quelltext = faelle_im_quelltext(module)
                erfunden = sorted(f["fall"] for f in zuordnung(datei)
                                  if f["fall"] not in im_quelltext)
                self.assertEqual([], erfunden,
                                 f"{datei} nennt Faelle, die es im Quelltext nicht gibt "
                                 "(umbenannt oder entfernt): " + ", ".join(erfunden))

    def test_jeder_genannte_gegenpart_existiert_wirklich(self):
        """Ohne diese Frage koennte eine Zuordnung auf Zeugen zeigen, die es nie
        gab, und saehe dabei vollstaendig aus."""
        fehlend = []
        for datei, _ in ZUORDNUNGEN:
            for fall in zuordnung(datei):
                ziel = fall.get("gegenpart", "")
                if not ziel:
                    continue
                # Ein Eintrag kann mehrere Stellen nennen; geprueft wird jede,
                # die wie ein Pfad aussieht.
                for stueck in ziel.replace(" und ", " ").split():
                    if "/" not in stueck:
                        continue
                    pfadteil, _, name = stueck.partition("::")
                    pfad = os.path.join(REPO, pfadteil)
                    if not os.path.exists(pfad):
                        fehlend.append(f"{datei} / {fall['fall']}: "
                                       f"die Datei {pfadteil} gibt es nicht")
                        continue
                    if name:
                        with open(pfad, encoding="utf-8") as f:
                            text = f.read()
                        if f"fn {name}" not in text:
                            fehlend.append(f"{datei} / {fall['fall']}: "
                                           f"{pfadteil} hat kein {name}")
        self.assertEqual([], fehlend, "\n".join(fehlend))

    def test_ein_grund_steht_auch_im_fliesstext_der_zuordnung(self):
        """Die Gegenprobe zur Frage davor, und sie hat einen eigenen Anlass.

        Ein Gegenpart wird geprueft, wenn er wie ein Pfad aussieht. Ein Grund
        darf ebenfalls einen nennen, und dann ist es dieselbe Zusicherung an
        einer Stelle, die niemand prueft: die Zeile sieht belegt aus, und die
        Datei dahinter kann seit Monaten weg sein.
        """
        fehlend = []
        for datei, _ in ZUORDNUNGEN:
            for fall in zuordnung(datei):
                for stueck in fall.get("grund", "").replace(",", " ").split():
                    if not stueck.startswith("tests/") and not stueck.startswith("werkzeug/"):
                        continue
                    pfadteil, _, name = stueck.partition("::")
                    pfad = os.path.join(REPO, pfadteil)
                    if not os.path.exists(pfad):
                        fehlend.append(f"{datei} / {fall['fall']}: "
                                       f"der Grund nennt {pfadteil}, das es nicht gibt")
                        continue
                    if name:
                        with open(pfad, encoding="utf-8") as f:
                            if f"fn {name}" not in f.read():
                                fehlend.append(f"{datei} / {fall['fall']}: "
                                               f"der Grund nennt {name}, das {pfadteil} "
                                               "nicht hat")
        self.assertEqual([], fehlend, "\n".join(fehlend))

    def test_jede_luecke_traegt_ihren_grund(self):
        for datei, _ in ZUORDNUNGEN:
            with self.subTest(zuordnung=datei):
                ohne = sorted(f["fall"] for f in zuordnung(datei)
                              if f["art"] == "keiner" and len(f.get("grund", "")) < 30)
                self.assertEqual([], ohne,
                                 "eine Luecke ohne Grund ist eine, die niemand mehr "
                                 "nachsieht: " + ", ".join(ohne))

    def test_jede_art_ist_eine_der_vier(self):
        erlaubt = {"tafel", "zeuge", "draht", "keiner"}
        for datei, _ in ZUORDNUNGEN:
            with self.subTest(zuordnung=datei):
                fremd = sorted({f["art"] for f in zuordnung(datei)} - erlaubt)
                self.assertEqual([], fremd, f"unbekannte Art in {datei}: {fremd}")

    def test_wer_einen_gegenpart_hat_nennt_ihn(self):
        """Die andere Richtung: `tafel`, `zeuge` und `draht` ohne Ziel waeren
        eine Zusicherung ohne Gegenstand."""
        for datei, _ in ZUORDNUNGEN:
            with self.subTest(zuordnung=datei):
                leer = sorted(f["fall"] for f in zuordnung(datei)
                              if f["art"] != "keiner" and not f.get("gegenpart"))
                self.assertEqual([], leer,
                                 "Gegenpart behauptet, aber keiner genannt: " + ", ".join(leer))

    def test_jede_zuordnung_wird_wirklich_gefahren(self):
        """Der Zaun ueber dem Zaun.

        Die Liste `ZUORDNUNGEN` ist eine Handliste, und eine Handliste
        vergisst. Liegt in `abdeckung/` eine Zuordnung, die hier nicht steht,
        wird sie von keiner der Fragen oben beruehrt und altert still.
        """
        ordner = os.path.join(REPO, "abdeckung")
        auf_der_platte = {n for n in os.listdir(ordner) if n.endswith("-faelle.json")}
        gefahren = {datei for datei, _ in ZUORDNUNGEN}
        vergessen = sorted(auf_der_platte - gefahren)
        self.assertEqual([], vergessen,
                         "diese Zuordnung faehrt kein Zaun: " + ", ".join(vergessen))


if __name__ == "__main__":
    unittest.main()
