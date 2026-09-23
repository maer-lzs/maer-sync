# Die Vektoren, bevor eine Zeile Rust entsteht

## Eingefroren seit dem 24.09.2026

**Die Dateien hier sind eine AUFZEICHNUNG, und sie wachsen nicht mehr.** Jede
bis auf `noise-nk.json` wurde von einem Erzeuger gezogen, der die
Python-Fassung des Dienstes fuhr und aufschrieb, was sie antwortete. Am
24.09.2026 ging die Python-Fassung aus diesem Repo (ENTSCHEID des Betreibers),
und mit ihr die Erzeuger und ihr Zeuge `tests/test_erzeuger.py`, der jede Datei
byte-gleich gegen einen frischen Lauf hielt. Der letzte Stand, an dem sie
liefen, ist `6415e8c`; dort sind sie nachzulesen und zu fahren.

**Was die Dateien damit noch sagen:** so hat Fassung 5 des Vertrags auf der
Leitung geantwortet, gemessen an einer zweiten, unabhaengigen Umsetzung. Die
Rust-Seite haelt sich weiter Fall fuer Fall daran. **Was sie nicht mehr
sagen:** wie sich eine NEUE Fassung verhalten soll. Eine Aenderung am Dienst,
die einen dieser Faelle bewegt, ist entweder ein Fehler oder eine neue Fassung
des Vertrags; im zweiten Fall bekommt der Fall eine Ausnahme mit Grund im
Zeugen, und die Datei bleibt, wie sie gemessen wurde.

**Die erwarteten Antworten einer neuen Fassung schreibt nicht, wer den Dienst
baut.** Ohne zweite Umsetzung fehlt die Gegenprobe; an ihre Stelle tritt ein
anderer Autor, der die Faelle aus dem Vertrag schreibt, und ein Lauf der App
selbst gegen den Dienst.

**Eine Vektordatei ohne Erzeuger ist eine Behauptung** (Entscheid 18.09.2026), und so hat es gegolten, solange es die Python-Fassung gab.
Die sieben Dateien der Scheiben 1 bis 3 haben ihren seit dem **19.09.2026**, und
sie sind dabei **byte-gleich geblieben**: jeder Erzeuger zieht genau die Datei,
die vorher von Hand danebenlag, mit derselben Fallzahl. Das ist das Ergebnis und
nicht die Voraussetzung gewesen, denn eine Abweichung haette geheissen, dass eine
der Zahlen falsch war.

**Ohne Erzeuger bleibt genau eine Datei, und sie braucht keinen:**
`noise-nk.json` kommt von aussen. Sie nachzuziehen hiesse, die
veroeffentlichten Vektoren durch unsere eigene Rechnung zu ersetzen, und damit
waere genau das weg, was sie wertvoll macht.

Scheibe 1 von H3 ist nicht das Geruest, sondern **diese Zahlen**. Der Auftrag sagt
es so: die Rahmen, die Abdruecke und der Noise-Handschlag muessen in Rust
byte-gleich herauskommen, BEVOR ein Pfad gebaut wird. Damit das eine Messung
wird und keine Selbstbestaetigung, liegen sie hier als Daten, getrennt von jeder
Implementierung.

## Was hier liegt, und wie stark es jeweils ist

| Datei | Herkunft | Staerke |
|---|---|---|
| `noise-nk.json` | **veroeffentlichte Vektoren**, unveraendert (bis `6415e8c` lag dieselbe Datei auch unter `tests/vectors/`, fuer die Python-Seite) | am staerksten: sie kommen von ausserhalb, zwei Fassungen, die keine unserer Umsetzungen kennt |
| `abdruecke.json` | aus `core/identity.py`, **und gegengerechnet** mit einer unabhaengigen Crockford-Umsetzung im Erzeuger | mittel: das Verfahren ist SHA-256 plus eine veroeffentlichte Alphabet-Tabelle, also von Dritten nachrechenbar |
| `rahmen.json` | aus `core/frames.py` | am schwaechsten: das Format ist unser eigenes, die Zahlen halten den heutigen Stand fest |
| `protokoll.json` | **an der laufenden Anwendung gemessen**, der Protokoll-Handler mitgeschrieben | mittel: die Saetze sind unsere eigenen, aber nichts darin ist behauptet |
| `befehle.json` | **an der echten Python-CLI gemessen**, jeder Befehl gefahren | mittel: Ausgang und Satz sind unsere eigenen, aber nichts darin ist behauptet |
| `abweisung.json` | **an der Python-Fassung unter uvicorn gemessen**, Aufstieg versucht und die Antwort gelesen | mittel: es misst, was ein fremder Server aus unserem Schliesscode macht, und genau das ist die Frage |
| `horcher.json` | **am laufenden Socket gemessen**, jeder Fall als echter ASGI-Socket gefahren | mittel: die Schliesscodes sind unsere eigene Tafel, aber nichts darin ist behauptet |
| `wege.json` | **an der laufenden `service/app.py` gemessen**, Fall fuer Fall gerufen | mittel: nichts darin ist behauptet, aber es misst unsere eigene Anwendung, also beweist es Gleichheit der zwei Fassungen und nicht Richtigkeit |
| `schreiben.json` | aus `core/versions.py`, `admission.py`, `formats.py`, `writes.py`, `paths.py`, jedes Tor gerufen | mittel: Urteil UND Satz stehen drin, also faellt auch eine falsche Begruendung auf; es bleibt unsere eigene Regel |
| `sitzungen.json` | aus `core/sessions.py`, jeder Rand gerufen | mittel: die Faelle sind Raender und Ordnungen, nicht Mitten; die Zahlen selbst sind unsere Wahl |
| `zeichen.json` | aus `core/authorization.py`, Salz eingespeist statt gezogen | mittel: die Abdruecke sind SHA-256 und nachrechenbar, die gespeicherte Form ist unser Format |
| `rueckkehr.json` | aus `core/restore.py`, jeder Ausgang gerufen | mittel: die drei Ausgaenge und ihre Saetze sind unsere eigenen, aber nichts darin ist behauptet |
| `kopf-und-schluessel.json` | aus `core/frames.py` und `core/identity.py`, jeder Fehlerfall wirklich gerufen | mittel: dass eine Abweisung abweist, ist gemessen; was sie sagt, ist unsere Wahl |

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

