# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Fassung 6 gegen ein LAUFENDES Erzeugnis: jeder Weg eines Geheimnisses,
von der Klient-Seite gefahren, wie die App ihn faehrt.

    python3 werkzeug/geheimnisprobe.py <erzeugnis>

Das Erzeugnis laeuft dabei auf einer WEGWERF-Aufstellung: ein freier Port auf
127.0.0.1, ein eigener Laden und ein eigener Schluessel in einem Verzeichnis,
das nach dem Lauf geloescht wird. Nie gegen einen Dienst, den jemand nutzt.

Die Zeugen in `tests/ein_geheimnis.rs` pruefen die Entscheidungen ohne Netz.
Diese Probe prueft, was nur ein echter Lauf zeigt: dass das Erzeugnis die Wege
tatsaechlich auf der Leitung beantwortet, im Tunnel, mit den Befehlen des
Betreibers daneben. Ein Nachbau beweist die Logik, erst ein Lauf die
Verdrahtung.

Die Ableitung macht die Probe selbst, wie das Geraet: PBKDF2 aus `hashlib`,
HKDF von Hand nach RFC 5869. Bevor sie irgendetwas misst, eicht sie beides an
`vektoren/sign-in-v6.json`; stimmt das nicht, haelt sie an, denn dann misst sie
ihren eigenen Fehler.

