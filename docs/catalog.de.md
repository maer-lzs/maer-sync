# Der Katalog-Index (optional)

Was das Abrufprogramm fragt, was es speichert und wie du es ein- und ausschaltest. Zurück zur [README](../README.de.md).

## Katalog-Index (optional)

**Aus, solange du ihn nicht einschaltest.** Ein Wirt kann einen Katalog-Index
öffentlicher Quellen tragen, damit Geräte die freien Kataloge (Titel, Rechte,
später Cover) von ihrem eigenen Server lesen, statt jede Quelle selbst zu
fragen. Die Audiodatei kommt immer von der Quelle, nie über diesen Dienst.

**Was es abruft.** Ein zweites Programm, `maer-sync-abruf`, in einem eigenen
Container, fragt sieben Quellen: archive.org für LibriVox, seine Hörbücher,
die Netlabels, die Themenregale der 78-rpm-Sammlung und Old Time Radio,
Wikimedia Commons für die Aufnahmen von Musopen, und Radio Browser für seine
Sender. Eine Quelle nach der anderen, höchstens eine Anfrage pro Sekunde, als
`Maer/1.0 (+https://maer.fm)`. Jede Quelle bekommt einmal pro Woche einen
vollen Durchlauf und täglich einen Blick auf das, was hinzugekommen ist;
Musopen und Radio Browser lassen sich danach nicht fragen, also bekommen sie
jeden Tag einen vollen Durchlauf. Gemessen am 25.11.2026 für die fünf
archive.org-Quellen: etwa 35 Anfragen und 20 MB am Tag, und einmal pro Woche
ein voller Durchlauf von etwa 165 MB. Project Gutenberg kommt von seinem
Spiegel `gutenberg.pglaf.org`: einmal pro Woche der Katalog-Dump (etwa 21 MB)
und der Datensatz jedes Werks vom Typ Sound (etwa 1100, je ein paar KB, einer
pro Sekunde); ein Werk, dessen Datensatz ein Urheberrecht angibt, bleibt
draußen. Die Hörspiel-Feeds folgen, sobald ihr Leser gebaut ist.

**Die Beliebtheitsstufe.** Ein voller Durchlauf fragt jede Quelle auch nach
ihrer Zahl (archive.org `downloads`, auch im täglichen Blick, wo sie
ungenutzt bleibt; Radio Browser `votes`; die Downloads aus dem
Gutenberg-Datensatz) und gibt jeder Zeile das Zehntel ihrer Quelle,
`beliebt_stufe` 1 bis 10; Musopen nennt keine Zahl. Nur ein voller Durchlauf
setzt sie, also ändert sich eine Zeile nur, wenn sich ihr Zehntel ändert. Die
Zeile eines Laufs nennt `stufen_gewechselt=N`, die Zeilen, deren Stufe sich
bewegt hat.

**Was es speichert.** Nur Datensätze (Titel, Autor, Jahr, Rechte, Regale) in
einer Datei, `katalog.db`, auf einem eigenen Volume; kein Audio, kein Konto,
keine Nutzerdaten. Das Abrufprogramm hat keinen Port und sieht den
Nutzerspeicher nie; der Dienst bindet den Katalog nur lesend ein und hat kein
Schreibrecht darauf. Ohne die Datei, oder wenn sie unlesbar ist, antworten
die Katalog-Pfade mit 404 oder 503, und alles andere funktioniert wie zuvor.

**Einschalten:**

    MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db \
      docker compose --profile katalog up -d

Setz `MAER_SYNC_KATALOG` in der Umgebung von Compose (oder in `.env`), damit
der Dienst die Datei liest, und starte das Profil, damit das Abrufprogramm
läuft. Sein Log hat eine Zeile pro Lauf.

**Wenn eine Quelle schrumpft.** Der Katalog weist einen vollen Durchlauf ab,
der eine Quelle leeren würde oder mehr als ein Fünftel ihrer lebenden Zeilen
löschen würde, sobald sie 50 oder mehr hat; nichts wird geschrieben, und der
Lauf wird später erneut versucht. Seine Zeile lautet dann
`ausgang=abgewiesen grund=Gesperrt(...)`, mit den gelesenen Zeilen
(`gelesen`), den lebenden Zeilen, die der Katalog hält (`lebend`), und dem
Befehl, der ihn bestätigt. Ist die Quelle wirklich geschrumpft, bestätige
diesen einen Lauf:

    docker exec maer-sync-abruf /maer-sync-abruf --bestaetigt <quelle>

Er fährt einen vollen Durchlauf dieser Quelle, schreibt ihn, gibt seine Zeile
mit `bestaetigt=ja` aus und endet mit 0 (1, wenn der Lauf scheiterte). Die
Bestätigung gilt nur für diesen Lauf; der nächste große Verlust wird wieder
abgewiesen. Steckt das Programm gerade in einem eigenen Lauf, endet der
Befehl sofort mit 1 und `grund=Belegt`, nichts abgerufen oder geschrieben;
führ ihn erneut aus, sobald der laufende Lauf beendet ist.
Dass nur ein Lauf zur Zeit läuft, hält die Datei `katalog.db.lauf` neben dem Katalog fest.

**Wenn die Seiten einer Quelle nicht enden.** Ein Durchlauf liest höchstens
die Seiten, die seine Zählung verlangt, und zwei weitere, dazu die eine, die
abgewiesen wird. Eine Quelle, die darüber hinaus antwortet oder einen Cursor
ein zweites Mal nennt, wird mit `grund=Seiten(...)` abgewiesen; nichts wird
geschrieben, und der Lauf wird später erneut versucht.

**Ausschalten:** `docker compose --profile katalog stop maer-sync-abruf`,
`MAER_SYNC_KATALOG` entfernen und `docker compose up -d`; die Katalog-Pfade
antworten wieder mit 404. Das Volume `maer-sync-katalog` kann entfernt
werden; es wird beim nächsten Lauf neu aufgebaut.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
