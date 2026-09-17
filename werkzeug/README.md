# Die Bau-Vorrichtung

`bau.sh` reicht seine Argumente an `cargo` im gepinnten Abbild weiter:

    werkzeug/bau.sh --version
    werkzeug/bau.sh test
    werkzeug/bau.sh build --release

**Warum im Container.** Das Lieferstueck ist ein statisches Binary, und Debian
13 hat keine Rust-Standardbibliothek fuer musl (gemessen an `apt-cache`:
`libstd-rust-dev` ohne musl-Ziel). Das Abbild `rust:1.85-alpine` bringt musl
nativ mit, sein Vorgabe-Ziel ist `x86_64-unknown-linux-musl` auf dem Tiny und
`aarch64-unknown-linux-musl` auf dem Pi. Beide Wirte bleiben ohne
Werkzeugkette, und der Rueckweg heisst Abbild wegwerfen.

**Gepinnt am Index-Digest**, nicht am Etikett, und fail-closed: liegt das
Abbild nicht vor, bricht der Bau ab und nennt den Holbefehl, statt auf ein
Etikett auszuweichen.

**Eine eigene Schicht, und nur eine.** `Dockerfile` setzt auf das gepinnte
Grundabbild genau ein Paket: `musl-dev`. Gemessen und nicht vermutet: das
Grundabbild bringt `gcc` mit, aber nicht `musl-dev`, und jede Crate mit einem
Proc-Macro (hier `curve25519-dalek-derive`) wird als Shared Object fuer den Wirt
gebaut, wobei `ld` die Startdateien der C-Bibliothek sucht. Ohne `musl-dev`
bricht das mit `cannot find crti.o` ab, **bei beiden Crate-Serien**, es ist also
kein Crate-Problem. `bau.sh` baut die Schicht beim ersten Aufruf selbst.

**Die Schicht ist die zweite bewegliche Angabe, und sie ist gepinnt.** Der
frueheste Satz hier behauptete, ihre Paketfassungen haengen am Digest des
Grundabbilds; das war falsch. `apk add` liest den Index zur Bauzeit aus dem
Netz. Gemessen: im Abbild liegt `musl 1.2.5-r9`, der Index bietet heute
`1.2.5-r11`. Deshalb steht die Fassung im `Dockerfile`, fail-closed, und ein
Wechsel ist ein Commit mit Grund, wie beim Digest. Nebenwirkung, benannt:
`musl-dev=1.2.5-r11` hebt `musl` selbst von r9 auf r11.

**Statisch heisst: Ziel setzen UND `+crt-static`.** Gemessen, und der frueheste
Stand dieses Skripts hatte es anders begruendet: ohne gesetztes Ziel gilt der
Schalter auch fuer Proc-Macros, und der Bau bricht ab. Mit gesetztem Ziel gilt
er nur fuer das Ziel. Der Beweis ist nicht `ldd` (das nennt bei static-pie den
Lader ohnehin), sondern **ein Lauf in einem Abbild ohne musl**: dort startet das
Erzeugnis und rechnet die Vektoren nach, waehrend die Fassung ohne Schalter
nicht einmal anlaeuft.

**Was hier NICHT liegt: eine Crate.** Der Auftrag haelt die Crate-Liste als
eigenen Entscheid fest, und bis der vorliegt, traegt der Zweig kein
`Cargo.toml` mit Abhaengigkeiten. Die gemessenen Baeume stehen im Bericht.
