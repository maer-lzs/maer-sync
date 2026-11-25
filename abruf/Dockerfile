# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

# The fetch program of the catalog index as a container, built like the
# service (same base image digest and musl-dev pin, held by the witness
# abruf/tests/zaeune.rs). The context is the repository root, because the
# fetch program builds against the service's library and reads the shelf list
# of the vectors. The result holds the binary, the catalog directory of its
# own uid and the licence texts, nothing else.
FROM rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118 AS bau
RUN apk add --no-cache musl-dev~1.2.5
WORKDIR /bau
ENV CARGO_HOME=/bau/ziel/cargo-home \
    CARGO_TARGET_DIR=/bau/ziel/abbau
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY vektoren/katalog/schellack/regale.json ./vektoren/katalog/schellack/regale.json
COPY abruf/Cargo.toml abruf/Cargo.lock ./abruf/
COPY abruf/src ./abruf/src
RUN set -eu; \
    case "$(uname -m)" in \
      x86_64)  ziel=x86_64-unknown-linux-musl;  schalter=CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS ;; \
      aarch64) ziel=aarch64-unknown-linux-musl; schalter=CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS ;; \
      *) echo "unknown architecture $(uname -m)" >&2; exit 1 ;; \
    esac; \
    env "$schalter=-C target-feature=+crt-static" cargo build --release --locked --manifest-path abruf/Cargo.toml --target "$ziel"; \
    cp "$CARGO_TARGET_DIR/$ziel/release/maer-sync-abruf" /maer-sync-abruf; \
    mkdir -p /leer/var/lib/maer-sync-katalog

FROM scratch
COPY --from=bau /maer-sync-abruf /maer-sync-abruf
# The catalog directory belongs to the fixed uid of the fetch program; the
# service reads the same volume read only under its own uid.
COPY --from=bau --chown=10002:10002 /leer/var/lib/maer-sync-katalog /var/lib/maer-sync-katalog
COPY LICENSE LICENSE-ADDITIONAL-TERMS abruf/FREMDTEILE.md /
ENV MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db
USER 10002:10002
VOLUME ["/var/lib/maer-sync-katalog"]
ENTRYPOINT ["/maer-sync-abruf"]
