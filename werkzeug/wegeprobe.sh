#!/bin/sh
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# Die VOLLE Wege-Tafel gegen ein laufendes Erzeugnis, mit dem Bestand, den
# ihr Erzeuger angelegt hat.
#
#   werkzeug/wegeprobe.sh rust
#   werkzeug/wegeprobe.sh python
#
# WARUM ES DAS GIBT. `gegenprobe.py` gegen einen nackten Dienst gespielt
# meldet vier Abweichungen, und drei davon sind kein Befund: die Tafel traegt
# den Zustand, den ihr Erzeuger hatte. Die vierte ist es auch nicht, aber sie
# liegt woanders, und das hat mich einen berichtigten Bericht gekostet: die
# Tafel meldet sich als `anna` mit dem Geheimnis an, das im Erzeuger steht.
# Ein Pruefstand mit einem eigenen Konto kann diesen Fall nie bestehen.
#
# DAS FIXTUR-GEHEIMNIS STEHT IM REPO, und deshalb gehoert dieses Konto auf
# KEINE Aufstellung, die ins Netz soll (ENTSCHEID des Gegenlesers,
# 19.09.2026). Diese Probe legt es auf einer WEGWERF-Aufstellung an, die nur
# auf 127.0.0.1 horcht und nach dem Lauf geloescht wird. Der Pruefstand fuer
# die App-Abnahme behaelt sein eigenes Konto und sein eigenes Geheimnis.
set -eu

WURZEL=$(cd "$(dirname "$0")/.." && pwd)
WELCHE=${1:-}
PORT=${PORT:-8873}
VENV=${VENV:-}
ERZEUGNIS=${ERZEUGNIS:-$WURZEL/ziel/abbau/x86_64-unknown-linux-musl/release/maer-sync}

# Genau die Werte aus `vektoren/erzeuge_wege.py`. Sie hier zu wiederholen ist
# eine Handliste, und eine Handliste vergisst; deshalb werden sie unten
# GEGEN den Erzeuger geprueft, statt ihm geglaubt zu werden.
KONTO=anna
GEHEIM="ein hinreichend langes geheimnis"
GERAET=geraet-1

case "$WELCHE" in
  rust|python) ;;
  *) echo "usage: wegeprobe.sh [rust|python]" >&2; exit 2 ;;
esac

# --- Die Fixtur gegen ihre Quelle halten -----------------------------------
# Ein Erzeuger, der sein Konto umbenennt, macht diese Probe sonst still
# sinnlos: sie liefe weiter, nur gegen einen anderen Bestand.
pruefe_fixtur() {
  quelle=$WURZEL/vektoren/erzeuge_wege.py
  zeile=$(grep -E '^BESITZER, GEHEIMNIS = ' "$quelle" || true)
  soll="BESITZER, GEHEIMNIS = \"$KONTO\", \"$GEHEIM\""
  if [ "$zeile" != "$soll" ]; then
    echo "HALT: die Fixtur stimmt nicht mehr mit dem Erzeuger ueberein." >&2
    echo "  hier:  $soll" >&2
    echo "  dort:  $zeile" >&2
    exit 1
  fi
  if ! grep -q "scope=$GERAET" "$quelle"; then
    echo "HALT: der Erzeuger legt keinen Bereich $GERAET mehr an." >&2
    exit 1
  fi
}

frei_oder_halt() {
  if ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]$1\$"; then
    echo "HALT: Port $1 ist belegt. Ein Lauf dagegen misst einen fremden Dienst." >&2
    exit 1
  fi
}

aufraeumen() {
  if [ -n "${PID:-}" ]; then
    kill "$PID" 2>/dev/null || true
    n=0
    while ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]$PORT\$"; do
      n=$((n + 1)); [ "$n" -gt 20 ] && break
      [ "$n" -eq 10 ] && kill -9 "$PID" 2>/dev/null
      sleep 0.5
    done
    if ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]$PORT\$"; then
      echo "HALT: auf Port $PORT horcht nach dem Beenden immer noch etwas." >&2
    else
      echo "beendet, Port $PORT ist frei (gemessen)"
    fi
  fi
  [ -n "${TMP:-}" ] && rm -rf "$TMP" && echo "Wegwerf-Aufstellung geloescht"
  return 0
}
trap aufraeumen EXIT INT TERM

pruefe_fixtur
frei_oder_halt "$PORT"
TMP=$(mktemp -d)
DB="$TMP/a.sqlite"

MAER_SYNC_DB="$DB"
MAER_SYNC_KEY="$TMP/identity.key"
export MAER_SYNC_DB MAER_SYNC_KEY

