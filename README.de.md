# Mär Sync Service

Ein kleiner, selbst gehosteter Dienst für die Daten der Audio-App [Mär](https://maer.fm): Hörstände, Hörstatistik, Mär-Wiedergabelisten, deine Entscheidungen in der App und welches Gerät gerade spielt.

**Freiwillig, nie nötig.** Mär läuft vollständig ohne ihn. Er steht neben iCloud als zweiter Weg, und du wählst einen davon; er ersetzt iCloud nicht und ist keine Voreinstellung. Er ist für alle, die diese Daten auf einem eigenen Wirt haben wollen oder Geräte ohne iCloud nutzen.

> „Mär“ ist ein altes deutsches Wort für Erzählung oder Geschichte. Der Dienst ist ein einziges, statisch gelinktes Rust-Programm mit einer SQLite-Datei, betrieben hinter einem Proxy, der das Zertifikat hält, oder in einem Container.

> **Hinweis:** Dies ist ein privates Hobby-Projekt ohne Gewähr.

**Zuhause:** <https://maer.fm> · **Quelltext:** <https://github.com/maer-lzs/maer-sync> · **App:** <https://github.com/maer-lzs/maer>

## Was er speichert

Eine SQLite-Datei hält alles, was der Dienst weiß, je Konto in sechs Arten von Dokumenten:

- `stands`: Hörstände
- `stats`: Hörstatistik
- `playlists`: Mär-Wiedergabelisten
- `choices`: deine Entscheidungen in der App
- `claim`: welches Gerät gerade spielt, damit das andere aufhört
- `feedauth`: Zugangsdaten zu Feeds, auf dem Gerät versiegelt mit einem Schlüssel, der das Gerät nie verlässt

Der Dienst schaut in ein Dokument nicht weiter hinein als bis zu seiner Größe und seiner Form; er bewahrt es auf und gibt es zurück. Jedes angemeldete Gerät hält über einen Socket Schritt, der ihm sagt, wenn sich etwas geändert hat. Audio geht nie durch den Dienst.

## Was wohin reist

- **Zwischen deinen Geräten und deinem Dienst, sonst nichts.** Der Dienst stellt selbst keine Anfragen an irgendwen. Er antwortet deinen Geräten und sonst niemandem.
- **Nur verschlüsselt.** Hinter einem TLS-Proxy, wie unten beschrieben. Erreicht ein Gerät den Dienst über einfaches `http`, geht jede Anfrage in einer verschlüsselten Sitzung (Noise `NK`), gebunden an den Schlüssel deines Dienstes, den jedes Gerät beim ersten Kontakt festhält.
- **Kein Passwort auf dem Dienst.** Seit Vertragsfassung 6 leitet ein Gerät aus dem Passwort einen Beweis ab und meldet sich damit an; der Dienst speichert diesen Beweis noch einmal gestreckt, nie das Passwort.
- **Was der Dienst außer den Dokumenten festhält:** je Gerät, in welcher Form es sich zuletzt angemeldet hat (`maer-sync devices <owner>` zeigt die Formen, nie einen Wert), und je Anfrage eine Protokollzeile mit Absenderadresse, Methode, Pfad und Status.
- **Der freiwillige Katalog-Index** ist ein zweites Programm, das öffentliche Kataloge fragt (archive.org, Wikimedia Commons für Musopen, Radio Browser und einen Spiegel von Project Gutenberg), höchstens eine Anfrage pro Sekunde an jede Quelle, als `Maer/1.0 (+https://maer.fm)`. Es ist aus, bis du es einschaltest, und sieht die Nutzerdaten nie.

Aussage für Aussage, mit den Pfaden und dem Vertrag: [docs/design.de.md](docs/design.de.md) und [docs/accounts.de.md](docs/accounts.de.md).

## Voraussetzungen

- Ein Linux-Wirt mit **Docker** (mit Compose) und git, x86_64 oder arm64; oder, für den Weg ohne Container, ein Rechner mit Docker zum Bauen des Programms und ein beliebiger Linux-Wirt zum Betreiben
- Ein **Name**, der auf den Wirt zeigt, und ein **Proxy mit TLS** davor (Caddy oder nginx; Beispiele im Schnellstart)
- **Mär** mit einer Einstellung für den Sync-Dienst, die Vertragsfassung 6 spricht
- Sonst nichts auf dem Wirt: kein Interpreter, keine Systembibliothek, kein Paketmanager im Abbild

## Schnellstart

Gefahren am 28.11.2026 aus einem frischen Klon bei `v0.4.0` auf einem Debian-Wirt; der ganze Lauf mit seiner Ausgabe steht in [docs/container.de.md](docs/container.de.md).

1. Holen und starten:

       git clone https://github.com/maer-lzs/maer-sync.git
       cd maer-sync
       git checkout v0.4.0
       docker compose up -d --build
       docker compose exec maer-sync /maer-sync health

       healthy: 127.0.0.1:8099

2. Bewahre eine Kopie des Schlüssels deines Dienstes auf, weg vom Wirt. Jedes Gerät hält ihn fest; geht er verloren, muss jedes Gerät dem Dienst neu vertrauen:

       docker compose cp maer-sync:/var/lib/maer-sync/identity.key ./identity.key

3. Lege ein Konto an. Das Passwort kommt über stdin, nie über die Befehlszeile, und öffnet nur die erste Anmeldung:

       printf '%s' "$SECRET" | docker compose exec -T maer-sync /maer-sync account someone

       account someone laid down

4. Stell einen Proxy mit TLS davor, der den WebSocket-Aufstieg durchreicht, und prüf ihn (dieser Schritt war nicht Teil des Laufs vom 28.11.2026: die Zeilen für den Proxy sind aus den Anforderungen des Dienstes geschrieben, und die `403` wurde am veröffentlichten Port ohne Proxy gemessen): ohne Token antwortet der Aufstieg mit `403`; ein `404` heißt, dass der Proxy ihn verschluckt hat. Die Zeilen für Caddy und nginx: [docs/container.de.md](docs/container.de.md).

Ist `8099` auf deinem Wirt belegt, starte mit `MAER_SYNC_HOST_PORT=18099` vor `docker compose up`; der Dienst drinnen bleibt auf `8099`.

Ohne Docker, Schritt für Schritt, mit Unit-Datei und einer Sicherung, die sich selbst prüft: [docs/setup.de.md](docs/setup.de.md).

## In der App anmelden

- In Mär: Einstellungen, Abschnitt Mär-Sync-Dienst
- Adresse mit Schema, `https://sync.example.com`, der Besitzer (`someone`) und das Passwort aus Schritt 3
- Die App verlangt sofort ein neues Passwort, mindestens zwölf Zeichen; ab dann hält der Dienst nur noch dessen Beweis

Was ein Gerät bei der Anmeldung tut und wie ein vergessenes Passwort zurückkommt: [docs/accounts.de.md](docs/accounts.de.md).

## Sicherungen und Aktualisieren

- `maer-sync backup <datei>` kopiert den Speicher, öffnet die Kopie wie eine Wiederherstellung und endet nur dann mit 0, wenn jedes Dokument zurückkam. Ein Speicher ohne Inhalt meldet `not_measured` und endet mit 1, mit Absicht.
- Die Schlüsseldatei ist mit Absicht nicht in dieser Sicherung; kopier sie getrennt.
- Bewahre vor einem Update das Abbild oder Programm, das gerade dient, unter einem eigenen Namen auf. Ein älterer Tag lässt sich vielleicht nicht mehr bauen (gemessen: `v0.3.0` baut nicht mehr, weil sein Baupaket aus Alpine verschwunden ist), also ist das aufbewahrte Abbild der Rückweg.

Der ganze Ablauf, und was der Rückweg kostet: [docs/operation.de.md](docs/operation.de.md).

## Katalog-Index (optional)

Der Dienst kann einen Index freier Kataloge tragen, damit die Geräte sie von deinem Wirt lesen, statt jede Quelle selbst zu fragen. Aus, solange du nicht `MAER_SYNC_KATALOG` setzt und das Profil des Abrufprogramms startest:

    MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db \
      docker compose --profile katalog up -d

Was es fragt, was es speichert und wie du es wieder abschaltest: [docs/catalog.de.md](docs/catalog.de.md).

## Weitere Dokumentation

- [Im Container betreiben](docs/container.de.md), der Schnellstart so, wie er gelaufen ist, und was das Abbild entscheidet
- [Von Hand einrichten](docs/setup.de.md), und die Umgebung, die der Dienst liest
- [Einen laufenden Wirt betreiben](docs/operation.de.md): Updates, der Rückweg, die Identität des Dienstes
- [Konten und Anmeldung](docs/accounts.de.md)
- [Der Katalog-Index](docs/catalog.de.md)
- [Wie es gebaut ist](docs/design.de.md): der Kern, die Schale und was er nicht verstehen wird
- [Tests und Messungen](docs/testing.de.md)
- [Der Vertrag](CONTRACT.md), dieselbe Datei wie in der App (englisch)

Alle Seiten auch auf Englisch: [README.md](README.md).

## Lizenz

GPL-3.0-or-later, siehe [LICENSE](LICENSE). Zusätzlich gelten Bedingungen nach GPL-3.0 Abschnitt 7, siehe [LICENSE-ADDITIONAL-TERMS](LICENSE-ADDITIONAL-TERMS). Das Abrufprogramm trägt die Lizenztexte der Teile, aus denen es gebaut ist, aufgeführt in [abruf/FREMDTEILE.md](abruf/FREMDTEILE.md).

## Über dieses Projekt

Der Mär-Sync-Dienst ist mit Unterstützung von KI entstanden: Anthropic Claude (Opus, Sonnet und Fable), dazu lokal betriebene Modelle mit offenen Gewichten (qwen3, devstral und OpenAIs offene gpt-oss-Gewichte) über Ollama für Werkzeuge.
