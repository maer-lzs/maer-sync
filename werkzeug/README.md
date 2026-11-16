# Die Bau-Vorrichtung

`bau.sh` reicht seine Argumente an `cargo` im gepinnten Abbild weiter:

    werkzeug/bau.sh --version
    werkzeug/bau.sh test
    werkzeug/bau.sh build --release

**Warum im Container.** Das Lieferstueck ist ein statisches Binary, und Debian
13 hat keine Rust-Standardbibliothek fuer musl (gemessen an `apt-cache`:
`libstd-rust-dev` ohne musl-Ziel). Das Abbild `rust:1.85-alpine` bringt musl
nativ mit, sein Vorgabe-Ziel ist `x86_64-unknown-linux-musl` auf dem x86_64-Wirt und
`aarch64-unknown-linux-musl` auf dem aarch64-Wirt. Beide Wirte bleiben ohne
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
Netz. Gemessen: im Abbild liegt `musl 1.2.5-r9`, der Index bot am 24.09.2026
`1.2.5-r11`. Deshalb steht die Fassung im `Dockerfile`, fail-closed, und ein
Wechsel ist ein Commit mit Grund, wie beim Digest. Nebenwirkung, benannt:
`musl-dev` hebt `musl` selbst auf seine Revision.

**Gepinnt ist die Fassung, nicht die Revision** (`musl-dev~1.2.5`). Alpine
haelt je Fassung nur die neueste Revision im Index. Der Pin auf `1.2.5-r11`
brach am 08.11.2026 jeden Bau, auch den oeffentlichen Weg, weil der Index nur
noch `1.2.5-r12` fuehrte. Eine Revision ist eine Korrektur derselben Fassung;
zwei Wirte koennen also zwei Revisionen bekommen, nie zwei Fassungen. Fest am
Digest bleibt das Grundabbild mit seiner Rust-Kette. `tests/abbild.rs` haelt
beide Dockerfiles gleich und weist einen Pin mit `=` ab.

**Statisch heisst: Ziel setzen UND `+crt-static`.** Gemessen, und der frueheste
Stand dieses Skripts hatte es anders begruendet: ohne gesetztes Ziel gilt der
Schalter auch fuer Proc-Macros, und der Bau bricht ab. Mit gesetztem Ziel gilt
er nur fuer das Ziel. Der Beweis ist nicht `ldd` (das nennt bei static-pie den
Lader ohnehin), sondern **ein Lauf in einem Abbild ohne musl**: dort startet das
Erzeugnis und rechnet die Vektoren nach, waehrend die Fassung ohne Schalter
nicht einmal anlaeuft.

**Die Suite laeuft NACH `git add`, nicht davor.** Der Lizenz-Zaun fragt git,
was VERFOLGT ist; eine Datei, die nur angelegt wurde, sieht er nicht. Wer die
Suite vor dem Hinzufuegen faehrt, bekommt gruen und committet die Luecke. Genau
das ist hier zweimal passiert, beim zweiten Mal einen Schritt frueher als beim
ersten. Reihenfolge also: `git add -A`, dann die Zaeune des Repos und
`bau.sh test`, dann committen. `vor-dem-commit.sh` faehrt genau das, in dieser
Reihenfolge, und haelt bei Rot an.

**Die Zaeune des Repos bleiben in Python, und nur sie.** Seit dem 24.09.2026
liegt hier kein Dienst mehr in Python. Was bleibt, sind `tests/test_license.py`
und `tests/test_repo_hygiene.py` mit ihrem gemeinsamen `tests/gitquery.py`: sie
fragen `git`, was verfolgt ist, und `git` gibt es auf dem Wirt, nicht im
Bau-Abbild der Rust-Suite. Sie brauchen nur die Standardbibliothek.

**Die Crates stehen in `Cargo.toml` und `Cargo.lock`, und jede ist einzeln
entschieden.** Eine neue Crate ist ein Entscheid vor dem Commit, nicht ein
Commit mit Begruendung: Kandidat mit Baum, Lizenz und Pflegestand vorlegen,
`Cargo.toml` erst nach dem Wort.

## Zwei Dockerfiles, und warum

`werkzeug/Dockerfile` ist die Bau-Schicht fuer `bau.sh`: das gepinnte
Grundabbild plus `musl-dev`, sonst nichts. Das `Dockerfile` an der Wurzel ist
das Betriebsabbild: es baut in derselben Umgebung und legt am Ende NUR das
Binary in ein leeres Abbild (`FROM scratch`). Ein Dockerfile kann kein anderes
einbinden, also stehen der Digest und die `musl-dev`-Fassung zweimal da.
`tests/abbild.rs` haelt beide Stellen und `bau.sh` gleich; wer einen Pin
bewegt, bewegt alle drei, oder die Suite wird rot.

## Die Proben gegen einen LAUFENDEN Dienst

    python3 werkzeug/gegenprobe.py --gegen http://127.0.0.1:8081 --zeichen <wert>
    python3 werkzeug/horchprobe.py --gegen http://127.0.0.1:8081 --zeichen <wert>

`gegenprobe.py` spielt `vektoren/wege.json` ueber eine echte Leitung ab,
`horchprobe.py` die Horcher-Tafel ueber einen echten WebSocket. Beide fahren den
Tunnel als KLIENT und bringen dafuer die Klient-Seite mit: `klient/frames.py`
und `klient/handshake.py`. Die zwei Dateien lagen bis zum 24.09.2026 in `core/`
neben der Python-Fassung des Dienstes; mit ihr ging der Rest, und die Proben
behielten, was sie als Klient brauchen.

**Sie brauchen `cryptography`, und nur sie.** Der Handschlag braucht X25519 und
ChaCha20-Poly1305, und beides hat die Standardbibliothek nicht. Der Dienst
selbst braucht davon nichts. Eine `requirements.txt` gibt es seit dem Auszug der
Python-Fassung nicht mehr; wer die Proben faehrt, nimmt das Paket seines
Systems (auf Debian `python3-cryptography`) oder eine eigene Umgebung.

**`wege.json` ist eingefroren** (siehe `vektoren/README.md`). Die Gegenprobe
misst also gegen die Fassung 5, wie sie am 18.09.2026 gemessen wurde. Aendert
eine spaetere Fassung einen Fall bewusst, bekommt er eine Ausnahme mit Grund,
und die Tafel bleibt, wie sie gemessen wurde.
