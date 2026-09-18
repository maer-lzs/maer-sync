# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/wege.json`, indem die ECHTE Anwendung gefahren wird.

    python3 vektoren/erzeuge_wege.py > vektoren/wege.json

Nichts hier behauptet, was `service/app.py` tut. Jeder Fall wird gerufen und
die Antwort aufgeschrieben. Das ist der ganze Sinn der Datei: beim ersten
Schreiben hielt ich den Rahmenkopf fuer `{method, path, query}`, er ist aber
`{method, target}`, und sieben Tunnelfaelle kamen als "this is not a request"
zurueck, bis dieser Erzeuger es zeigte. Eine von Hand geschriebene Erwartung
waere genauso falsch gewesen wie die Umsetzung und damit still gruen.

**Die Ausgabe ist byte-stabil.** Was zufaellig ist, steht nicht drin: der
Sitzungsname gar nicht, der Schluessel und das Zeichen nur als Feldname, das
gueltige Bearer-Zeichen nur als `<zeichen>`. Kein Zeitstempel. `tests/
test_erzeuger.py` faehrt diesen Erzeuger und haelt das Ergebnis byte-gleich
gegen die eingecheckte Datei; weicht es ab, ist entweder der Dienst geaendert
worden oder die Datei ist alt, und beides gehoert bemerkt.

**Maskiert wird der WERT, nicht der Name der Kopfzeile.** `Bearer
nichtvergeben` und `Basic irgendwas` tragen kein Geheimnis und stehen im
Klartext. Der erste Versuch maskierte jede `authorization`-Zeile, und damit
wurden drei verschiedene Faelle zu einem: der Zeuge setzte das echte Zeichen
ein, wo ein unbekanntes stehen sollte.