Ausgang 0: jede Zeile wie erwartet. 1: mindestens eine Abweichung. 2: nicht
gemessen (Eichung, Start, Handschlag).
"""

import base64, hashlib, hmac, json, os, shutil, socket, subprocess, sys, tempfile, time
import unicodedata, urllib.error, urllib.request

WURZEL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from klient.frames import pack, unpack
from klient.handshake import HandshakeState

KONTO, ALT, RUNDEN = "probe", "ein altes passwort der fassung fuenf", 600000


def hkdf(ikm, info, laenge=32):
    prk = hmac.new(b"\x00" * 32, ikm, hashlib.sha256).digest()
    aus, t, i = b"", b"", 1
    while len(aus) < laenge:
        t = hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        aus += t
        i += 1
    return aus[:laenge]


def ableiten(passwort, salz, runden=RUNDEN):
    p = unicodedata.normalize("NFC", passwort).encode("utf-8")
    m = hashlib.pbkdf2_hmac("sha256", p, salz, runden, 32)
    return hkdf(m, b"maer sign in v1"), hkdf(m, b"maer feed auth v2")


def eichen():
    v = json.load(open(os.path.join(WURZEL, "vektoren", "sign-in-v6.json")))
    for f in v["vectors"]:
        pw = bytes.fromhex(f["password_utf8_hex"]).decode("utf-8")
        anmelden, feed = ableiten(pw, bytes.fromhex(f["salt_hex"]), f["rounds"])
        if anmelden.hex() != f["sign_in_hex"] or feed.hex() != f["feed_hex"]:
            return f["name"]
    return None


def hole(url, method="GET", kopf=None, koerper=None):
    a = urllib.request.Request(url, method=method, data=koerper)
    for k, w in (kopf or {}).items():
        a.add_header(k, w)
    try:
        with urllib.request.urlopen(a, timeout=30) as r:
            return r.status, {k.lower(): w for k, w in r.headers.items()}, r.read()
    except urllib.error.HTTPError as e:
        return e.code, {k.lower(): w for k, w in e.headers.items()}, e.read()


class Tunnel:
    def __init__(self, basis):
        self.basis = basis
        st, _, roh = hole(basis + "/v2/key")
        if st != 200:
            raise RuntimeError(f"/v2/key gab {st}")
        fremd = base64.b64decode(json.loads(roh)["key"])
        rufer = HandshakeState(initiator=True, remote_static=fremd)
        st, kopf, antwort = hole(basis + "/v1/session", "POST",
                                 {"Content-Type": "application/octet-stream"}, rufer.write_message(b""))
        if st != 200:
            raise RuntimeError(f"Handschlag gab {st}")
        self.name = kopf["x-maer-session"]
        rufer.read_message(antwort)
        self.hin, self.her = rufer.split()

    def frage(self, method, ziel, koerper=None, kopf=None):
        roh = json.dumps(koerper).encode() if koerper is not None else b""
        k = {"method": method, "target": ziel, "headers": dict(kopf or {})}
        st, _, antwort = hole(self.basis + "/v1/session", "POST",
                              {"X-Maer-Session": self.name, "Content-Type": "application/octet-stream"},
                              self.hin.encrypt_with_ad(b"", pack(k, roh)))
        if st != 200:
            return st, {}, b""
        kopfrahmen, inhalt = unpack(self.her.decrypt_with_ad(b"", antwort))
        return kopfrahmen["status"], {x.lower(): y for x, y in kopfrahmen["headers"].items()}, inhalt


def main():
    if len(sys.argv) != 2:
        print("usage: geheimnisprobe.py <erzeugnis>", file=sys.stderr)
        return 2
    erzeugnis = os.path.abspath(sys.argv[1])
    falsch = eichen()
    if falsch:
        print(f"NICHT GEMESSEN: die Ableitung dieser Probe trifft den Vektor '{falsch}' nicht")
        return 2

    ort = tempfile.mkdtemp(prefix="geheimnisprobe-")
    s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
    umgebung = dict(os.environ, MAER_SYNC_DB=os.path.join(ort, "sync.sqlite"),
                    MAER_SYNC_KEY=os.path.join(ort, "identity.key"),
                    MAER_SYNC_BIND=f"127.0.0.1:{port}", MAER_SYNC_PLAIN_WAYS="on")
    befehl = lambda *a, eingabe=None: subprocess.run([erzeugnis, *a], env=umgebung, input=eingabe,
                                                     capture_output=True, text=True, timeout=60)
    dienst = None
    zeilen, abweichungen = [], 0

    def pruefe(was, hat, will):
        nonlocal abweichungen
        ok = hat == will
        abweichungen += 0 if ok else 1
        zeilen.append(f"{'gleich ' if ok else 'ANDERS '}| {was} | {hat!r}" + ("" if ok else f" statt {will!r}"))

    try:
        r = befehl("key", "--new")
        if r.returncode != 0:
            print(f"NICHT GEMESSEN: key --new gab {r.returncode}: {r.stderr.strip()}")
            return 2
        r = befehl("account", KONTO, eingabe=ALT + "\n")
        if r.returncode != 0:
            print(f"NICHT GEMESSEN: account gab {r.returncode}: {r.stderr.strip()}")
            return 2
        dienst = subprocess.Popen([erzeugnis, "serve"], env=umgebung,
                                  stdout=open(os.path.join(ort, "protokoll.txt"), "w"), stderr=subprocess.STDOUT)
        basis = f"http://127.0.0.1:{port}"
        for _ in range(100):
            try:
                if hole(basis + "/v1/health")[0] == 200:
                    break
            except OSError:
                pass
            time.sleep(0.1)
        else:
            print("NICHT GEMESSEN: der Dienst kam nicht hoch")
            return 2

        # Die Wege vor jeder Anmeldung.
        st, kopf, _ = hole(basis + "/v1/key")
        pruefe("GET /v1/key (Riegel c)", st, 404)
        st, kopf, _ = hole(basis + "/v2/key")
        pruefe("GET /v2/key", (st, kopf.get("x-maer-contract")), (200, "6"))
        for m, p in [("POST", "/v1/token"), ("GET", "/v1/salt?owner=" + KONTO), ("POST", "/v1/password")]:
            st, _, _ = hole(basis + p, m, {"Content-Type": "application/json"},
                            json.dumps({"owner": KONTO, "secret": ALT}).encode() if m == "POST" else None)
            pruefe(f"{m} {p.split('?')[0]} von aussen", st, 426)

        t = Tunnel(basis)
        st, _, roh = t.frage("GET", "/v1/salt?owner=" + KONTO)
        pruefe("Salz des Kontos der Fassung 5", (st, json.loads(roh).get("form")), (200, "secret"))
        st, _, roh1 = t.frage("GET", "/v1/salt?owner=niemand")
        st2, _, roh2 = t.frage("GET", "/v1/salt?owner=niemand")
        pruefe("Schein-Salz stabil und in Form proof",
               (st, roh1 == roh2, json.loads(roh1).get("form"), json.loads(roh1).get("rounds")),
               (200, True, "proof", RUNDEN))

        # Ein Zeichen der alten Form, das nach dem Umzug zurueckgezogen sein muss.
        st, _, roh = t.frage("POST", "/v1/token", {"owner": KONTO, "secret": ALT, "device": "alt"})
        pruefe("Anmeldung der Fassung 5 vor dem Umzug", st, 200)
        alt_zeichen = json.loads(roh)["token"]

        # Der Umzug.
        neu_pw, salz = "korrekt pferd batterie heftklammer", os.urandom(16)
        anmelden, _ = ableiten(neu_pw, salz)
        st, _, roh = t.frage("POST", "/v1/password", {
            "owner": KONTO, "device": "probe", "current": {"secret": ALT},
            "new": {"proof": base64.b64encode(anmelden).decode(), "salt": base64.b64encode(salz).decode(),
                    "kdf": "pbkdf2-hmac-sha256", "rounds": RUNDEN}})
        pruefe("Umzug per /v1/password", st, 200)

        st, kopf, _ = t.frage("GET", "/v1/doc/playback?scope=x", kopf={"Authorization": f"Bearer {alt_zeichen}"})
        pruefe("altes Zeichen nach dem Umzug", (st, kopf.get("x-maer-refusal")), (401, "password-changed"))

        # Anmelden wie ein zweites Geraet: Salz holen, ableiten, Beweis schicken.
        st, _, roh = t.frage("GET", "/v1/salt?owner=" + KONTO)
        s2 = json.loads(roh)
        pruefe("Salz nach dem Umzug ist das gewaehlte", (s2.get("form"), s2.get("salt")),
               ("proof", base64.b64encode(salz).decode()))
        b2, _ = ableiten(neu_pw, base64.b64decode(s2["salt"]), s2["rounds"])
        st, _, roh = t.frage("POST", "/v1/token", {"owner": KONTO, "proof": base64.b64encode(b2).decode(),
                                                   "device": "zweites"})
        pruefe("Anmeldung mit Beweis", st, 200)
        st, _, _ = t.frage("POST", "/v1/token", {"owner": KONTO, "secret": ALT, "device": "x"})
        pruefe("altes Passwort nach dem Umzug", st, 401)

        # Das Einmal-Passwort des Betreibers.
        r = befehl("reset", KONTO)
        einmal = r.stdout.split(": ", 1)[1].split()[0] if r.returncode == 0 and ": " in r.stdout else ""
        pruefe("reset gibt ein Einmal-Passwort", (r.returncode, len(einmal)), (0, 24))
        st, _, _ = t.frage("POST", "/v1/token", {"owner": KONTO, "secret": einmal, "device": "x"})
        pruefe("Anmeldung mit dem Einmal-Passwort", st, 428)
        st, _, roh = t.frage("GET", "/v1/salt?owner=" + KONTO)
        pruefe("Salz nach reset", json.loads(roh).get("form"), "secret")
        pw3, salz3 = "ein drittes passwort mit laenge", os.urandom(16)
        b3, _ = ableiten(pw3, salz3)
        st, _, _ = t.frage("POST", "/v1/password", {
            "owner": KONTO, "device": "nach-reset", "current": {"secret": einmal},
            "new": {"proof": base64.b64encode(b3).decode(), "salt": base64.b64encode(salz3).decode(),
                    "kdf": "pbkdf2-hmac-sha256", "rounds": RUNDEN}})
        pruefe("Wechsel mit dem Einmal-Passwort", st, 200)

        r = befehl("devices", KONTO)
        formen = sorted(z.split()[1] for z in r.stdout.splitlines() if z.startswith("device="))
        pruefe("devices nennt jede Form", (r.returncode, formen),
               (0, ["form=move", "form=proof", "form=reset", "form=secret"]))

        # Die Versuchsgrenze, direkt von der Schleife (keine X-Real-IP).
        stati = [t.frage("POST", "/v1/token", {"owner": "raten", "secret": f"falsch{i}"})[0] for i in range(11)]
        st, kopf, _ = t.frage("POST", "/v1/token", {"owner": "raten", "secret": "falsch"})
        pruefe("zehn Fehlversuche, dann 429", (stati[:10].count(401), stati[10], st, "retry-after" in kopf),
               (10, 429, 429, True))

        # Was im Protokoll des Dienstes steht: nie ein Passwort, nie ein Beweis.
        dienst.terminate(); dienst.wait(timeout=10); dienst = None
        protokoll = open(os.path.join(ort, "protokoll.txt")).read()
        geheim = [ALT, neu_pw, einmal, pw3, base64.b64encode(anmelden).decode(), base64.b64encode(b2).decode()]
        pruefe("Protokoll ohne Passwort oder Beweis", sum(protokoll.count(g) for g in geheim if g), 0)
        pruefe("Protokoll hat Zeilen", protokoll.count("maer.sync") > 10, True)
    finally:
        if dienst:
            dienst.terminate()
            try: dienst.wait(timeout=10)
            except subprocess.TimeoutExpired: dienst.kill()
        shutil.rmtree(ort, ignore_errors=True)

    for z in zeilen:
        print(z)
    print(f"{len(zeilen)} Zeilen, {abweichungen} anders")
    return 1 if abweichungen else 0


if __name__ == "__main__":
    sys.exit(main())
