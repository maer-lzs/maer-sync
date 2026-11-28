# Einen laufenden Wirt betreiben

Updates und der Rückweg, welcher Build läuft, die Identität des Dienstes und die Sitzung. Zurück zur [README](../README.de.md).

## Einen laufenden Wirt aktualisieren, und der Rückweg

Das ist der eine Ablauf, bei dem ein Fehler echte Daten trifft. Der gemessene Wirt
ist ihn am 2026-09-20 gegangen, vom Python-Build zum Rust-Build; dieselben
Schritte tragen jedes Update seitdem.

**Bevor du irgendetwas stoppst.** Bau das Produkt ([Schritte 2 und 3 von „Von Hand einrichten“](setup.de.md)), kopier es auf den
Wirt und probe gegen eine **Kopie** des laufenden Speichers, nie gegen den laufenden
selbst. Die Sicherung aus Schritt 9, gemacht mit der Binärdatei, die gerade bedient, ist diese Kopie.
Dann lass den neuen Build auf einem freien Port gegen die Kopie laufen und lies die Dokumente
zurück. Hebt der neue Build das Schema des Speichers an, tut er das an dieser Kopie, und
es ist der Lauf, der das zeigt, nicht das Lesen.

Probe noch eine Sache, die keine Checkliste nennt: **ob bestehende Tokens
noch gehen.** Leg dem neuen Build auf der Kopie ein Token vor, das der laufende Dienst ausgestellt hat.
Antwortet er 200, muss sich kein Gerät neu anmelden.

**Der Wechsel selbst.**

1. Stopp den Dienst und **miss**, dass der Port frei ist; nimm es nicht an.
2. Sichere den Speicher, solange er gestoppt ist, mit der Binärdatei, die bedient hat,
   und prüf, dass die Sicherung `fits` sagt.
3. Leg die neue Binärdatei an ihren Platz und heb die vorherige daneben auf, unter ihrem eigenen
   Namen.
4. Lass die Schlüsseldatei, wo sie ist. Eine neue Binärdatei ist keine neue Identität.
5. `systemctl daemon-reload`, falls sich die Unit geändert hat, starten und
   `is-active` prüfen.
6. Beweis es mit `health` **und** `/v2/key`, lokal und durch den Proxy.
7. Zähl die Dokumente mit einer Sicherung und vergleich mit Schritt 2.

**Der Rückweg** ist die vorherige Binärdatei und die Sicherung aus Schritt 2: stoppen, die
vorherige Binärdatei zurücklegen, die Sicherung über den Speicher zurückspielen, starten, beweisen.
**Alles, was nach dieser Sicherung geschrieben wurde, ist verloren**, und das ist der Preis des
Rückwegs, kein Unfall dabei. Ein neuerer Build kann das Schema angehoben haben,
und ein älterer Build weist einen Speicher ab, den ein neuerer geschrieben hat; er sagt das
und bedient nicht. Also geht die Sicherung zurück, nicht die Datei, die der neuere Build
angefasst hat.

**Der Umstieg auf Version 6 hat zwei Bedingungen, die keine Probe auf dem Wirt
zeigt.** Version 6 hebt das Schema des Speichers auf 3, also weist ein älterer Build den
Speicher danach ab, und der Rückweg ist die Sicherung. Und `/v1/key` antwortet in Version 6
mit `404`, was eine App der Version 5 als „der Dienst ist älter als
ich“ liest, und sie hört auf abzugleichen, bevor sie ein Passwort sendet. **Jedes Gerät jedes
Kontos spricht Version 6, bevor der Dienst es tut.** Nach dem Wechsel sagt
`maer-sync devices <owner>` je Gerät, wie es sich zuletzt angemeldet hat.

**Die Grenze dieses Rückwegs, laut gesagt:** er trägt, solange die
Clients noch sprechen, was der vorherige Build beantwortet. Sobald sich ein Client auf
etwas verlässt, das nur der neuere Vertrag hat, kann der vorherige Build es nicht beantworten,
und der Rückweg wird zur Einbahnstraße. Das ist kein Argument gegen die
Reihenfolge der Auslieferung, es ist ihr Grund: erst der Dienst, dann die Clients.

## Wissen, welcher Build läuft

Die Binärdatei trägt ihre Version noch nicht: `Cargo.toml` sagt `0.0.0`, und der
Dienst gibt beim Start keine Version aus. Bis das behoben ist, sind die ehrlichen
Griffe indirekt:

    systemctl show maer-sync.service -p ExecStart --value
    sha256sum /srv/maer-sync/maer-sync

