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
| `wege.json` | **an der laufenden `service/app.py` gemessen**, Fall fuer Fall gerufen | mittel: nichts darin ist behauptet, aber es misst unsere eigene Anwendung, also beweist es Gleichheit der zwei Fassungen und nicht Richtigkeit |

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

## Der Kopf: eine Bedeutung, zwei erlaubte Schreibweisen

**VERDIKT des Gegenlesers (18.09.2026), gemessen, nicht gemeint:** der Kopf ist
ein JSON-Objekt in UTF-8 mit sortierten Schluesseln. Nicht-ASCII darf **roh
oder entwertet** geschrieben werden, gelesen wird die **Bedeutung**. Es gibt
keine Byte-Kanonik, und die drei Umsetzungen schreiben heute schon verschieden:
`core/frames.py` entwertet (`json.dumps`, Vorgabe `ensure_ascii`),
`MaerSyncFrame.swift` und `src/frames.rs` schreiben rohes UTF-8. Der Dienst
liest beides. **Ein Leser, der nur eine Schreibweise annimmt, ist der Befund,
nicht der Umlaut.**

Deshalb traegt `rahmen.json` den Abschnitt `gleiche_bedeutung`: ein Rahmen mit
einem Umlaut in ZWEI gueltigen Schreibweisen (entwertet, 49 Byte Kopf; roh, 45)
und EINER Bedeutung. Der Zeuge prueft, dass beide dieselbe Bedeutung ergeben
und dass die eigene Schreibweise genau eine der beiden ist.

**Warum der Satz hier steht und nicht im Vertrag.** Er stand kurz in
`CONTRACT.md`, und das war falsch: Invariante 1 des Auftrags verlangt den
Vertrag **byte-gleich** in beiden Repos, Fassung 5, Digest `547d9a75…`. Ein
Satz dort hebt den Digest und laesst die App-Seite mit der alten Zahl stehen,
ohne dass einer der beiden Zaeune es merkt, weil jede Seite nur ihre Datei
gegen ihre eigene Konstante haelt. Die Vertragsfassung 6 mit diesem Satz ist
ein eigener Schritt auf `main`, mit der App-Konstante im selben Zug. Hier
beschreibt der Satz nur, was die Vektoren zeigen.

## Was noch NICHT gemessen ist

**Es gibt auf diesem Wirt keine Rust-Werkzeugkette** (weder `cargo` noch `rustc`,
auf Pi und Tiny geprueft). Solange sie fehlt, kann niemand hier eine Rust-Zeile
gruen sehen, und deshalb liegt hier auch keine. Nach Regel 7 ist das eine
Abstinenz mit Grund und kein gruener Haken: die Werkzeugkette und die Crates
sind Entscheide und liegen zur Vorlage.


## `wege.json`: die Reihenfolge der Tore

Diese Datei ist nicht von Hand geschrieben. Ein Erzeuger hat die ASGI-Anwendung
gebaut und **jeden Fall gerufen**, und was hier steht, ist die Antwort. Der
Unterschied ist nicht akademisch: beim Schreiben der Faelle hielt ich den
Rahmenkopf fuer `{method, path, query}`, er ist aber `{method, target}` mit dem
Fragezeichen darin, und sieben Tunnelfaelle kamen als "this is not a request"
zurueck, bis der Erzeuger es zeigte.

**Was die Datei festhaelt, ist die ORDNUNG, nicht die einzelne Antwort.** Drei
Stellen, an denen eine zweite Umsetzung aus Gewohnheit anders liegt:

- `/v1/health` antwortet **vor der Methodenpruefung**, also auch auf `POST`,
  und auch bei geschlossenen Wegen.
- Das Tor der alten Wege steht **vor** der Zeichenpruefung. Ohne sie ist eine
  Anfrage mit gueltigem Zeichen 426 und nicht 401, ein unbekannter Weg 426 und
  nicht 404, und die Anmeldung selbst liegt hinter dem Tor.
- Die Bereichspruefung steht **nach** der Wegerkennung. Ein unbekannter Weg mit
  kaputtem Bereich ist 404, nicht 400.

Der Abschnitt `tunnel` faehrt eine echte Sitzung von der Client-Seite. Der
Handschlag ist zufaellig, also stehen dort Status und Satz und nicht die Bytes;
die Bytes stehen in `noise-nk.json`. Zwei Dinge sind dort gemessen und nicht
vermutet: die Weigerung "this path is not answered inside a session" reist
**unversiegelt**, und eine **gelogene Laenge im Rahmenkopf zaehlt nicht**, weil
der wahre Koerper schon da ist.

**Das echte Zeichen steht nirgends in der Datei.** Maskiert ist der WERT und
nicht der Name der Kopfzeile: `Bearer nichtvergeben` und `Basic irgendwas`
tragen kein Geheimnis und stehen im Klartext. Der erste Versuch maskierte jede
`authorization`-Zeile, und damit wurden drei verschiedene Faelle zu einem; der
Zeuge fiel darueber, weil ein unbekanntes Zeichen dann als gueltiges einsetzte.
