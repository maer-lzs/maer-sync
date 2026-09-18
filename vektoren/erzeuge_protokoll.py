# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/protokoll.json`: welche Zeile welche Abweisung hinterlaesst.

    python3 vektoren/erzeuge_protokoll.py > vektoren/protokoll.json

**Warum das eine eigene Tafel braucht.** Regel 3 des Regelwerks sagt, kein
Fehler verschwindet spurlos. Ohne diese Zeilen ist der Fall, fuer den der
Dienst gebaut ist (ein Geraet schreibt leer, wird abgewiesen, der Klient
schluckt die 409), fuer den Betreiber unsichtbar, solange er dauert. Was eine
Zeile trotzdem NIE nennen darf, ist ein Zeichen, ein Geheimnis oder ein
Koerper: das Protokoll ist die Stelle, an der ein Wert in Dateien wandert, die
jemand anders liest.

Gemessen wird die NACHRICHT, nicht die Zeile: Zeitstempel, Stufe und Name des
Protokolls setzt Pythons `logging` aus seinem Format
(`%(asctime)s %(levelname)s %(name)s %(message)s`), und die Stufe steht hier
als eigenes Feld daneben.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""

import asyncio, base64, json, logging, os, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from service.app import App
from service.store import Store
from service.identity import open_identity, key_path, public_of
from core.handshake import HandshakeState

BESITZER, GEHEIMNIS = "anna", "ein hinreichend langes geheimnis"
QUELLE = ("10.0.0.7", 51234)


class Mitschrift(logging.Handler):
    """Faengt die Nachrichten, nicht die fertigen Zeilen."""

    def __init__(self):
        super().__init__(level=logging.INFO)
        self.zeilen = []

    def emit(self, satz):
        self.zeilen.append((satz.levelname, satz.getMessage()))


async def ruf(app, method, pfad, frage=b"", kopfzeilen=None, koerper=b"", abbruch=False):
    scope = {"type": "http", "method": method, "path": pfad, "query_string": frage,
             "headers": [(k.encode(), v.encode()) for k, v in (kopfzeilen or {}).items()],
             "client": QUELLE}
    zustand = {"gesendet": False}

    async def receive():
        if zustand["gesendet"]:
            return {"type": "http.disconnect"}
        zustand["gesendet"] = True
        if abbruch:
            # Mitten im Koerper weg: ein halbes Dokument, kein kleineres.
            return {"type": "http.request", "body": koerper, "more_body": True}
        return {"type": "http.request", "body": koerper, "more_body": False}

    async def send(_):
        pass

    await app(scope, receive, send)


async def socket(app, pfad, kopfzeilen=None):
    scope = {"type": "websocket", "path": pfad,
             "headers": [(k.encode(), v.encode()) for k, v in (kopfzeilen or {}).items()],
             "client": QUELLE}
    herein = [{"type": "websocket.connect"}, {"type": "websocket.disconnect"}]

    async def receive():
        return herein.pop(0) if herein else {"type": "websocket.disconnect"}

    async def send(_):
        pass

    await app(scope, receive, send)


async def main():
    mit = Mitschrift()
    log = logging.getLogger("maer.sync")
    log.addHandler(mit)
    log.setLevel(logging.INFO)

    tmp = tempfile.mkdtemp()
    laden = Store(os.path.join(tmp, "a.sqlite"))
    laden.create_account(BESITZER, GEHEIMNIS)
    zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1")
    ident = open_identity(laden, key_path(os.path.join(tmp, "a.sqlite")))
    app = App(laden, identity=ident)
    mit.zeilen.clear()

    faelle = []

    async def fall(name, tun, was_nie_drinsteht=()):
        mit.zeilen.clear()
        await tun()
        eintrag = {"name": name, "zeilen": [
            {"stufe": s, "nachricht": n} for s, n in mit.zeilen]}
        if was_nie_drinsteht:
            eintrag["darf_nicht_enthalten"] = list(was_nie_drinsteht)
        faelle.append(eintrag)

    mit_zeichen = {"authorization": "Bearer " + zeichen}

    await fall("ohne Zeichen an ein Schriftstueck",
               lambda: ruf(app, "GET", "/v1/doc/playback"))
    await fall("mit einem Zeichen, das niemandem gehoert",
               lambda: ruf(app, "GET", "/v1/doc/playback", b"",
                           {"authorization": "Bearer unsinnszeichen"}),
               ["unsinnszeichen"])
    await fall("eine Anmeldung mit falschem Geheimnis",
               lambda: ruf(app, "POST", "/v1/token", b"", {},
                           json.dumps({"owner": BESITZER, "secret": "falschgeheim"}).encode()),
               ["falschgeheim"])
    await fall("eine Anmeldung, die glueckt",
               lambda: ruf(app, "POST", "/v1/token", b"", {},
                           json.dumps({"owner": BESITZER, "secret": GEHEIMNIS,
                                       "device": "geraet-9"}).encode()),
               [GEHEIMNIS])
    await fall("ein Handschlag, der keiner ist",
               lambda: ruf(app, "POST", "/v1/session", b"", {}, b"kein handschlag"))
    await fall("ein unlesbarer Schrieb",
               lambda: ruf(app, "PUT", "/v1/doc/playback", b"scope=g1",
                           {**mit_zeichen, "x-maer-format": "1"}, b"nicht json"),
               ["nicht json"])
    await fall("ein Schrieb, der mitten im Koerper abbricht",
               lambda: ruf(app, "PUT", "/v1/doc/playback", b"scope=g1",
                           {**mit_zeichen, "x-maer-format": "1", "content-length": "99"},
                           b'{"a": 1}', abbruch=True))
    # Erst ein Stand, damit das Tor etwas abzuweisen hat.
    await ruf(app, "PUT", "/v1/doc/playback", b"scope=g2",
              {**mit_zeichen, "x-maer-format": "1"}, json.dumps({"a": 1}).encode())
    await fall("ein angenommener Schrieb sagt NICHTS",
               lambda: ruf(app, "PUT", "/v1/doc/playback", b"scope=g3",
                           {**mit_zeichen, "x-maer-format": "1"},
                           json.dumps({"a": 2}).encode()))
    await fall("ein Schrieb, den das Tor abweist",
               lambda: ruf(app, "PUT", "/v1/doc/playback", b"scope=g2",
                           {**mit_zeichen, "x-maer-format": "1"}, b""))
    await fall("ein Horcher ohne Zeichen",
               lambda: socket(app, "/v1/changes"))
    await fall("ein Horcher mit fremdem Zeichen",
               lambda: socket(app, "/v1/changes",
                              {"authorization": "Bearer anderesunsinnszeichen"}),
               ["anderesunsinnszeichen"])

    laden.close()
    print(json.dumps({
        "was": "Welche Zeile welche Abweisung hinterlaesst, und was nie darin steht.",
        "wie": "Die echte Anwendung gefahren und der Protokoll-Handler mitgeschrieben; "
               "gemessen wird die NACHRICHT, nicht die fertige Zeile.",
        "gestalt": "Pythons logging setzt davor "
                   "'%(asctime)s %(levelname)s %(name)s ' mit dem Namen 'maer.sync'.",
        "hinweis": "darf_nicht_enthalten nennt je Fall die Werte, die in KEINER Zeile "
                   "auftauchen duerfen: ein Zeichen, ein Geheimnis, ein Koerper.",
        "faelle": faelle}, indent=2, ensure_ascii=False))


asyncio.run(main())
