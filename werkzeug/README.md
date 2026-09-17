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

**Was hier NICHT liegt: eine Crate.** Der Auftrag haelt die Crate-Liste als
eigenen Entscheid fest, und bis der vorliegt, traegt der Zweig kein
`Cargo.toml` mit Abhaengigkeiten. Die gemessenen Baeume stehen im Bericht.
