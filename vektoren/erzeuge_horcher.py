# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/horcher.json`, indem der ECHTE Socket gefahren wird.

    python3 vektoren/erzeuge_horcher.py > vektoren/horcher.json

Der Socket ist der Teil des Dienstes, der am wenigsten von selbst auffaellt:
er antwortet im Fehlerfall mit einer ZAHL und sonst nichts. Ein falscher Code
ist auf dem Geraet nicht von einem anderen zu unterscheiden, ausser jemand
haelt die Tabelle dagegen, und diese Datei ist die Tabelle.

Eine Ansage erreicht einen Horcher nur, solange er offen ist. Deshalb laeuft
er hier als Aufgabe: verbinden, warten bis angenommen ist, dann schreiben, und
erst dann trennen. Wer stattdessen der Reihe nach faehrt, misst einen Socket,
der nie etwas gehoert hat, und bekommt eine leere Liste, die gruen aussieht.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""

import asyncio, base64, json, os, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from service.app import App
from service.store import Store
from service.identity import open_identity, key_path, public_of
from core.handshake import HandshakeState
from core.frames import pack, unpack

BESITZER, GEHEIMNIS = "anna", "ein hinreichend langes geheimnis"
FREMDER, FREMD_GEHEIMNIS = "bodo", "ein ganz anderes langes geheimnis"


class Draht:
    """Ein Socket, wie ASGI ihn sieht: eine Schlange herein, eine Liste
    heraus. Die Schlange ist eine echte `asyncio.Queue`, damit der Horcher
    wirklich WARTET statt ein Ende zu lesen, das noch niemand geschickt hat."""

    def __init__(self):
        self.herein = asyncio.Queue()
        self.heraus = []

    async def receive(self):
        return await self.herein.get()

    async def send(self, nachricht):
        self.heraus.append(nachricht)

    def angenommen(self):
        return any(m["type"] == "websocket.accept" for m in self.heraus)

    def geschlossen(self):
        for m in self.heraus:
            if m["type"] == "websocket.close":
                return m.get("code")
        return None


async def bis(pruefung, grenze=2.0):
    """Warten, bis etwas eingetreten ist, aber nie ewig. Eine feste Pause
    waere entweder zu kurz (stille Fehlmessung) oder zu lang (eine Suite, die
    niemand mehr faehrt)."""
    vergangen = 0.0
    while vergangen < grenze:
        if pruefung():
            return True
        await asyncio.sleep(0.002)
        vergangen += 0.002
    return False


async def ruf_http(app, method, path, query=b"", headers=None, body=b""):
    scope = {"type": "http", "method": method, "path": path, "query_string": query,
             "headers": [(k.encode(), v.encode()) for k, v in (headers or {}).items()],
             "client": ("192.0.2.7", 51234)}
    handed, sent = [False], []

    async def receive():
        if handed[0]:
            return {"type": "http.disconnect"}
        handed[0] = True
        return {"type": "http.request", "body": body, "more_body": False}

    async def send(m):
        sent.append(m)

    await app(scope, receive, send)
    start = next(m for m in sent if m["type"] == "http.response.start")
    aus = b"".join(m.get("body", b"") for m in sent if m["type"] == "http.response.body")
    kopf = {k.decode().lower(): v.decode() for k, v in start["headers"]}
    return start["status"], kopf, aus