if [ "$WELCHE" = rust ]; then
  [ -x "$ERZEUGNIS" ] || { echo "HALT: $ERZEUGNIS gibt es nicht. Erst bauen:" >&2
    echo "  werkzeug/bau.sh build --release --offline" >&2; exit 1; }
  RUF="$ERZEUGNIS"
  MAER_SYNC_BIND="127.0.0.1:$PORT"; export MAER_SYNC_BIND
  starte() { "$RUF" serve > "$TMP/protokoll.txt" 2>&1 & PID=$!; }
else
  if [ ! -x "$VENV" ]; then
    echo "UEBERSPRUNGEN MIT GRUND: kein Python mit uvicorn unter $VENV." >&2
    echo "Ohne Server kann diese Kopie die Frage nicht stellen (Regel 7)." >&2
    exit 0
  fi
  RUF="$VENV"
  MAER_SYNC_HOST=127.0.0.1; MAER_SYNC_PORT="$PORT"
  export MAER_SYNC_HOST MAER_SYNC_PORT
  starte() { ( cd "$WURZEL" && "$RUF" -m service serve > "$TMP/protokoll.txt" 2>&1 & echo $! > "$TMP/pid" )
             PID=$(cat "$TMP/pid"); }
fi

# --- Die Fixtur anlegen, mit denselben Handgriffen wie der Erzeuger --------
if [ "$WELCHE" = rust ]; then
  "$RUF" key --new > "$TMP/anlage.txt" 2>&1
  printf '%s\n' "$GEHEIM" | "$RUF" account "$KONTO" >> "$TMP/anlage.txt" 2>&1
else
  ( cd "$WURZEL" && "$RUF" -m service key --new ) > "$TMP/anlage.txt" 2>&1
  ( cd "$WURZEL" && printf '%s\n' "$GEHEIM" | "$RUF" -m service account "$KONTO" ) \
    >> "$TMP/anlage.txt" 2>&1
fi
ABDRUCK=$(grep -ohE '[0-9A-Z]{4}(-[0-9A-Z]{4}){5}' "$TMP/anlage.txt" | head -1)
[ -n "$ABDRUCK" ] || { echo "HALT: kein Abdruck in der Anlage." >&2; exit 1; }

starte
sleep 2

# GUTFALL ZUERST, UND ER IDENTIFIZIERT DEN GEGENSTAND: nicht "antwortet
# etwas", sondern "antwortet das, was wir gerade angelegt haben". Am
# 18.09.2026 hat ein Lauf gegen einen belegten Port neun Statuszahlen von
# einem fremden Dienst gemessen, und alle sahen plausibel aus.
GEMELDET=$(curl -s --max-time 5 "http://127.0.0.1:$PORT/v1/key" \
  | grep -ohE '[0-9A-Z]{4}(-[0-9A-Z]{4}){5}' | head -1 || true)
if [ "$GEMELDET" != "$ABDRUCK" ]; then
  echo "HALT: es antwortet '${GEMELDET:-nichts}', angelegt wurde $ABDRUCK." >&2
  sed 's/^/  /' "$TMP/protokoll.txt" >&2
  exit 1
fi
echo "$WELCHE auf Port $PORT antwortet als SICH SELBST ($ABDRUCK)"

# Ein Zeichen holen und den einen Schrieb hinlegen, den der Erzeuger hinlegt.
ZEICHEN=$(curl -s --max-time 5 -X POST "http://127.0.0.1:$PORT/v1/token" \
  -H 'content-type: application/json' \
  -d "{\"owner\":\"$KONTO\",\"secret\":\"$GEHEIM\",\"device\":\"$GERAET\"}" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin).get("token",""))')
[ -n "$ZEICHEN" ] || { echo "HALT: kein Zeichen zu bekommen." >&2; exit 1; }

STATUS=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
  -X PUT "http://127.0.0.1:$PORT/v1/doc/playback?scope=$GERAET" \
  -H "authorization: Bearer $ZEICHEN" -H 'x-maer-format: 1' -d '{"a": 1}')
[ "$STATUS" = 200 ] || { echo "HALT: der Fixtur-Schrieb gab $STATUS statt 200." >&2; exit 1; }
echo "Fixtur liegt: Konto $KONTO, ein Dokument im Bereich $GERAET"

# --- Und jetzt die Tafel ---------------------------------------------------
python3 "$WURZEL/werkzeug/gegenprobe.py" --gegen "http://127.0.0.1:$PORT" \
  --zeichen "$ZEICHEN" > "$TMP/gegenprobe.txt" 2>&1 || true
grep -E "^ *gleich:" "$TMP/gegenprobe.txt" || tail -3 "$TMP/gegenprobe.txt"
grep -E "ABWEICHUNG" "$TMP/gegenprobe.txt" | sed 's/^/  /' || true
cp "$TMP/gegenprobe.txt" "/tmp/wegeprobe-$WELCHE.txt"
echo "die ganze Ausgabe: /tmp/wegeprobe-$WELCHE.txt"
