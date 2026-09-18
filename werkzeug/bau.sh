#!/bin/sh
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
# Baut den Rust-Dienst im gepinnten Abbild, damit auf dem Wirt keine
# Werkzeugkette liegt.
#
# Gepinnt wird der INDEX-Digest, nicht das Etikett: `rust:1.85-alpine` wandert
# mit der naechsten Punktfassung, der Digest nicht. Der Index deckt
# linux/amd64 und linux/arm64/v8, also beide Wirte mit einer Angabe. Ein
# Wechsel des Digests ist ein neuer Abbild-Entscheid (Regel 4).
#
# Das Ziel wird AUSDRUECKLICH gesetzt, und der frueheste Stand dieses Skripts
# begruendete das Gegenteil. Gemessen: ohne `--target` gilt
# `-C target-feature=+crt-static` auch fuer Proc-Macros, und dann bricht der Bau
# ab (»cannot produce proc-macro for curve25519-dalek-derive as the target
# x86_64-unknown-linux-musl does not support these crate types«). Mit gesetztem
# Ziel gelten die Schalter nur fuer das Ziel, das Proc-Macro baut weiter, und
# erst dann entsteht ein Erzeugnis, das ohne musl laeuft.
#
# Ohne `+crt-static` ist das Erzeugnis NICHT statisch: `ldd` nennt den
# musl-Lader, und in einem glibc-Abbild startet es nicht. Mit dem Schalter
# laeuft dasselbe Erzeugnis dort und rechnet die Vektoren nach. `ldd` allein
# taugt als Beweis nicht, weil es bei static-pie den Lader ohnehin nennt.
#
# CARGO_HOME liegt im Projekt, nicht im Heim des Wirts: der Bau soll nichts
# ausserhalb des Baums hinterlassen, und "Rueckweg" heisst hier wegwerfen.
set -eu

GRUND="rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118"
ABBILD="maer-sync-bau:1.85.1"
WURZEL=$(cd "$(dirname "$0")/.." && pwd)

command -v docker >/dev/null 2>&1 || { echo "bau: docker fehlt" >&2; exit 1; }

# ERREICHBARKEIT VOR VORHANDENSEIN. Ohne diesen Schritt sieht ein Zugriffs-
# fehler genauso aus wie ein fehlendes Abbild: `docker image inspect` gibt in
# beiden Faellen 1, und die Meldung, die den Unterschied traegt, faellt in
# `2>/dev/null`.
#
# GENAU DAS IST PASSIERT (19.09.2026): das sudo-Fenster war abgelaufen, die
# Schleuse bekam »permission denied«, und diese Vorrichtung meldete »das
# gepinnte Grundabbild liegt nicht vor«. Das Abbild lag die ganze Zeit da. Der
# Fehlschluss ging als Befund in einen Bericht und in den Kanal.
if ! fehler=$(docker version --format '{{.Server.Version}}' 2>&1); then
  echo "bau: docker antwortet nicht, es wurde NICHTS ueber die Abbilder gemessen:" >&2
  echo "  $fehler" >&2
  exit 1
fi

# Fail-closed: ohne das gepinnte Grundabbild wird NICHT auf ein Etikett
# zurueckgefallen, sonst baut irgendwann eine andere Fassung unbemerkt.
docker image inspect "$GRUND" >/dev/null 2>&1 || {
  echo "bau: das gepinnte Grundabbild liegt nicht vor. Holen mit:" >&2
  echo "  docker pull $GRUND" >&2
  exit 1
}

# Die eigene Schicht: nur `musl-dev`, ohne das kein Proc-Macro linkt
# (gemessen: »cannot find crti.o«). Sie entsteht aus dem gepinnten Grundabbild,
# also gibt es keine zweite bewegliche Angabe.
docker image inspect "$ABBILD" >/dev/null 2>&1 || {
  echo "bau: baue die Schicht $ABBILD aus $GRUND" >&2
  docker build -q -t "$ABBILD" -f "$(dirname "$0")/Dockerfile" "$(dirname "$0")" >/dev/null \
    || { echo "bau: die Schicht liess sich nicht bauen" >&2; exit 1; }
}

# Ziel je Wirt: der Tiny ist x86_64, der Pi ist aarch64, jedes Ziel hat also
# einen eigenen Bauwirt und es gibt keine Kreuzkompilierung.
case "$(uname -m)" in
  x86_64)  ZIEL=x86_64-unknown-linux-musl;  SCHALTER=CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS ;;
  aarch64) ZIEL=aarch64-unknown-linux-musl; SCHALTER=CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS ;;
  *) echo "bau: unbekannte Architektur $(uname -m)" >&2; exit 1 ;;
esac

mkdir -p "$WURZEL/ziel/cargo-home"
exec docker run --rm \
  -v "$WURZEL:/bau" \
  -e CARGO_HOME=/bau/ziel/cargo-home \
  -e CARGO_TARGET_DIR=/bau/ziel/abbau \
  -e CARGO_BUILD_TARGET="$ZIEL" \
  -e "$SCHALTER=-C target-feature=+crt-static" \
  -w /bau \
  "$ABBILD" \
  cargo "$@"