und, für die Frage, die in der Praxis wirklich zählt, ob der laufende
Build das Anfrage-Log mitbringt:

    grep -c 'access from=' /srv/maer-sync/maer-sync

Ein Build ab `v0.2.0` enthält diese Zeichenkette, ein älterer nicht.
Das ist ein Behelf, keine Funktion, und er steht hier, weil die
Alternative, `/v1/health` zu erweitern, vom Vertrag verboten ist: dieser Weg
antwortet mit einem Status und mit nichts sonst.

## Betrieb

**Nur hinter TLS.** Das Token reist in einem Header, und über reines http ist ein
Header für alles lesbar, was zwischen dem Gerät und dem Dienst sitzt. Der
Dienst selbst bindet an `127.0.0.1`, und ein Proxy mit einem Zertifikat steht
davor; dieser Proxy muss das WebSocket-Upgrade auf
`/v1/changes` durchreichen.

    MAER_SYNC_DB=maer-sync.sqlite       # the store
    MAER_SYNC_KEY=<beside the store>    # the static key file, default identity.key
    MAER_SYNC_BIND=127.0.0.1:8081       # one address, host and port together
    MAER_SYNC_MAX_BODY_BYTES=67108864   # host protection, 413 above it
    MAER_SYNC_PLAIN_WAYS=on             # off: the five older ways answer only inside a session
    MAER_SYNC_KATALOG=                  # empty: no catalog; a path: read this catalog file

### Die Identität dieses Dienstes

**`/v2/key` und `/v1/session` tragen kein Token, und das ist das Muster,
kein Versäumnis.** `/v2/key` gibt die öffentliche Hälfte heraus, damit ein Gerät sie
festhalten kann, und `/v1/session` tauscht einen Noise-Handshake gegen eine verschlüsselte Sitzung. Wer anfragt, beweist INNERHALB der Sitzung, wer er ist,
und genau das lässt eine Anmeldung schützen: eine Anmeldung trägt das Geheimnis des
Kontos und geschieht, bevor es irgendein Token gibt, mit dem sich etwas beweisen ließe.

**`/v2/key` ist der Kanal für Vertrauen beim ersten Kontakt, und über eine reine Verbindung
kann jeder, der dazwischensitzt, mit seinem eigenen Schlüssel antworten.** Das ist der
gewählte Übergang: beim ersten Mal vertrauen, danach festhalten. Das Festhalten
geschieht auf dem Gerät, und es macht aus einem späteren Austausch ein Ereignis.

Beim ersten Start macht sich der Dienst ein statisches Schlüsselpaar und gibt den
Fingerabdruck aus. Von da an ist dieser Fingerabdruck, wer er ist, und Geräte halten
ihn fest.

    $ maer-sync key
    key file: /var/lib/maer-sync/identity.key
    fingerprint: <fingerprint>
    this store expects: <fingerprint>
    the key <fingerprint> is the one this store expects

(im Container ohne den Fingerabdruck.)

**Die private Hälfte ist eine Datei neben dem Speicher, und mit Absicht nicht in der
Datenbank.** Eine Kopie der Datenbank reist dorthin, wohin Sicherungen reisen, und ein
privater Schlüssel darin ließe jeden, der eine alte Kopie hält, als dieser
Dienst jedem Gerät antworten, das ihn je festgehalten hat. Der Speicher merkt sich nur den
Fingerabdruck, der benennt, ohne zu beweisen, und gefahrlos in einer Sicherung liegen darf.

**Also gehört die Schlüsseldatei getrennt in deine Sicherung**, und ein Zurückspielen
sagt dann von selbst das Richtige. Auf dieselbe Maschine zurückgespielt, ist der Schlüssel
noch da, und nichts wird gefragt. Auf eine neue zurückgespielt, ist der Schlüssel nicht
da, und das ist jetzt ein anderer Dienst.

**Ein Schlüssel, der sich geändert hat oder fehlt, stoppt den Dienst, statt einen
neuen wachsen zu lassen.** Die Abweisung nennt den Fingerabdruck, den sie erwartet hat, den,
den sie gefunden hat, und den Ausweg. Der Dienst endet mit 3 und bindet keinen Port.

Zwei Befehle sagen, dass eine Änderung gewollt ist, und nichts anderes im Dienst
ändert je eine Identität:

    maer-sync key --adopt   # the key here is the right one now
    maer-sync key --new     # make a new identity

