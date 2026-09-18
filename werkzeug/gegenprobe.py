# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Spielt `vektoren/wege.json` gegen einen LAUFENDEN Dienst ab.

    python3 werkzeug/gegenprobe.py --gegen http://127.0.0.1:8081 --zeichen <wert>

Die Vektordatei traegt jede Anfrage und jede erwartete Antwort. Sie laesst
sich also abspielen, und zwar gegen JEDE Umsetzung: die Fassung, die sie
erzeugt hat, und jede andere, die dasselbe behauptet. Genau das ist die
Abnahme, die eine Uebersetzung braucht.

**Warum das mehr ist als die Zeugen.** Die Rust-Zeugen rufen die
Entscheidungen direkt und beweisen, dass sie stimmen. Hier geht alles ueber
eine echte Leitung: durch das Lesen des Koerpers, durch die Kopfzeilen, durch
die Antwortzeile. Fehler, die zwischen Entscheidung und Draht sitzen, sind nur
so zu finden.

**Regel 7 gilt.** Was diese Kopie nicht fahren KANN, wird ausgewiesen
uebersprungen, mit Grund, und nie als bestanden gezaehlt. Zwei Sorten sind
das: Faelle einer Anwendung ohne statischen Schluessel (das Erzeugnis hat
immer einen) und Faelle mit dem Schalter `innen` (den setzt die
Sitzungsschicht, er ist von aussen nicht erreichbar; die Tunnelfaelle decken
ihn).

