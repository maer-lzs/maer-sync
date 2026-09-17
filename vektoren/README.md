# Die Vektoren, bevor eine Zeile Rust entsteht

Scheibe 1 von H3 ist nicht das Geruest, sondern **diese Zahlen**. Der Auftrag sagt
es so: die Rahmen, die Abdruecke und der Noise-Handschlag muessen in Rust
byte-gleich herauskommen, BEVOR ein Pfad gebaut wird. Damit das eine Messung
wird und keine Selbstbestaetigung, liegen sie hier als Daten, getrennt von jeder
Implementierung.

## Was hier liegt, und wie stark es jeweils ist

| Datei | Herkunft | Staerke |
|---|---|---|
| `noise-nk.json` | **veroeffentlichte Vektoren**, unveraendert aus `tests/vectors/` | am staerksten: sie kommen von ausserhalb, zwei Fassungen, die keine unserer Umsetzungen kennt |
| `abdruecke.json` | aus `core/identity.py`, **und gegengerechnet** mit einer unabhaengigen Crockford-Umsetzung im Erzeuger | mittel: das Verfahren ist SHA-256 plus eine veroeffentlichte Alphabet-Tabelle, also von Dritten nachrechenbar |
| `rahmen.json` | aus `core/frames.py` | am schwaechsten: das Format ist unser eigenes, die Zahlen halten den heutigen Stand fest |

**Diese Abstufung ist der Punkt.** Ein Vektor, den unsere Python-Fassung erzeugt
und unsere Rust-Fassung bestaetigt, beweist Gleichheit der beiden, nicht
Richtigkeit. Nur bei `noise-nk.json` sagt jemand Dritter, was richtig ist. Bei
den Abdruecken ist die Luecke geschlossen, indem der Erzeuger sie ein zweites
Mal aus SHA-256 und der Tabelle gerechnet hat, ohne `identity.py` zu rufen; alle
vier stimmen, und sie stimmen ausserdem mit den vier festen Werten in
`tests/test_identity.py`. Bei den Rahmen bleibt die Luecke offen und ist hier
benannt, statt sie wegzureden.

## Die Verfahren, in einem Satz je Datei

- **Abdruck:** SHA-256 des rohen 32-Byte-Schluessels, die ersten 15 Byte,
  Crockford-base32 (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`, ohne `I`, `L`, `O`, `U`),
  24 Zeichen in sechs Gruppen von vier. Ein Schluessel anderer Laenge wird
  abgewiesen, nicht abgedruckt.
- **Rahmen:** zwei Byte Kopflaenge big endian, dann ein JSON-Kopf mit sortierten
  Schluesseln und ohne Leerzeichen, dann der rohe Koerper. Der Koerper geht
  ausdruecklich nicht durch JSON. `ungueltig` fuehrt vier Formen von "kein
  Rahmen", die alle denselben Fehler ergeben muessen.
- **Handschlag:** `Noise_NK_25519_ChaChaPoly_SHA256`, die zwei veroeffentlichten
  Vektoren mit ihren Schluesseln und Nachrichten.

## Was noch NICHT gemessen ist

**Es gibt auf diesem Wirt keine Rust-Werkzeugkette** (weder `cargo` noch `rustc`,
auf Pi und Tiny geprueft). Solange sie fehlt, kann niemand hier eine Rust-Zeile
gruen sehen, und deshalb liegt hier auch keine. Nach Regel 7 ist das eine
Abstinenz mit Grund und kein gruener Haken: die Werkzeugkette und die Crates
sind Entscheide und liegen zur Vorlage.
