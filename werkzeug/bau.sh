#!/bin/sh
# Baut den Rust-Dienst im gepinnten Abbild, damit auf dem Wirt keine
# Werkzeugkette liegt.
#
# Gepinnt wird der INDEX-Digest, nicht das Etikett: `rust:1.85-alpine` wandert
# mit der naechsten Punktfassung, der Digest nicht. Der Index deckt
# linux/amd64 und linux/arm64/v8, also beide Wirte mit einer Angabe. Ein
# Wechsel des Digests ist ein neuer Abbild-Entscheid (Regel 4).
#
# Das Ziel wird NICHT gesetzt: das Abbild ist alpine, sein Vorgabe-Ziel ist
# schon `<arch>-unknown-linux-musl` (im Abbild gemessen). Ein zusaetzliches
# --target waere eine zweite Wahrheit, die auseinanderlaufen kann.
#
# CARGO_HOME liegt im Projekt, nicht im Heim des Wirts: der Bau soll nichts
# ausserhalb des Baums hinterlassen, und "Rueckweg" heisst hier wegwerfen.
set -eu

ABBILD="rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118"
WURZEL=$(cd "$(dirname "$0")/.." && pwd)

command -v docker >/dev/null 2>&1 || { echo "bau: docker fehlt" >&2; exit 1; }

# Fail-closed: ohne das gepinnte Abbild wird NICHT auf ein Etikett
# zurueckgefallen, sonst baut irgendwann eine andere Fassung unbemerkt.
docker image inspect "$ABBILD" >/dev/null 2>&1 || {
  echo "bau: das gepinnte Abbild liegt nicht vor. Holen mit:" >&2
  echo "  docker pull $ABBILD" >&2
  exit 1
}

mkdir -p "$WURZEL/ziel/cargo-home"
exec docker run --rm \
  -v "$WURZEL:/bau" \
  -e CARGO_HOME=/bau/ziel/cargo-home \
  -e CARGO_TARGET_DIR=/bau/ziel/abbau \
  -w /bau \
  "$ABBILD" \
  cargo "$@"