## Wie die Zeugen gefahren werden

**Auf dem Wirt liegt weiterhin keine Rust-Werkzeugkette**, weder `cargo` noch
`rustc`, auf Pi und Tiny geprueft. Das ist ein Entscheid und kein Mangel: gebaut
wird im gepinnten Abbild ueber `werkzeug/bau.sh`, und auf dem Wirt bleibt
nichts liegen.

*(Bis zum 19.09.2026 stand hier, es koenne deshalb niemand eine Rust-Zeile gruen
sehen. Das war einmal wahr und ist es seit der Bau-Vorrichtung nicht mehr; die
Zeugen laufen, die Zahl steht im jeweiligen Bericht. Ein Satz ueber einen
Zustand, den niemand misst, altert genau so.)*


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


## `abweisung.json`: was aus einem Schliesscode vor dem Annehmen wird

Ein `websocket.close` VOR dem `accept` traegt einen Code. Auf der Leitung gibt
es ihn nicht, weil es noch keine Leitung gibt: der Server macht daraus einen
HTTP-Status. **Gemessen** (Fassung 5 aus `main`, uvicorn 0.53.0): alle vier
Faelle werden `403` mit leerem Koerper, ob falscher Weg, fehlendes Zeichen,
unbekanntes Zeichen oder erfundene Sitzung. Der Code geht also im SERVER
verloren, nicht bei uns.

**Nur Status und Koerperlaenge stehen in der Datei, und das ist der Befund.**
Die zwei uvicorn-Fassungen, die hier lagen, schicken verschiedene Kopfzeilen:
0.52.4 mit `Connection: close`, `Content-Type: text/plain` und Grundphrase,
0.53.0 nur `content-length`. Wer die Kopfzeilen nachbaute, band sich an eine
Fassung eines fremden Servers.

**Die Koerperlaenge steht mit dabei, weil sie einen echten Unterschied
gefangen hat:** ohne `content-length: 0` waehlt hyper `chunked` und schickt
fuenf Byte Abschlussmarke, wo uvicorn null schickt. An der Schnittstelle ist
das unsichtbar, auf der Leitung nicht.


## `befehle.json`: was der Betreiber sieht

Vom Dienst sieht ein Betreiber zuerst diese Befehle, und was er sieht, ist ein
**Ausgangscode und ein Satz**. Ein Ausgang, der sich um eins verschiebt, macht
aus einer geglueckten Sicherung eine gescheiterte, und sein Zeitplan alarmiert
auf die falsche.

Die Faelle teilen sich Speicher: `account` zweimal ist nur dann ein Fall, wenn
das erste `account` davor lief. Die Tafel nennt deshalb je Fall ihren `lauf`,
und `werkzeug/befehlsprobe.py` stellt ihn nach.

**Zwei Sorten Text gehoeren nicht uns, und beide sind benannt statt
stillschweigend uebergangen.** Bei einem fehlenden Argument antwortet auf der
Python-Seite `argparse` mit einem Wortlaut, der den Python-Aufruf nennt
(`satz_von: argparse`, es wird nur der Ausgang verglichen). Und hinter
`the copy did not get written:` schreibt der Datenbanktreiber
(`satz_bis`, verglichen wird bis einschliesslich unseres Doppelpunkts): zwei
Treiber schreiben `unable to open database file` und
`unable to open database: <pfad>`, und das ist ein Unterschied in der Herkunft
des Textes, nicht im Verhalten.

**Der Gutfall faehrt die Tafel gegen die Fassung, aus der sie stammt.** Genau
das hat einen Fehler in der Maskierung gefangen: ein LEERES Geheimnis stand
als `<geheimnis>` da, und beim Abspielen wurde ein gueltiges eingesetzt, womit
der Fall »leeres Geheimnis« das Gegenteil dessen prueft, wofuer er dasteht.