async def main():
    tmp = tempfile.mkdtemp()
    laden = Store(os.path.join(tmp, "a.sqlite"))
    laden.create_account(BESITZER, GEHEIMNIS)
    laden.create_account(FREMDER, FREMD_GEHEIMNIS)
    zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1")
    fremdes = laden.issue_token(FREMDER, FREMD_GEHEIMNIS, "geraet-x")
    ident = open_identity(laden, key_path(os.path.join(tmp, "a.sqlite")))
    privat, _ = ident

    offen = App(laden, identity=ident)
    zu = App(laden, identity=ident, plain_ways=False)

    faelle = []

    def notiere(name, anwendung, anfrage, draht, ansagen=None, zusatz=None):
        eintrag = {"name": name, "anwendung": anwendung, "anfrage": anfrage,
                   "erwartet": {"angenommen": draht.angenommen()}}
        code = draht.geschlossen()
        if code is not None:
            eintrag["erwartet"]["schliesscode"] = code
        eintrag["erwartet"]["gesendet"] = [m["type"] for m in draht.heraus]
        if ansagen is not None:
            eintrag["erwartet"]["ansagen"] = ansagen
        if zusatz:
            eintrag["erwartet"].update(zusatz)
        faelle.append(eintrag)

    async def nur_verbinden(app, pfad, kopfzeilen, anwendung, name, gezeigt=None):
        """Ein Socket, der abgewiesen wird: verbinden, und sehen, was kommt."""
        d = Draht()
        await d.herein.put({"type": "websocket.connect"})
        await d.herein.put({"type": "websocket.disconnect"})
        scope = {"type": "websocket", "path": pfad,
                 "headers": [(k.encode(), v.encode()) for k, v in kopfzeilen.items()],
                 "client": ("192.0.2.7", 51234)}
        await app(scope, d.receive, d.send)
        notiere(name, anwendung,
                {"path": pfad, "headers": gezeigt if gezeigt is not None else kopfzeilen}, d)
        return d

    # --- Die Abweisungen, alle ohne Sitzung ---
    await nur_verbinden(offen, "/v1/nirgendwo", {}, "offen",
                        "ein anderer Weg ist kein Horcher")
    await nur_verbinden(offen, "/v1/changes", {}, "offen",
                        "ohne Zeichen wird vor dem Annehmen geschlossen")
    await nur_verbinden(offen, "/v1/changes", {"authorization": "Bearer nichtvergeben"}, "offen",
                        "ein unbekanntes Zeichen ebenso")
    await nur_verbinden(zu, "/v1/changes", {"authorization": "Bearer " + zeichen}, "geschlossen",
                        "bei geschlossenen Wegen hilft auch ein gueltiges Zeichen nicht",
                        gezeigt={"authorization": "<zeichen>"})
    await nur_verbinden(zu, "/v1/changes", {"x-maer-session": "gibtsnicht"}, "geschlossen",
                        "eine Sitzung, die es nicht gibt")

    # --- Der offene Horcher, der wirklich etwas hoert ---
    async def offener_horcher(app, anwendung, name, kopfzeilen, gezeigt,
                              schreib_als, erwartet_gehoert, bereich):
        d = Draht()
        scope = {"type": "websocket", "path": "/v1/changes",
                 "headers": [(k.encode(), v.encode()) for k, v in kopfzeilen.items()],
                 "client": ("192.0.2.7", 51234)}
        aufgabe = asyncio.create_task(app(scope, d.receive, d.send))
        await d.herein.put({"type": "websocket.connect"})
        assert await bis(d.angenommen), f"{name}: nie angenommen"
        # Angenommen ist NICHT eingetragen. Wer hier schon schreibt, misst
        # einen Wettlauf: die Ansage faellt, bevor der Horcher auf der Liste
        # steht, und die leere Liste sieht hinterher wie ein Befund aus.
        wer = BESITZER if erwartet_gehoert else FREMDER
        assert await bis(lambda: app.listeners.count(wer) > 0), \
            f"{name}: der Horcher kam nie auf die Liste"

        # Ein EIGENER Bereich je Fall, und der Schrieb muss angenommen sein.
        # Beim ersten Versuch ging der zweite Fall auf denselben Bereich, wurde
        # mit 412 abgewiesen, und damit entstand ueberhaupt keine Ansage: der
        # Fall "erreicht nur den eigenen Besitzer" haette LEER bestanden, ohne
        # je etwas gemessen zu haben.
        status, _, _ = await ruf_http(
            app, "PUT", "/v1/doc/playback", ("scope=" + bereich).encode(),
            {"authorization": "Bearer " + schreib_als, "x-maer-format": "1"},
            json.dumps({"a": 1}).encode())
        assert status == 200, f"{name}: der Schrieb wurde nicht angenommen ({status}), " \
                              f"also beweist dieser Fall nichts"
        if erwartet_gehoert:
            assert await bis(lambda: any(m["type"] == "websocket.send" for m in d.heraus)), \
                f"{name}: nichts gehoert"
        else:
            # Kurz warten, damit ein FALSCH zugestelltes Wort Zeit haette,
            # anzukommen. Ohne die Pause bewiese die leere Liste nichts.
            await asyncio.sleep(0.05)

        await d.herein.put({"type": "websocket.disconnect"})
        await asyncio.wait_for(aufgabe, timeout=2.0)

        gehoert = [json.loads(m["text"]) for m in d.heraus if m["type"] == "websocket.send"]
        notiere(name, anwendung, {"path": "/v1/changes", "headers": gezeigt}, d, ansagen=gehoert)

    await offener_horcher(offen, "offen", "ein Horcher mit Zeichen hoert seine Ansage",
                          {"authorization": "Bearer " + zeichen},
                          {"authorization": "<zeichen>"}, zeichen, True, "geraet-1")
    await offener_horcher(offen, "offen", "eine Ansage erreicht nur ihren eigenen Besitzer",
                          {"authorization": "Bearer " + fremdes},
                          {"authorization": "<fremdes zeichen>"}, zeichen, False, "geraet-2")

    # --- Der versiegelte Horcher ---
    async def sitzung_auf(app):
        rufer = HandshakeState(initiator=True, remote_static=public_of(privat))
        s, kopf, antwort = await ruf_http(app, "POST", "/v1/session", b"", {}, rufer.write_message(b""))
        assert s == 200, s
        rufer.read_message(antwort)
        hin, her = rufer.split()
        return kopf["x-maer-session"], hin, her

    async def sende_versiegelt(app, sname, hin, her, kopf, koerper=b""):
        """Eine Anfrage durch eine Sitzung, und die Antwort zurueck."""
        s, _, antwort = await ruf_http(app, "POST", "/v1/session", b"",
                                       {"x-maer-session": sname},
                                       hin.encrypt_with_ad(b"", pack(kopf, koerper)))
        assert s == 200, f"die versiegelte Anfrage kam nicht durch: {s}"
        innen_kopf, innen_koerper = unpack(her.decrypt_with_ad(b"", antwort))
        return innen_kopf["status"], innen_koerper

    async def versiegelter_horcher(name, erste_gestalt, kopf_zeichen, erwartet_gehoert):
        sname, hin, her = await sitzung_auf(zu)
        d = Draht()
        scope = {"type": "websocket", "path": "/v1/changes",
                 "headers": [(b"x-maer-session", sname.encode())],
                 "client": ("192.0.2.7", 51234)}
        aufgabe = asyncio.create_task(zu(scope, d.receive, d.send))
        await d.herein.put({"type": "websocket.connect"})
        assert await bis(d.angenommen), f"{name}: nie angenommen"

        if erste_gestalt == "rahmen":
            kopf = {"method": "GET", "target": "/v1/changes"}
            if kopf_zeichen:
                kopf["headers"] = {"authorization": "Bearer " + kopf_zeichen}
            await d.herein.put({"type": "websocket.receive",
                                "bytes": hin.encrypt_with_ad(b"", pack(kopf, b""))})
        elif erste_gestalt == "versiegelter_unsinn":
            await d.herein.put({"type": "websocket.receive",
                                "bytes": hin.encrypt_with_ad(b"", b"kein rahmen")})
        elif erste_gestalt == "text_statt_bytes":
            await d.herein.put({"type": "websocket.receive", "text": "kein siegel"})
        elif erste_gestalt == "kein_siegel":
            await d.herein.put({"type": "websocket.receive", "bytes": b"kein siegel"})

        gehoert = []
        if erwartet_gehoert:
            # Erst auf der Liste, dann schreiben. Dasselbe wie oben: das
            # Annehmen des Sockets geht dem Eintragen VORAUS, weil das
            # Eintragen erst nach dem ersten versiegelten Rahmen geschieht.
            assert await bis(lambda: zu.listeners.count(BESITZER) > 0), \
                f"{name}: der versiegelte Horcher kam nie auf die Liste"

            # **Der Schrieb braucht eine EIGENE Sitzung.** Bei geschlossenen
            # Wegen ist ein gewoehnlicher PUT 426, also entstuende gar keine
            # Ansage und der Fall bewiese nichts. Genau deshalb sagt der Dienst,
            # dass ein Geraet ZWEI Handschlaege oeffnet, einen je Richtung: die
            # Zaehler einer Sitzung laufen in der Reihenfolge des Versiegelns,
            # und ein Strom von Ansagen zwischen den Antworten einer
            # Anfrage-Sitzung kaeme aus dieser Reihenfolge heraus.
            zweite, zhin, zher = await sitzung_auf(zu)
            status, _ = await sende_versiegelt(
                zu, zweite, zhin, zher,
                {"method": "PUT", "target": "/v1/doc/playback?scope=geraet-3",
                 "headers": {"authorization": "Bearer " + zeichen, "x-maer-format": "1"}},
                json.dumps({"b": 2}).encode())
            assert status == 200, f"{name}: der Schrieb wurde nicht angenommen ({status}), " \
                                  f"also beweist dieser Fall nichts"
            assert await bis(lambda: any(m["type"] == "websocket.send" for m in d.heraus)), \
                f"{name}: nichts gehoert"
            for m in d.heraus:
                if m["type"] == "websocket.send":
                    gehoert.append(json.loads(her.decrypt_with_ad(b"", m["bytes"])))
        else:
            await bis(lambda: d.geschlossen() is not None)

        await d.herein.put({"type": "websocket.disconnect"})
        try:
            await asyncio.wait_for(aufgabe, timeout=2.0)
        except asyncio.TimeoutError:
            aufgabe.cancel()

        anfrage = {"path": "/v1/changes", "headers": {"x-maer-session": "<sitzung>"},
                   "erste_gestalt": erste_gestalt}
        if erste_gestalt == "rahmen":
            anfrage["erster_rahmen"] = {"method": "GET", "target": "/v1/changes"}
            if kopf_zeichen:
                anfrage["erster_rahmen"]["headers"] = {"authorization": "<zeichen>"}
        notiere(name, "geschlossen", anfrage, d,
                ansagen=gehoert if erwartet_gehoert else None,
                zusatz={"ansage_ist_versiegelt": erwartet_gehoert})

    await versiegelter_horcher("versiegelt: erst der Rahmen mit Zeichen, dann die Ansage",
                               "rahmen", zeichen, True)
    await versiegelter_horcher("versiegelt: der erste Rahmen ohne Zeichen",
                               "rahmen", None, False)
    await versiegelter_horcher("versiegelt: der erste Rahmen ist kein Rahmen",
                               "versiegelter_unsinn", None, False)
    await versiegelter_horcher("versiegelt: das erste Wort ist Text statt Bytes",
                               "text_statt_bytes", None, False)
    await versiegelter_horcher("versiegelt: das erste Wort ist nicht versiegelt",
                               "kein_siegel", None, False)

    laden.close()
    print(json.dumps({
        "was": "Der Horcher in service/app.py, an der Anwendung selbst gemessen.",
        "wie": "Jeder Fall wurde als echter ASGI-Socket gefahren; der offene Horcher laeuft "
               "als Aufgabe, damit eine Ansage ihn erreichen kann, solange er offen ist.",
        "anwendungen": {
            "offen": "App(store, identity=...) mit offenen alten Wegen",
            "geschlossen": "App(store, identity=..., plain_ways=False)"},
        "hinweis_werte": "<zeichen>, <fremdes zeichen> und <sitzung> stehen fuer echte Werte; "
                         "Werte stehen nie in dieser Datei.",
        "faelle": faelle}, indent=2, ensure_ascii=False))


asyncio.run(main())
