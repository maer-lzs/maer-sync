# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Erzeugt `vektoren/abweisung.json`: was ein abgewiesener Aufstieg auf der
LEITUNG wird.

    python3 vektoren/erzeuge_abweisung.py --python /pfad/zur/.venv/bin/python \\
        > vektoren/abweisung.json

**Warum das eine eigene Datei ist und nicht in `horcher.json` steht.** Die
Horcher-Tafel wird an der ASGI-Schnittstelle gemessen, in einem Erzeuger, der
nur die Standardbibliothek braucht und deshalb byte-gleich nachfahrbar ist.
Diese Messung hier braucht einen SERVER, also eine venv mit uvicorn. Stuende
sie in derselben Datei, koennte der Zeuge sie nicht mehr ohne venv nachfahren,
und die Byte-Gleichheit der ganzen Tafel haette eine Bedingung, die niemand
sieht. Zwei Dateien mit zwei Bedingungen sind ehrlicher als eine mit einer
versteckten.

**Was hier gemessen wird, und was ausdruecklich NICHT.** Ein `websocket.close`
vor dem `accept` traegt einen Schliesscode; auf der Leitung gibt es ihn nicht,
weil es noch keine Leitung gibt. Der Server macht daraus einen HTTP-Status.
Gemessen wird deshalb **Status und Koerperlaenge**, und sonst nichts: die
Kopfzeilen sind Zierrat dessen, der serviert, und die zwei uvicorn-Fassungen,
die hier lagen, schicken verschiedene (0.52.4 mit `Connection: close`,
`Content-Type` und Grundphrase, 0.53.0 nur `content-length`). Wer sie
nachbaute, band sich an eine Fassung eines fremden Servers.

Fail-closed: ohne `--python` bricht es ab. Ein Erzeuger, der ohne Server
einfach nichts misst, schriebe eine Datei, die zu sich selbst passt und nichts
sagt.
"""

import argparse, base64, json, os, socket, subprocess, sys, tempfile, time

WURZEL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def freier_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def aufstieg(port, pfad, kopfzeilen=None, frist=6.0):
    """Nur den Aufstieg versuchen und lesen, was zurueckkommt."""
    s = socket.create_connection(("127.0.0.1", port), timeout=frist)
    k = base64.b64encode(os.urandom(16)).decode()
    zeilen = [f"GET {pfad} HTTP/1.1", f"Host: 127.0.0.1:{port}",
              "Upgrade: websocket", "Connection: Upgrade",
              f"Sec-WebSocket-Key: {k}", "Sec-WebSocket-Version: 13"]
    zeilen += [f"{a}: {b}" for a, b in (kopfzeilen or {}).items()]
    s.sendall(("\r\n".join(zeilen) + "\r\n\r\n").encode())
    daten = b""
    try:
        while True:
            teil = s.recv(4096)
            if not teil:
                break
            daten += teil
    except socket.timeout:
        pass
    s.close()
    kopf, _, koerper = daten.partition(b"\r\n\r\n")
    erste = kopf.split(b"\r\n")[0].decode("utf-8", "replace")
    return int(erste.split(" ")[1]), len(koerper)


FAELLE = [
    ("ein anderer Weg", "/v1/nirgendwo", None),
    ("der Horchweg ohne Zeichen", "/v1/changes", None),
    ("der Horchweg mit unbekanntem Zeichen", "/v1/changes",
     {"Authorization": "Bearer nichtvergeben"}),
    ("der Horchweg mit erfundener Sitzung", "/v1/changes",
     {"X-Maer-Session": "gibtsnicht"}),
]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--python", required=True,
                   help="das Python einer venv mit uvicorn (siehe README)")
    p.add_argument("--quelle", default=WURZEL,
                   help="das Repo, aus dem `service` gefahren wird")
    a = p.parse_args()

    if not os.access(a.python, os.X_OK):
        print(f"{a.python} ist nicht ausfuehrbar", file=sys.stderr)
        return 2
    fassung = subprocess.run(
        [a.python, "-c", "import uvicorn;print(uvicorn.__version__)"],
        capture_output=True, text=True)
    if fassung.returncode != 0:
        print(f"in {a.python} liegt kein uvicorn: {fassung.stderr.strip()}", file=sys.stderr)
        return 2
    uv = fassung.stdout.strip()

    tmp = tempfile.mkdtemp()
    port = freier_port()
    umgebung = dict(os.environ,
                    MAER_SYNC_DB=os.path.join(tmp, "a.sqlite"),
                    MAER_SYNC_PORT=str(port))
    dienst = subprocess.Popen([a.python, "-m", "service"], cwd=a.quelle, env=umgebung,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        # Gutfall zuerst, und er IDENTIFIZIERT den Dienst: sonst misst man
        # womoeglich etwas anderes, das auf diesem Port antwortet.
        import urllib.request
        abdruck = None
        for _ in range(200):
            try:
                with urllib.request.urlopen(
                        f"http://127.0.0.1:{port}/v1/key", timeout=1) as r:
                    abdruck = json.load(r)["fingerprint"]
                    break
            except Exception:
                time.sleep(0.1)
        if abdruck is None:
            print("der Dienst kam nicht hoch, es wurde nichts gemessen", file=sys.stderr)
            return 3

        gemessen = []
        for name, pfad, kopfzeilen in FAELLE:
            status, laenge = aufstieg(port, pfad, kopfzeilen)
            eintrag = {"name": name, "anfrage": {"path": pfad}, "erwartet":
                       {"status": status, "koerper_bytes": laenge}}
            if kopfzeilen:
                eintrag["anfrage"]["headers"] = kopfzeilen
            gemessen.append(eintrag)
    finally:
        dienst.terminate()
        dienst.wait(timeout=10)

    print(json.dumps({
        "was": "Was aus einem Schliesscode wird, wenn er VOR dem Annehmen faellt.",
        "wie": f"Die Python-Fassung unter uvicorn {uv} gefahren und der Aufstieg "
               f"versucht; gemessen, nicht abgeschrieben.",
        "hinweis_kopfzeilen": "Nur Status und Koerperlaenge stehen hier. Die "
                              "Kopfzeilen sind Zierrat dessen, der serviert, und "
                              "wechseln zwischen zwei Fassungen desselben Servers.",
        "faelle": gemessen}, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
