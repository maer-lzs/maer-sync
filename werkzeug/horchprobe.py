# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Faehrt die Horcher-Tafel gegen einen LAUFENDEN Dienst.

    python3 werkzeug/horchprobe.py --gegen http://127.0.0.1:8081 --zeichen <wert>

Die Standardbibliothek hat keinen WebSocket-Klienten, also steht hier einer:
der Aufstieg nach RFC 6455 und so viel Rahmenwerk, wie diese Probe braucht.
Das ist Absicht und kein Notbehelf. Ein fremder Klient wuerde die Bequemlichkeit
mitbringen, Fehler zu verzeihen, und genau die will diese Probe sehen.

**Was hier NICHT geprueft werden kann, und warum.** Die Tafel haelt
Schliesscodes fest (4004, 4401, 4426, 4410, 4400). Ein Schliesscode ist erst
nach dem ANNEHMEN sichtbar: davor gibt es keine Leitung, auf der er reisen
koennte, und die Abweisung faellt als HTTP-Status. Welcher Status das ist, sagt
ASGI nicht; es haengt am Server, der die Python-Fassung traegt. Die Faelle vor
dem Annehmen werden deshalb hier gegen die UEBERSETZUNG geprueft, die der
Rust-Dienst ausdruecklich vornimmt, und der Unterschied ist im Bericht benannt,
nicht weggeraeumt.
"""

import argparse, base64, hashlib, json, os, socket, struct, sys, urllib.request

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from core.frames import pack, unpack
from core.handshake import HandshakeState

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


class Leitung:
    """Ein WebSocket, so weit diese Probe ihn braucht."""

    def __init__(self, wirt, port, pfad, kopfzeilen):
        self.s = socket.create_connection((wirt, port), timeout=10)
        schluessel = base64.b64encode(os.urandom(16)).decode()
        self.erwartet = base64.b64encode(
            hashlib.sha1((schluessel + GUID).encode()).digest()).decode()
        zeilen = [f"GET {pfad} HTTP/1.1", f"Host: {wirt}:{port}",
                  "Upgrade: websocket", "Connection: Upgrade",
                  f"Sec-WebSocket-Key: {schluessel}", "Sec-WebSocket-Version: 13"]
        zeilen += [f"{k}: {v}" for k, v in kopfzeilen.items()]
        self.s.sendall(("\r\n".join(zeilen) + "\r\n\r\n").encode())
        self.puffer = b""
        kopf = self._bis(b"\r\n\r\n")
        zeilen = kopf.decode("utf-8", "replace").split("\r\n")
        self.status = int(zeilen[0].split(" ")[1])
        self.kopf = {}
        for z in zeilen[1:]:
            if ": " in z:
                k, v = z.split(": ", 1)
                self.kopf[k.lower()] = v

    def _bis(self, marke):
        while marke not in self.puffer:
            teil = self.s.recv(4096)
            if not teil:
                raise EOFError("die Leitung ging zu, bevor der Kopf ganz war")
            self.puffer += teil
        i = self.puffer.index(marke) + len(marke)
        kopf, self.puffer = self.puffer[:i], self.puffer[i:]
        return kopf

    def _lies(self, n):
        while len(self.puffer) < n:
            teil = self.s.recv(65536)
            if not teil:
                raise EOFError("die Leitung ging zu")
            self.puffer += teil
        aus, self.puffer = self.puffer[:n], self.puffer[n:]
        return aus

    def sende(self, daten, binaer=True):
        """Ein Rahmen vom Klienten ist IMMER maskiert, so will es RFC 6455."""
        kopf = bytes([0x80 | (0x02 if binaer else 0x01)])
        maske = os.urandom(4)
        n = len(daten)
        if n < 126:
            kopf += bytes([0x80 | n])
        elif n < 65536:
            kopf += bytes([0x80 | 126]) + struct.pack("!H", n)
        else:
            kopf += bytes([0x80 | 127]) + struct.pack("!Q", n)
        verwuerfelt = bytes(b ^ maske[i % 4] for i, b in enumerate(daten))
        self.s.sendall(kopf + maske + verwuerfelt)

    def empfange(self, frist=5.0):
        """Ein Rahmen, oder nichts. Gibt (art, inhalt) zurueck."""
        self.s.settimeout(frist)
        try:
            erst = self._lies(2)
        except (socket.timeout, EOFError):
            return (None, None)
        opcode = erst[0] & 0x0F
        laenge = erst[1] & 0x7F
        if laenge == 126:
            laenge = struct.unpack("!H", self._lies(2))[0]
        elif laenge == 127:
            laenge = struct.unpack("!Q", self._lies(8))[0]
        inhalt = self._lies(laenge) if laenge else b""
        if opcode == 0x8:
            code = struct.unpack("!H", inhalt[:2])[0] if len(inhalt) >= 2 else None
            return ("zu", code)
        if opcode == 0x1:
            return ("text", inhalt.decode("utf-8", "replace"))
        if opcode == 0x2:
            return ("bytes", inhalt)
        return ("sonst", inhalt)

    def zu(self):
        try:
            self.s.close()
        except OSError:
            pass


def hole(url, method="GET", kopfzeilen=None, koerper=None):
    import urllib.error
    a = urllib.request.Request(url, method=method, data=koerper)
    for k, v in (kopfzeilen or {}).items():
        a.add_header(k, v)
    try:
        with urllib.request.urlopen(a, timeout=15) as r:
            return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as f:
        return f.code, dict(f.headers), f.read()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--gegen", required=True)
    p.add_argument("--zeichen", required=True)
    p.add_argument("--geschlossen", action="store_true")
    a = p.parse_args()
    basis = a.gegen.rstrip("/")
    wirt, port = basis.split("//")[1].split(":")
    port = int(port)

    status, _, gab = hole(basis + "/v1/key")
    if status != 200:
        print(f"  ABBRUCH: /v1/key gab {status}")
        return 2
    print(f"  Gutfall: der Dienst nennt sich {json.loads(gab)['fingerprint']}")
    statisch = base64.b64decode(json.loads(gab)["key"])

    gut, schlecht = 0, []

    def pruefe(name, bedingung, was):
        nonlocal gut
        if bedingung:
            gut += 1
            print(f"    ok            {name}")
        else:
            schlecht.append((name, was))
            print(f"    ABWEICHUNG    {name}: {was}")

    # Die Abweisungen vor dem Annehmen kommen aus `vektoren/abweisung.json`,
    # und die Datei ist an der Python-Fassung unter uvicorn GEMESSEN. Der
    # Schliesscode existiert hier nicht: es gibt noch keine Leitung, auf der
    # er reisen koennte.
    with open(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                           "vektoren", "abweisung.json"), encoding="utf-8") as f:
        tafel = json.load(f)
    print(f"\n  -- die Abweisungen vor dem Annehmen ({len(tafel['faelle'])} gemessene Faelle) --")
    for fall in tafel["faelle"]:
        an = fall["anfrage"]
        l = Leitung(wirt, port, an["path"], an.get("headers") or {})
        will = fall["erwartet"]
        pruefe(f"{fall['name']}: Status", l.status == will["status"],
               f"{l.status} statt {will['status']}")
        # Der Koerper zaehlt mit: ohne content-length schickt hyper `chunked`
        # und damit fuenf Byte, wo uvicorn null schickt. Das ist genau die
        # Sorte Unterschied, die an der Schnittstelle unsichtbar ist.
        rest = l.puffer
        pruefe(f"{fall['name']}: Koerper", len(rest) == will["koerper_bytes"],
               f"{len(rest)} Byte statt {will['koerper_bytes']}")
        l.zu()

    if not a.geschlossen:
        print("\n  -- der offene Horcher hoert seine Ansage --")
        l = Leitung(wirt, port, "/v1/changes", {"Authorization": f"Bearer {a.zeichen}"})
        pruefe("Aufstieg angenommen", l.status == 101, f"Status {l.status} statt 101")
        pruefe("der Aufstiegs-Schluessel stimmt",
               l.kopf.get("sec-websocket-accept") == l.erwartet,
               f"{l.kopf.get('sec-websocket-accept')!r} statt {l.erwartet!r}")
        status, _, _ = hole(basis + "/v1/doc/playback?scope=horchprobe", "PUT",
                            {"Authorization": f"Bearer {a.zeichen}", "X-Maer-Format": "1"},
                            json.dumps({"a": 1}).encode())
        pruefe("der Schrieb wurde angenommen", status == 200, f"HTTP {status}")
        art, inhalt = l.empfange()
        pruefe("die Ansage kam an, als TEXT", art == "text", f"{art!r}")
        if art == "text":
            gelesen = json.loads(inhalt)
            pruefe("die Ansage nennt Art, Bereich und Fassung",
                   gelesen == {"kind": "playback", "scope": "horchprobe", "version": "1"},
                   f"{gelesen}")
        l.zu()

        print("\n  -- ein getrennter Horcher hoert nichts mehr --")
        # Die Leitung von eben ist zu. Ein Schrieb jetzt darf NIRGENDS mehr
        # ankommen, und das ist an einer FRISCHEN Leitung zu messen: sie darf
        # die Ansage des Schriebs, der vor ihr lag, nicht bekommen.
        status, _, _ = hole(basis + "/v1/doc/playback?scope=horchprobe2", "PUT",
                            {"Authorization": f"Bearer {a.zeichen}", "X-Maer-Format": "1"},
                            json.dumps({"a": 1}).encode())
        pruefe("der Schrieb an den getrennten Horcher ging durch",
               status == 200, f"HTTP {status}")
        spaet = Leitung(wirt, port, "/v1/changes", {"Authorization": f"Bearer {a.zeichen}"})
        art, inhalt = spaet.empfange(frist=1.0)
        pruefe("die frische Leitung bekommt NICHTS nachgereicht",
               art is None, f"bekam {art!r}: {inhalt!r}")
        spaet.zu()

    else:
        print("\n  -- der versiegelte Horcher --")
        rufer = HandshakeState(initiator=True, remote_static=statisch)
        st, kopf, antwort = hole(basis + "/v1/session", "POST", {}, rufer.write_message(b""))
        rufer.read_message(antwort)
        hin, her = rufer.split()
        sname = {k.lower(): v for k, v in kopf.items()}["x-maer-session"]

        l = Leitung(wirt, port, "/v1/changes", {"X-Maer-Session": sname})
        pruefe("Aufstieg angenommen", l.status == 101, f"Status {l.status} statt 101")
        # Erst der versiegelte Rahmen mit dem Zeichen, dann steht er auf der Liste.
        l.sende(hin.encrypt_with_ad(b"", pack(
            {"method": "GET", "target": "/v1/changes",
             "headers": {"authorization": f"Bearer {a.zeichen}"}}, b"")))

        # Der Schrieb braucht eine EIGENE Sitzung: bei geschlossenen Wegen ist
        # ein gewoehnlicher Schrieb 426, und dann entstuende gar keine Ansage.
        r2 = HandshakeState(initiator=True, remote_static=statisch)
        st, kopf2, antwort2 = hole(basis + "/v1/session", "POST", {}, r2.write_message(b""))
        r2.read_message(antwort2)
        h2, e2 = r2.split()
        s2 = {k.lower(): v for k, v in kopf2.items()}["x-maer-session"]
        st, _, ant = hole(basis + "/v1/session", "POST", {"X-Maer-Session": s2},
                          h2.encrypt_with_ad(b"", pack(
                              {"method": "PUT", "target": "/v1/doc/playback?scope=versiegelt",
                               "headers": {"authorization": f"Bearer {a.zeichen}",
                                           "x-maer-format": "1"}},
                              json.dumps({"a": 1}).encode())))
        innen_kopf, _ = unpack(e2.decrypt_with_ad(b"", ant))
        pruefe("der Schrieb durch die zweite Sitzung ging durch",
               innen_kopf["status"] == 200, f"innen {innen_kopf['status']}")

        art, inhalt = l.empfange()
        pruefe("die Ansage kam an, als BYTES", art == "bytes", f"{art!r}")
        if art == "bytes":
            gelesen = json.loads(her.decrypt_with_ad(b"", inhalt))
            pruefe("sie geht in DIESER Sitzung auf und nennt den Bereich",
                   gelesen == {"kind": "playback", "scope": "versiegelt", "version": "1"},
                   f"{gelesen}")
        l.zu()

        print("\n  -- ein erstes Wort, das kein versiegelter Rahmen ist --")
        r3 = HandshakeState(initiator=True, remote_static=statisch)
        st, kopf3, antwort3 = hole(basis + "/v1/session", "POST", {}, r3.write_message(b""))
        r3.read_message(antwort3)
        h3, _ = r3.split()
        s3 = {k.lower(): v for k, v in kopf3.items()}["x-maer-session"]
        l = Leitung(wirt, port, "/v1/changes", {"X-Maer-Session": s3})
        pruefe("Aufstieg angenommen (erst annehmen, dann pruefen)",
               l.status == 101, f"Status {l.status}")
        l.sende(b"kein siegel")
        art, code = l.empfange()
        pruefe("geschlossen mit 4400", art == "zu" and code == 4400, f"{art!r} {code}")
        l.zu()

        print("\n  -- ein erster Rahmen ohne Zeichen --")
        r4 = HandshakeState(initiator=True, remote_static=statisch)
        st, kopf4, antwort4 = hole(basis + "/v1/session", "POST", {}, r4.write_message(b""))
        r4.read_message(antwort4)
        h4, _ = r4.split()
        s4 = {k.lower(): v for k, v in kopf4.items()}["x-maer-session"]
        l = Leitung(wirt, port, "/v1/changes", {"X-Maer-Session": s4})
        l.sende(h4.encrypt_with_ad(b"", pack({"method": "GET", "target": "/v1/changes"}, b"")))
        art, code = l.empfange()
        pruefe("geschlossen mit 4401", art == "zu" and code == 4401, f"{art!r} {code}")
        l.zu()

    print(f"\n  gleich: {gut}   abweichend: {len(schlecht)}")
    return 1 if schlecht else 0


if __name__ == "__main__":
    sys.exit(main())
