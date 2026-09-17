# Eine Schicht auf dem abgenommenen Abbild, nichts weiter.
#
# WARUM sie noetig ist, gemessen und nicht vermutet: `rust:1.85-alpine` bringt
# `gcc` mit, aber nicht `musl-dev`. Jede Crate mit einem Proc-Macro (hier
# `curve25519-dalek-derive`) wird als Shared Object fuer den Wirt gebaut, und
# dabei sucht `ld` die Startdateien der C-Bibliothek. Ohne `musl-dev` bricht es
# mit »cannot find crti.o« ab, und zwar bei BEIDEN Crate-Serien, also ist es
# kein Crate-Problem. Mit `musl-dev` baut derselbe Satz durch.
#
# Die Fassung von `musl-dev` haengt am Grundabbild, und das ist am Digest
# gepinnt; es gibt hier also keine zweite bewegliche Angabe.
FROM rust@sha256:4333721398de61f53ccbe53b0b855bcc4bb49e55828e8f652d7a8ac33dd0c118
RUN apk add --no-cache musl-dev
