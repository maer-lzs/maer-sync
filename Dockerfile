# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

# Mär Sync Service als Container: zwei Stufen, und im Ergebnis NUR das Binary.
#
# Stufe 1 baut das Binary in DERSELBEN Umgebung wie `werkzeug/bau.sh`: das
# Grundabbild am Index-Digest, `musl-dev` in der gepinnten Fassung, das Ziel
# ausdruecklich gesetzt und `+crt-static` NUR fuer das Ziel (sonst bricht der
# Bau am Proc-Macro ab, siehe `werkzeug/README.md`). Die beiden Pins stehen hier
# ein zweites Mal, weil ein Dockerfile kein anderes einbinden kann; der Zeuge
# `tests/abbild.rs` haelt sie gegen `werkzeug/Dockerfile`, damit sie nicht
# auseinanderlaufen.
#
# Stufe 2 ist leer (`FROM scratch`): kein Shell, keine libc, kein Paket. Das
# Binary ist statisch und braucht nichts davon. Was darin liegt, ist das
# Binary, das Datenverzeichnis fuer die UID 10001 und die zwei Lizenztexte.
FROM rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118 AS bau
RUN apk add --no-cache musl-dev=1.2.5-r11
WORKDIR /bau
# Dieselben Pfade wie `bau.sh`: die Pfade der Quellen stehen im Binary (in den
# Meldungen einer Panik), und mit anderen Pfaden waere das Binary im Abbild ein
# anderes als das, das ein Wirt per Kopie bekommt.
ENV CARGO_HOME=/bau/ziel/cargo-home \
    CARGO_TARGET_DIR=/bau/ziel/abbau
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# Das Ziel je Bauwirt, wie in `werkzeug/bau.sh`: der Tiny baut x86_64, der Pi
# aarch64, und gekreuzt wird nicht. `--locked`: das Lockfile ist die Zusage,
# und ein Bau, der es aendern wollte, soll abbrechen statt still aufzuloesen.
RUN set -eu; \
    case "$(uname -m)" in \
      x86_64)  ziel=x86_64-unknown-linux-musl;  schalter=CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS ;; \
      aarch64) ziel=aarch64-unknown-linux-musl; schalter=CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS ;; \
      *) echo "unbekannte Architektur $(uname -m)" >&2; exit 1 ;; \
    esac; \
    env "$schalter=-C target-feature=+crt-static" cargo build --release --locked --target "$ziel"; \
    cp "$CARGO_TARGET_DIR/$ziel/release/maer-sync" /maer-sync; \
    mkdir -p /leer/var/lib/maer-sync /leer/tmp

FROM scratch
COPY --from=bau /maer-sync /maer-sync
# Ein /tmp, weil `backup` seine Rueckspielprobe dort oeffnet
# (`std::env::temp_dir()` in `src/befehle.rs`). Ohne es endete jede Sicherung im
# Container mit `not measured … Permission denied`: gemessen am 24.09.2026, und
# richtig gemeldet, weil die Probe dann nicht »fits« sagt. Es gehoert der UID
# des Dienstes: ein `chmod 1777` in der Baustufe kam beim ersten Versuch NICHT
# im Abbild an (`docker export` zeigte `drwxr-xr-x root`), die Eigentuemerschaft
# traegt dagegen nachweislich, wie beim Datenverzeichnis darunter.
COPY --from=bau --chown=10001:10001 /leer/tmp /tmp
# Das Datenverzeichnis gehoert der festen UID, denn das Volume gehoert einer
# UID, und die muss jeden Neubau des Abbilds ueberleben.
COPY --from=bau --chown=10001:10001 /leer/var/lib/maer-sync /var/lib/maer-sync
COPY LICENSE LICENSE-ADDITIONAL-TERMS /

# Die Rust-Fassung liest EINE Adresse, `MAER_SYNC_BIND`, und kennt
# `MAER_SYNC_HOST` und `MAER_SYNC_PORT` nicht (null Vorkommen in `src/`).
# Gesetzt wie die frueheren zwei: alle Adressen im Container, Port 8099.
ENV MAER_SYNC_DB=/var/lib/maer-sync/store.sqlite \
    MAER_SYNC_BIND=0.0.0.0:8099

USER 10001:10001
VOLUME ["/var/lib/maer-sync"]
EXPOSE 8099
ENTRYPOINT ["/maer-sync"]
CMD ["serve"]
