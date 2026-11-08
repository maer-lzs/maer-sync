# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

# Mär Sync Service as a container: two stages, and the result holds ONLY the binary.
#
# Stage 1 builds the binary in THE SAME environment as `werkzeug/bau.sh`: the
# base image at its index digest, `musl-dev` at its pinned version, the target
# set explicitly and `+crt-static` ONLY for the target (otherwise the build
# breaks at the proc macro, see `werkzeug/README.md`). The two pins stand here a
# second time because one Dockerfile cannot include another; the witness
# `tests/abbild.rs` holds them against `werkzeug/Dockerfile` so they cannot
# drift apart.
#
# Stage 2 is empty (`FROM scratch`): no shell, no libc, no package. The binary
# is static and needs none of them. What it holds is the binary, the data
# directory for uid 10001 and the two license texts.
FROM rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118 AS bau
RUN apk add --no-cache musl-dev=1.2.5-r11
WORKDIR /bau
# The same paths as `bau.sh`: the paths of the sources stand in the binary (in
# the messages of a panic), and with other paths the binary in the image would
# be another one than the one a host gets by copy.
ENV CARGO_HOME=/bau/ziel/cargo-home \
    CARGO_TARGET_DIR=/bau/ziel/abbau
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# The target per build host, as in `werkzeug/bau.sh`: an x86_64 host builds
# x86_64, an aarch64 host builds aarch64, and nothing is cross compiled.
# `--locked`: the lockfile is the promise, and a build that wanted to change it
# should stop instead of quietly resolving.
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
# A /tmp, because `backup` opens its restore rehearsal there
# (`std::env::temp_dir()` in `src/befehle.rs`). Without it every backup in the
# container ended in `not measured … Permission denied`: measured on 24.09.2026,
# and reported correctly, because the rehearsal then does not say "fits". It
# belongs to the service's uid: a `chmod 1777` in the build stage did NOT arrive
# in the image on the first attempt (`docker export` showed `drwxr-xr-x root`),
# while the ownership demonstrably does, as for the data directory below.
COPY --from=bau --chown=10001:10001 /leer/tmp /tmp
# The data directory belongs to the fixed uid, because the volume belongs to a
# uid, and that uid has to survive every rebuild of the image.
COPY --from=bau --chown=10001:10001 /leer/var/lib/maer-sync /var/lib/maer-sync
COPY LICENSE LICENSE-ADDITIONAL-TERMS /

# The Rust build reads ONE address, `MAER_SYNC_BIND`, and does not know
# `MAER_SYNC_HOST` or `MAER_SYNC_PORT` (no occurrence in `src/`).
# Set like the earlier two: every address in the container, port 8099.
ENV MAER_SYNC_DB=/var/lib/maer-sync/store.sqlite \
    MAER_SYNC_BIND=0.0.0.0:8099

USER 10001:10001
VOLUME ["/var/lib/maer-sync"]
EXPOSE 8099
ENTRYPOINT ["/maer-sync"]
CMD ["serve"]
