# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

# Eine Schicht auf dem abgenommenen Grundabbild, mit gepinnter Paketfassung.
#
# WARUM sie noetig ist, gemessen: `rust:1.85-alpine` bringt `gcc` mit, aber
# nicht `musl-dev`. Jede Crate mit einem Proc-Macro (hier
# `curve25519-dalek-derive`) wird als Shared Object fuer den Wirt gebaut, und
# dabei sucht `ld` die Startdateien der C-Bibliothek. Ohne `musl-dev` bricht es
# mit »cannot find crti.o« ab, bei beiden Crate-Serien, es ist also kein
# Crate-Problem. Der Weg ohne Paket (`-C link-self-contained=yes`, damit rustc
# seine eigenen Startdateien anbietet) ist geprobt und traegt nicht.
#
# WARUM DIE FASSUNG DASTEHT: `apk add` ohne Fassung liest den Paketindex zur
# BAUZEIT aus dem Netz. Der Digest des Grundabbilds pinnt das NICHT, er pinnt
# nur, was im Abbild schon liegt. Gemessen: im Abbild liegt `musl 1.2.5-r9`
# (vom 2025-03-19), der Index von v3.21/main bot am 24.09.2026 `1.2.5-r11`. Ohne
# Fassung bekaemen zwei Wirte an zwei Tagen zwei verschiedene Abbilder, und
# genau das soll der Digest verhindern. Ein Fassungswechsel ist deshalb ein
# Commit mit Grund, wie beim Digest.
#
# NUR DIE FASSUNG, NICHT DIE REVISION: Alpine haelt je Fassung nur die neueste
# Revision im Index. Der Pin auf `1.2.5-r11` brach am 08.11.2026 jeden Bau, weil
# der Index nur noch `1.2.5-r12` fuehrte (`breaks: world[musl-dev=1.2.5-r11]`).
# `musl-dev~1.2.5` haelt die Fassung fest und nimmt die Revision, die der Index
# hat; eine Revision ist bei Alpine eine Korrektur derselben Fassung. Zwei Wirte
# an zwei Tagen koennen also zwei Revisionen bekommen, nie zwei Fassungen.
#
# NEBENWIRKUNG, benannt: `musl-dev` zieht `musl` auf dieselbe Revision mit.
# Die Schicht hebt also die C-Bibliothek des Bauwirts, nicht nur ihre Kopfdateien.
#
# Fail-closed: liegt die Fassung 1.2.5 nicht mehr im Index, bricht der Bau ab,
# statt eine andere zu nehmen.
FROM rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118
RUN apk add --no-cache musl-dev~1.2.5