`--adopt` ist dafür, einen Dienst auf eine Maschine umzuziehen, die ihren eigenen Schlüssel behält.
`--new` ist ein bewusster Wechsel: **jedes Gerät, das den alten Schlüssel festgehalten hat,
weist diesen Dienst ab, bis jemand den neuen annimmt.** Er bewahrt die
vorherige Schlüsseldatei neben der neuen als `identity.key.was-<date>-<time>` auf,
weil ein versehentlicher Wechsel nicht unumkehrbar sein darf. **Jeder
Wechsel behält seine eigene Datei**, also schreibt der zweite nicht über das, was
der erste beiseitegelegt hat; der Befehl gibt den Namen aus, den er benutzt hat.

### Innerhalb einer Sitzung

Sobald ein Gerät den Schlüssel hält, öffnet es einen Handshake auf `/v1/session` und sendet
von da an jede gewöhnliche Anfrage VERSIEGELT: `POST /v1/session` mit dem
Namen der Sitzung in `X-Maer-Session` und einem versiegelten Rahmen als Body. Der Rahmen
ist die gewöhnliche Leitung, ganz (Methode, Ziel, Header, Body), der Dienst schickt
ihn durch dieselben Türen, durch die eine reine Anfrage geht, und die Antwort kommt
versiegelt zurück. Niemand zwischen dem Gerät und diesem Prozess liest das Token, den
Pfad oder das Dokument. Die genaue Form steht in `CONTRACT.md`.

**Die fünf älteren Wege antworten weiter im Klartext, bis du sie abschaltest.**
Ein Gerät, das nicht aktualisiert wurde, spricht im Klartext, und es muss weiter abgleichen,
bis es aktualisiert ist. Wenn jedes Gerät innen spricht, setz `MAER_SYNC_PLAIN_WAYS=off`:
eine reine Anfrage bekommt dann ein `426` mit einem Satz, und `/v1/health`,
`/v2/key` und `/v1/session` antworten weiter. Die Suite fährt beide Einstellungen;
**welche deine Geräte brauchen, wird an ihnen gemessen, nicht hier.**

**Ein Neustart beendet jede Sitzung, mit Absicht.** Die Schlüssel leben im Arbeitsspeicher und
nirgends sonst, also wäre eine Sitzung, die einen Neustart überlebt, eine, deren Schlüssel
auf der Platte lagen. Das Gerät beantwortet ein `410` mit einem neuen Handshake und einem
neuen Versuch, und niemand sieht einen Satz.

**Ein Socket, der nicht öffnen darf, wird vor dem Upgrade abgewiesen, mit einem `403` und
einem leeren Body**, was auch immer der Grund ist: ein falscher Pfad, kein Token, ein unbekanntes
Token oder ein reiner Socket, während die reinen Wege aus sind. Der Grund geht nicht verloren:
die Log-Zeile nennt ihn, mit dem Close-Code, für den er steht (zum Beispiel 4401
für ein fehlendes Token, 4426 für einen reinen Socket).
Ein Client liest das `403` als „nicht berechtigt“, genauso wie ein REST-401, nie als
„nicht erreichbar“; seine REST-Aufrufe tragen dasselbe Token und antworten zuerst, also wird
der Stand der Berechtigung dort gefahren, und der Socket ist nur ein still
wieder geöffneter Beschleuniger.

**Jede Abweisung hinterlässt eine Zeile** im Log (`maer.sync`): die Quelladresse,
den Besitzer, das Dokument, den Status und den Satz des Tors; eine fehlgeschlagene Anmeldung
hinterlässt die Quelladresse und den Besitzer. Nie ein Token, nie ein Geheimnis, nie
einen Body, und ein Fall in der Suite liest das Log, um das zu halten. Ohne diese
Zeilen wäre der Fall, für den der Dienst gebaut ist, ein Gerät, das beim Start leer
schreibt, und ein Client, der das 409 verschluckt, unsichtbar, solange er
dauert.

**Die Grenze für den Body ist Schutz für den Wirt, keine Obergrenze für Daten.** Ein Dokument,
größer als `MAER_SYNC_MAX_BODY_BYTES`, wird mit 413 abgewiesen, bevor es
im Arbeitsspeicher gehalten wird. Sie ist mit Absicht großzügig und wird je Auslieferung gesetzt; das
Schrumpf-Tor schützt die Daten, dies schützt die Maschine.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