Nur die Standardbibliothek, ausser dem Repo selbst.
"""

import argparse, base64, json, os, sys, urllib.error, urllib.request

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from core.frames import pack, unpack
from core.handshake import HandshakeState


def hole(url, method="GET", kopfzeilen=None, koerper=None):
    anfrage = urllib.request.Request(url, method=method, data=koerper)
    for k, v in (kopfzeilen or {}).items():
        anfrage.add_header(k, v)
    try:
        with urllib.request.urlopen(anfrage, timeout=15) as antwort:
            return antwort.status, dict(antwort.headers), antwort.read()
    except urllib.error.HTTPError as fehler:
        return fehler.code, dict(fehler.headers), fehler.read()


def vergleiche(name, erwartet, status, kopf, koerper):
    """Was NICHT stimmt, als Liste. Leer heisst gleich."""
    kopf = {k.lower(): v for k, v in kopf.items()}
    abweichungen = []
    if status != erwartet["status"]:
        abweichungen.append(f"Status {status} statt {erwartet['status']}")
    for k, v in (erwartet.get("kopfzeilen") or {}).items():
        if kopf.get(k) != v:
            abweichungen.append(f"Kopfzeile {k}: {kopf.get(k)!r} statt {v!r}")
    try:
        gelesen = json.loads(koerper)
    except ValueError:
        gelesen = None
    if "grund" in erwartet:
        hat = gelesen.get("reason") if isinstance(gelesen, dict) else None
        if hat != erwartet["grund"]:
            abweichungen.append(f"Satz {hat!r} statt {erwartet['grund']!r}")
    if "felder" in erwartet:
        hat = sorted(gelesen) if isinstance(gelesen, dict) else None
        if hat != erwartet["felder"]:
            abweichungen.append(f"Felder {hat} statt {erwartet['felder']}")
    if "koerper" in erwartet and gelesen != erwartet["koerper"]:
        abweichungen.append(f"Koerper {gelesen!r} statt {erwartet['koerper']!r}")
    return abweichungen


def fahre_wege(basis, zeichen, welche, faelle):
    gleich, ab, weg = 0, [], []
    for fall in faelle:
        if fall["anwendung"] != welche:
            continue
        an = fall["anfrage"]
        if an.get("innen"):
            weg.append((fall["name"], "der Schalter `innen` wird von der Sitzungsschicht "
                                      "gesetzt und ist von aussen nicht erreichbar; "
                                      "die Tunnelfaelle decken ihn"))
            continue
        url = basis + an["path"] + ("?" + an["query"] if an.get("query") else "")
        kopfzeilen = {k: (f"Bearer {zeichen}" if v == "<zeichen>" else v)
                      for k, v in (an.get("headers") or {}).items()}
        koerper = base64.b64decode(an["koerper_b64"]) if an.get("koerper_b64") else None
        status, kopf, gab = hole(url, an["method"], kopfzeilen, koerper)
        schlecht = vergleiche(fall["name"], fall["erwartet"], status, kopf, gab)
        if schlecht:
            ab.append((fall["name"], schlecht))
        else:
            gleich += 1
    return gleich, ab, weg


def fahre_tunnel(basis, zeichen, tunnel):
    """Eine echte Sitzung, von der Client-Seite, in der Reihenfolge der Datei.

    Die Reihenfolge ist nicht Geschmack: die Zaehler der Chiffre ruecken mit
    jedem Siegel, und ein Fall, der ausgelassen wird, verschiebt alle
    folgenden.
    """
    status, kopf, schluessel = hole(basis + "/v1/key")
    if status != 200:
        return 0, [("der Handschlag", [f"/v1/key gab {status}"])], []
    fremd = base64.b64decode(json.loads(schluessel)["key"])

    rufer = HandshakeState(initiator=True, remote_static=fremd)
    status, kopf, antwort = hole(basis + "/v1/session", "POST",
                                 {"Content-Type": "application/octet-stream"},
                                 rufer.write_message(b""))
    if status != 200:
        return 0, [("der Handschlag", [f"/v1/session gab {status}"])], []
    sname = {k.lower(): v for k, v in kopf.items()}["x-maer-session"]
    rufer.read_message(antwort)
    hin, her = rufer.split()

    gleich, ab, weg = 0, [], []
    for fall in tunnel:
        gestalt = fall["gestalt"]
        if gestalt == "rahmen":
            kopfrahmen = json.loads(json.dumps(fall["rahmen"]))
            if "headers" in kopfrahmen:
                kopfrahmen["headers"] = {
                    k: (f"Bearer {zeichen}" if v == "<zeichen>" else v)
                    for k, v in kopfrahmen["headers"].items()}
            inhalt = (base64.b64decode(fall["rahmen_koerper_b64"])
                      if fall.get("rahmen_koerper_b64") else b"")
            gesendet = hin.encrypt_with_ad(b"", pack(kopfrahmen, inhalt))
        elif gestalt == "versiegelter_unsinn":
            gesendet = hin.encrypt_with_ad(b"", b"kein rahmen")
        elif gestalt == "kein_siegel":
            gesendet = b"kein siegel"
        elif gestalt == "fremde_sitzung":
            gesendet = b"egal"
        else:
            weg.append((fall["name"], f"unbekannte Gestalt {gestalt}"))
            continue

        an_wen = fall.get("fremder_name", sname)
        status, _, antwort = hole(basis + "/v1/session", "POST",
                                  {"X-Maer-Session": an_wen,
                                   "Content-Type": "application/octet-stream"},
                                  gesendet)
        schlecht = []
        if status != fall["aussen"]["status"]:
            schlecht.append(f"aussen {status} statt {fall['aussen']['status']}")
        elif "grund" in fall["aussen"]:
            hat = json.loads(antwort).get("reason")
            if hat != fall["aussen"]["grund"]:
                schlecht.append(f"aussen Satz {hat!r} statt {fall['aussen']['grund']!r}")
        if not schlecht and "innen" in fall:
            innen_kopf, innen_koerper = unpack(her.decrypt_with_ad(b"", antwort))
            schlecht = vergleiche(fall["name"], fall["innen"], innen_kopf["status"],
                                  innen_kopf.get("headers", {}), innen_koerper)
        if schlecht:
            ab.append((fall["name"], schlecht))
        else:
            gleich += 1
    return gleich, ab, weg


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--gegen", required=True, help="Adresse des laufenden Dienstes")
    p.add_argument("--zeichen", required=True, help="ein gueltiges Bearer-Zeichen")
    p.add_argument("--welche", default="mit_kennung",
                   choices=["mit_kennung", "geschlossen"],
                   help="welche Anwendung dieser Dienst ist")
    p.add_argument("--nur-tunnel", action="store_true")
    p.add_argument("--kein-tunnel", action="store_true")
    a = p.parse_args()

    wurzel = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    with open(os.path.join(wurzel, "vektoren", "wege.json"), encoding="utf-8") as f:
        tafel = json.load(f)

    basis = a.gegen.rstrip("/")
    gleich, ab, weg = 0, [], []

    # Gutfall zuerst, und er identifiziert den GEGENSTAND: ohne ihn misst man
    # womoeglich einen fremden Dienst auf demselben Port.
    status, _, gab = hole(basis + "/v1/key")
    if status != 200:
        print(f"ABBRUCH: {basis}/v1/key gab {status}, hier antwortet nichts Brauchbares")
        return 2
    print(f"  Gutfall: der Dienst nennt sich {json.loads(gab)['fingerprint']}")

    if not a.nur_tunnel:
        g, x, w = fahre_wege(basis, a.zeichen, a.welche, tafel["faelle"])
        gleich += g; ab += x; weg += w
    if not a.kein_tunnel and a.welche == "geschlossen":
        g, x, w = fahre_tunnel(basis, a.zeichen, tafel["tunnel"])
        gleich += g; ab += x; weg += w

    # Die Faelle einer Anwendung, die es als Erzeugnis nicht gibt.
    for fall in tafel["faelle"]:
        if fall["anwendung"] == "ohne_kennung":
            weg.append((fall["name"],
                        "das Erzeugnis hat immer eine Kennung; eine Anwendung ohne "
                        "statischen Schluessel gibt es nur als Bibliothek"))

    print(f"\n  gleich: {gleich}   abweichend: {len(ab)}   ausgewiesen uebersprungen: {len(weg)}")
    for name, gruende in ab:
        print(f"    ABWEICHUNG  {name}")
        for g in gruende:
            print(f"                {g}")
    for name, grund in weg:
        print(f"    uebersprungen  {name}\n                   {grund}")
    return 1 if ab else 0


if __name__ == "__main__":
    sys.exit(main())