Nur die Standardbibliothek, ausser dem Repo selbst.
"""
import asyncio, base64, json, os, sys, tempfile

# Das Repo selbst, von dieser Datei aus. Kein fester Pfad: der Erzeuger
# liegt jetzt IM Repo und muss dort laufen, wo das Repo liegt.
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from service.app import App
from service.store import Store
from service.identity import open_identity, key_path, public_of
from core.handshake import HandshakeState
from core.frames import pack, unpack

BESITZER, GEHEIMNIS = "anna", "ein hinreichend langes geheimnis"


async def ruf(app, method, path, query=b"", headers=None, body=b"", inside=False):
    scope = {"type": "http", "method": method, "path": path, "query_string": query,
             "headers": [(k.encode(), v.encode()) for k, v in (headers or {}).items()],
             "client": ("192.0.2.7", 51234)}
    if inside:
        scope["maer.inside"] = True
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
    out = b"".join(m.get("body", b"") for m in sent if m["type"] == "http.response.body")
    kopf = {k.decode().lower(): v.decode() for k, v in start["headers"]}
    return start["status"], kopf, out


def erwartet(status, kopf, body, zufall=False):
    e = {"status": status}
    interessant = {k: v for k, v in kopf.items()
                   if k in ("www-authenticate", "content-type", "etag", "x-maer-format")}
    if interessant:
        e["kopfzeilen"] = interessant
    try:
        gelesen = json.loads(body)
    except ValueError:
        e["koerper_b64"] = base64.b64encode(body).decode()
        return e
    if zufall:
        e["felder"] = sorted(gelesen)
    elif isinstance(gelesen, dict) and set(gelesen) == {"reason"}:
        e["grund"] = gelesen["reason"]
    else:
        e["koerper"] = gelesen
    return e


async def main():
    tmp = tempfile.mkdtemp()
    laden = Store(os.path.join(tmp, "a.sqlite"))
    laden.create_account(BESITZER, GEHEIMNIS)
    zeichen = laden.issue_token(BESITZER, GEHEIMNIS, "geraet-1")
    mit = {"authorization": "Bearer " + zeichen}

    # Drei Anwendungen: ohne Kennung, mit Kennung, und mit geschlossenen alten Wegen.
    ohne_kennung = App(laden)
    ident = open_identity(laden, key_path(os.path.join(tmp, "a.sqlite")))
    mit_kennung = App(laden, identity=ident)
    geschlossen = App(laden, identity=ident, plain_ways=False)

    # Ein Schrieb, damit es etwas zu lesen gibt.
    await ruf(mit_kennung, "PUT", "/v1/doc/playback", b"scope=geraet-1",
              {**mit, "x-maer-format": "1"}, json.dumps({"a": 1}).encode())

    faelle = []
    gehoert = []

    async def horcher(nachricht):
        gehoert.append(json.loads(nachricht["text"]))

    for a in (mit_kennung, geschlossen):
        a.listeners.add(BESITZER, horcher)

    async def fall(name, app, welche, *a, zufall=False, **k):
        s, kopf, b = await ruf(app, *a, **k)
        anfrage = {"method": a[0], "path": a[1]}
        if len(a) > 2 and a[2]:
            anfrage["query"] = a[2].decode()
        kopfzeilen = k.get("headers") or (a[3] if len(a) > 3 else None) or {}
        # Das Zeichen selbst ist ein Wert: es steht nie in der Datei.
        gezeigt = {kk: ("<zeichen>" if vv == "Bearer " + zeichen else vv)
                   for kk, vv in kopfzeilen.items()}
        if gezeigt:
            anfrage["headers"] = gezeigt
        if k.get("body"):
            anfrage["koerper_b64"] = base64.b64encode(k["body"]).decode()
        if k.get("inside"):
            anfrage["innen"] = True
        eintrag = {"name": name, "anwendung": welche,
                   "anfrage": anfrage, "erwartet": erwartet(s, kopf, b, zufall)}
        if gehoert:
            eintrag["ansage"] = list(gehoert)
            gehoert.clear()
        faelle.append(eintrag)

    # --- Die Gesundheit steht VOR allem, auch vor der Methode ---
    await fall("gesundheit GET", mit_kennung, "mit_kennung", "GET", "/v1/health")
    await fall("gesundheit POST antwortet genauso", mit_kennung, "mit_kennung", "POST", "/v1/health")
    await fall("gesundheit auch bei geschlossenen Wegen", geschlossen, "geschlossen", "GET", "/v1/health")

    # --- Der Schluessel ---
    await fall("Schluessel ohne Kennung", ohne_kennung, "ohne_kennung", "GET", "/v1/key")
    await fall("Schluessel mit Kennung", mit_kennung, "mit_kennung", "GET", "/v1/key", zufall=True)
    await fall("Schluessel mit falscher Methode", mit_kennung, "mit_kennung", "POST", "/v1/key")
    await fall("Schluessel auch bei geschlossenen Wegen", geschlossen, "geschlossen", "GET", "/v1/key", zufall=True)

    # --- Die Sitzung ---
    await fall("Sitzung ohne Kennung", ohne_kennung, "ohne_kennung", "POST", "/v1/session")
    await fall("Sitzung mit falscher Methode", mit_kennung, "mit_kennung", "GET", "/v1/session")
    await fall("Sitzung, kein Handschlag", mit_kennung, "mit_kennung", "POST", "/v1/session",
               b"", {}, body=b"kein handschlag")

    # --- Das Zeichen ---
    await fall("Zeichen mit falscher Methode", mit_kennung, "mit_kennung", "GET", "/v1/token")
    await fall("Zeichen ohne JSON", mit_kennung, "mit_kennung", "POST", "/v1/token", b"", {}, body=b"{")
    await fall("Zeichen ohne Geheimnis", mit_kennung, "mit_kennung", "POST", "/v1/token", b"", {},
               body=json.dumps({"owner": BESITZER}).encode())
    await fall("Zeichen, falsches Geheimnis", mit_kennung, "mit_kennung", "POST", "/v1/token", b"", {},
               body=json.dumps({"owner": BESITZER, "secret": "falsch"}).encode())
    await fall("Zeichen, Konto gibt es nicht", mit_kennung, "mit_kennung", "POST", "/v1/token", b"", {},
               body=json.dumps({"owner": "niemand", "secret": "falsch"}).encode())
    await fall("Zeichen, richtig", mit_kennung, "mit_kennung", "POST", "/v1/token", b"", {},
               body=json.dumps({"owner": BESITZER, "secret": GEHEIMNIS}).encode(), zufall=True)

    # --- Die Tuer: ohne Zeichen kommt niemand an ein Schriftstueck ---
    await fall("Schriftstueck ohne Zeichen", mit_kennung, "mit_kennung", "GET", "/v1/doc/playback")
    await fall("Schriftstueck mit unbekanntem Zeichen", mit_kennung, "mit_kennung", "GET",
               "/v1/doc/playback", b"", {"authorization": "Bearer nichtvergeben"})
    await fall("Schriftstueck mit kaputter Kopfzeile", mit_kennung, "mit_kennung", "GET",
               "/v1/doc/playback", b"", {"authorization": "Basic irgendwas"})

    # --- Reihenfolge: unbekannter Weg mit Zeichen ist 404, nicht 400 ---
    await fall("unbekannter Weg", mit_kennung, "mit_kennung", "GET", "/v1/nirgendwo", b"", mit)
    await fall("unbekannter Weg mit kaputtem Bereich bleibt 404", mit_kennung, "mit_kennung",
               "GET", "/v1/nirgendwo", b"scope=" + b"x" * 200, mit)
    await fall("Art mit Grossbuchstaben ist kein Weg", mit_kennung, "mit_kennung",
               "GET", "/v1/doc/Playback", b"", mit)

    # --- Bereich und Methode ---
    await fall("kaputter Bereich", mit_kennung, "mit_kennung", "GET", "/v1/doc/playback",
               b"scope=" + b"x" * 200, mit)
    await fall("Schriftstueck mit falscher Methode", mit_kennung, "mit_kennung", "DELETE",
               "/v1/doc/playback", b"scope=geraet-1", mit)
    await fall("Liste mit falscher Methode", mit_kennung, "mit_kennung", "POST",
               "/v1/docs/playback", b"", mit)

    # --- Lesen und Listen ---
    await fall("Schriftstueck lesen", mit_kennung, "mit_kennung", "GET", "/v1/doc/playback",
               b"scope=geraet-1", mit)
    await fall("Schriftstueck gibt es nicht", mit_kennung, "mit_kennung", "GET", "/v1/doc/playback",
               b"scope=geraet-9", mit)
    await fall("Liste", mit_kennung, "mit_kennung", "GET", "/v1/docs/playback", b"", mit)
    await fall("Liste einer leeren Art", mit_kennung, "mit_kennung", "GET", "/v1/docs/queue", b"", mit)

    # --- Schreiben ---
    await fall("Schrieb ohne Formatangabe", mit_kennung, "mit_kennung", "PUT", "/v1/doc/playback",
               b"scope=geraet-2", mit, body=b"{}")
    await fall("Schrieb mit unlesbarem Koerper", mit_kennung, "mit_kennung", "PUT", "/v1/doc/playback",
               b"scope=geraet-2", {**mit, "x-maer-format": "1"}, body=b"nicht json")
    await fall("Schrieb mit leerem Koerper geht an den Laden", mit_kennung, "mit_kennung", "PUT",
               "/v1/doc/playback", b"scope=geraet-2", {**mit, "x-maer-format": "1"}, body=b"")

    # --- Die geschlossenen Wege: das Tor steht VOR dem Zeichen ---
    await fall("geschlossen: Schriftstueck mit gueltigem Zeichen", geschlossen, "geschlossen",
               "GET", "/v1/doc/playback", b"scope=geraet-1", mit)
    await fall("geschlossen: Schriftstueck ohne Zeichen ist 426, nicht 401", geschlossen, "geschlossen",
               "GET", "/v1/doc/playback", b"scope=geraet-1")
    await fall("geschlossen: das Zeichen selbst ist hinter dem Tor", geschlossen, "geschlossen",
               "POST", "/v1/token", b"", {},
               body=json.dumps({"owner": BESITZER, "secret": GEHEIMNIS}).encode())
    await fall("geschlossen: unbekannter Weg ist 426, nicht 404", geschlossen, "geschlossen",
               "GET", "/v1/nirgendwo", b"", mit)
    await fall("geschlossen aber INNEN: Schriftstueck lesen", geschlossen, "geschlossen",
               "GET", "/v1/doc/playback", b"scope=geraet-1", mit, inside=True)
    await fall("geschlossen aber INNEN: Zeichen antwortet", geschlossen, "geschlossen",
               "POST", "/v1/token", b"", {},
               body=json.dumps({"owner": BESITZER, "secret": GEHEIMNIS}).encode(),
               inside=True, zufall=True)

    # --- Der Tunnel: eine echte Sitzung, von der Client-Seite gefahren ---
    tunnel = []

    async def sitzung_auf(app, privat):
        rufer = HandshakeState(initiator=True, remote_static=public_of(privat))
        erst = rufer.write_message(b"")
        s, kopf, antwort = await ruf(app, "POST", "/v1/session", b"", {}, erst)
        assert s == 200, s
        rufer.read_message(antwort)
        hin, her = rufer.split()
        return kopf["x-maer-session"], hin, her

    async def tunnelfall(name, app, name_der_sitzung, hin, her, kopf, koerper=b"",
                         roh=None, zufall=False, gestalt="rahmen"):
        gesendet = roh if roh is not None else hin.encrypt_with_ad(b"", pack(kopf, koerper))
        s, _, antwort = await ruf(app, "POST", "/v1/session", b"",
                                  {"x-maer-session": name_der_sitzung}, gesendet)
        eintrag = {"name": name, "gestalt": gestalt}
        if gestalt == "rahmen":
            gezeigt = dict(kopf)
            if "headers" in gezeigt:
                gezeigt["headers"] = {k: ("<zeichen>" if v == "Bearer " + zeichen else v)
                                      for k, v in gezeigt["headers"].items()}
            eintrag["rahmen"] = gezeigt
            if koerper:
                eintrag["rahmen_koerper_b64"] = base64.b64encode(koerper).decode()
        if name_der_sitzung != sname:
            eintrag["fremder_name"] = name_der_sitzung
        eintrag["aussen"] = {"status": s}
        if s == 200:
            innen_kopf, innen_koerper = unpack(her.decrypt_with_ad(b"", antwort))
            e = {"status": innen_kopf["status"]}
            try:
                gelesen = json.loads(innen_koerper)
                if zufall:
                    e["felder"] = sorted(gelesen)
                elif isinstance(gelesen, dict) and set(gelesen) == {"reason"}:
                    e["grund"] = gelesen["reason"]
                else:
                    e["koerper"] = gelesen
            except ValueError:
                e["koerper_b64"] = base64.b64encode(innen_koerper).decode()
            eintrag["innen"] = e
        else:
            eintrag["aussen"]["grund"] = json.loads(antwort)["reason"]
        if gehoert:
            eintrag["ansage"] = list(gehoert)
            gehoert.clear()
        tunnel.append(eintrag)

    privat, _ = ident
    sname, hin, her = await sitzung_auf(geschlossen, privat)
    def kopf_mit(pfad, methode="GET", q="", h=None):
        kopf = {"method": methode, "target": pfad + ("?" + q if q else "")}
        if h:
            kopf["headers"] = h
        return kopf

    await tunnelfall("innen: Gesundheit", geschlossen, sname, hin, her,
                     kopf_mit("/v1/health"))
    await tunnelfall("innen: Schriftstueck mit Zeichen", geschlossen, sname, hin, her,
                     kopf_mit("/v1/doc/playback", "GET", "scope=geraet-1",
                              {"authorization": "Bearer " + zeichen}))
    await tunnelfall("innen: Schriftstueck ohne Zeichen bleibt 401", geschlossen, sname, hin, her,
                     kopf_mit("/v1/doc/playback", "GET", "scope=geraet-1"))
    await tunnelfall("innen: die Sitzung schachtelt nicht", geschlossen, sname, hin, her,
                     kopf_mit("/v1/session", "POST"))
    await tunnelfall("innen: der Schluessel wird innen nicht beantwortet", geschlossen, sname, hin, her,
                     kopf_mit("/v1/key"))
    await tunnelfall("innen: Anmeldung", geschlossen, sname, hin, her,
                     kopf_mit("/v1/token", "POST"),
                     json.dumps({"owner": BESITZER, "secret": GEHEIMNIS}).encode(), zufall=True)
    await tunnelfall("innen: ein Schrieb sagt an", geschlossen, sname, hin, her,
                     kopf_mit("/v1/doc/playback", "PUT", "scope=geraet-1",
                              {"authorization": "Bearer " + zeichen,
                               "x-maer-format": "1", "if-match": "1"}),
                     json.dumps({"a": 2}).encode())
    await tunnelfall("innen: eine gelogene Laenge im Rahmenkopf zaehlt nicht",
                     geschlossen, sname, hin, her,
                     kopf_mit("/v1/doc/playback", "PUT", "scope=geraet-1",
                              {"authorization": "Bearer " + zeichen,
                               "x-maer-format": "1", "if-match": "2",
                               "content-length": "999"}),
                     json.dumps({"a": 3}).encode())
    await tunnelfall("innen: kein Rahmen", geschlossen, sname, hin, her, {},
                     roh=hin.encrypt_with_ad(b"", b"kein rahmen"),
                     gestalt="versiegelter_unsinn")
    await tunnelfall("kein versiegelter Brief", geschlossen, sname, hin, her, {},
                     roh=b"kein siegel", gestalt="kein_siegel")
    await tunnelfall("eine Sitzung, die es nicht gibt", geschlossen, "gibtsnicht", hin, her, {},
                     roh=b"egal", gestalt="fremde_sitzung")

    laden.close()
    print(json.dumps({
        "was": "Die Reihenfolge der Tore in service/app.py, gemessen an der Anwendung selbst.",
        "wie": "Jeder Fall wurde gegen die echte ASGI-Anwendung gerufen; nichts ist behauptet.",
        "anwendungen": {
            "ohne_kennung": "App(store) ohne statischen Schluessel",
            "mit_kennung": "App(store, identity=...) mit Schluessel, alte Wege offen",
            "geschlossen": "App(store, identity=..., plain_ways=False)"},
        "hinweis_zeichen": "<zeichen> steht fuer ein gueltiges Bearer-Zeichen; Werte stehen nie in dieser Datei.",
        "hinweis_tunnel": "Der Handschlag ist zufaellig; festgehalten sind Status und Satz, nicht die Bytes. Die Bytes stehen in noise-nk.json.",
        "faelle": faelle,
        "tunnel": tunnel}, indent=2, ensure_ascii=False))


asyncio.run(main())
