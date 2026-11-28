# Wie es gebaut ist

Warum es das gibt, die eine Umsetzung und ihr Vertrag, der Kern, die Schale und was es nicht verstehen wird. Zurück zur [README](../README.de.md).

## Stand

**Stand: in der App seit 3.0.** Die netzlosen Entscheidungseinheiten, ein SQLite-Speicher,
ein HTTP-Dienst mit seinem Changes-Socket, Anmeldung per Bearer-Token,
die verschlüsselte Sitzung, die Konto-Befehle und die Sicherung mit ihrer
Probe, alles unter Test. Die App erreicht ihn über
`MaerSyncCarriers.swift`: eine Sitzung trägt die fünf Dokumentarten
(`stands`, `claim`, `stats`, `playlists`, `choices`) hinter den Nähten der Träger,
also wissen die Abläufe in der App nichts von ihm; die Einstellungsseite
(Einstellungen, Sync) meldet an, prüft die Verbindung und zeigt einen abgewiesenen Schreibvorgang
mit dem eigenen Satz des Dienstes. Er läuft als eine statische Binärdatei auf einem Wirt
dessen, der ihn betreibt, hinter einem Proxy, der das Zertifikat hält. **Seit Vertragsversion 6
sieht der Dienst nie ein Passwort**: das Gerät leitet daraus einen Nachweis ab
und meldet sich damit an, und der Schlüssel, der die Feed-Zugangsdaten versiegelt, verlässt
das Gerät nie (siehe [„Anmelden (Version 6)“ in „Konten und Anmeldung“](accounts.de.md) und `CONTRACT.md`). Die Übernahme von Ende
zu Ende (ein gesperrtes, spielendes Gerät stoppt, wenn das andere startet) braucht zwei
Geräte und wird im Gerätetest gemessen.

## Eine Umsetzung, ein Vertrag

Der Dienst ist in Rust geschrieben und liegt unter `src/`. `CONTRACT.md` neben
dieser Datei ist, was ihn bindet, und die App trägt dieselbe Datei mit demselben
Digest.

**Bis zum 24.09.2026 waren es zwei.** Die erste Umsetzung war Python, und
die Rust-Umsetzung wurde gegen sie geschrieben: die Wegetabelle in `vektoren/wege.json`
wurde gegen beide Seiten gemessen, 28 Fälle, 0 abweichend. Der Python-Build hat
dieses Repository am 24.09.2026 verlassen. Was er gemessen hat, bleibt als Beleg: die Dateien in
`vektoren/` sind auf diesem Stand eingefroren, und die Rust-Suite hält sich weiter Fall für Fall
an sie (siehe `vektoren/README.md`). Der letzte Commit, der den
Python-Build trägt, ist `6415e8c`.

**Der Rückweg ist die vorherige Binärdatei und eine Sicherung des Speichers**, keine
zweite Sprache. Siehe [„Einen laufenden Wirt aktualisieren, und der Rückweg“ in „Einen laufenden Wirt betreiben“](operation.de.md).

## Warum es das gibt

Fünf Arten von Daten reisen zwischen den Geräten einer Person, und nur eine davon
(Podcast-Positionen) ist mit einem Medienserver doppelt. Die anderen vier sind
Märs eigene, und kein Medienserver kennt sie. Es gibt keinen Server, den alle, die Mär
nutzen, haben, und iCloud sperrt Android aus.

Die volle Begründung und die Anforderungen, gegen die das gebaut ist, liegen im
Lieferungen-Repo, `kontext/sync-dienst-anforderungen.md`, **das jemand, der
dieses Repository liest, nicht hat.** Es bindet die Leute, die diesen Dienst geschrieben haben;
es kann niemanden binden, der es nicht öffnen kann, und es hier bindend zu nennen, wäre
ein Versprechen ohne Deckung. **Was jeden bindet, der DIESES Repository liest, ist
`CONTRACT.md` neben dieser Datei, und diese Datei.** Das Anforderungsdokument ist
für den Beleg genannt, nicht als Anweisung.

## Was der Kern entscheidet

Die Einheiten unten sind rein und netzlos, jede mit ihren eigenen Tests. **Hier steht
mit Absicht keine Zahl:** eine Zahl im Fließtext altert mit jeder neuen Einheit, und
die Liste daneben ist das, was stimmt. Es gibt sie als Einheiten und nicht als
Zeilen in Anfrage-Handlern, weil das die Entscheidungen sind, die Leute
ihre Daten kosten, wenn sie falsch sind.

- **`src/admission.rs`**: darf dieser Zustand den gespeicherten ersetzen? Weist
  einen leeren Zustand über einem vollen ab, und einen Einbruch unter eine Untergrenze, außer der
  Client sagt, dass er es so gemeint hat. Der bekannte Verlustfall ist kein kaputtes Update; es
  ist ein Gerät, das beim Start einen leeren Zustand schreibt, bevor es geladen hat.
- **`src/versions.rs`**: welche gespeicherten Versionen einen Schreibvorgang überleben. Die
  letzten N zu behalten, deckt ein kaputtes Update und einen Client, der Unsinn geschrieben hat, mit
  einer Konstruktion ab, und darauf ist „gestern wiederherstellen“ gebaut.
- **`src/restore.rs`**: ist ein geprobtes Zurückspielen wirklich zurückgekommen? Drei
  Ausgänge, und der dritte ist der Punkt: eine Probe, die nicht laufen konnte, ist
  keine bestandene Probe.
- **`src/formats.rs`**: die zwei Versionsnummern, die ein Zurückrollen
  sicher machen, auseinandergehalten: das Dokumentformat (das des Clients) und das Schema des Speichers
  (das des Dienstes).
- **`src/writes.rs`**: die drei Tore in einer Reihenfolge, die eine eigene Entscheidung
  ist: Vorbedingung, dann Format, dann Größe. Ein Client hinter der
  aktuellen Version hat meist einen KLEINEREN Zustand, also würde ihm „zu klein“
  vor „noch einmal lesen“ zu sagen, ihn das Falsche reparieren lassen.
- **`src/authorization.rs`**: wer fragt, und ob er darf. **Die
  Anforderungen stellen diese Einheit von den übrigen ab**, und ihr eigener Kopf sagt,
  warum: ein Fehler hier öffnet die Hörstände anderer Leute, und das ist die
  Art, die man nicht mit einem Update repariert. Drei Entscheidungen leben darin, jede
  ein leichter Fehler: ein fehlendes Konto und ein falsches Geheimnis geben DIESELBE Antwort
  (alles andere macht den Endpunkt zu einer Liste, wer hier ein Konto hat),
  jedes Geheimnis wird in konstanter Zeit verglichen, und der Besitzer kommt aus der
  Antwort des Speichers und nie aus irgendetwas, das der Aufrufer gesagt hat.
- **`src/versuche.rs`**: wie viele fehlgeschlagene Anmeldungen ein Besitzername
  von einer Adresse hatte, und ob die nächste überhaupt beantwortet wird. Unbekannte
  Namen zählen genauso, sonst würde die Grenze sagen, welche Namen es gibt.
- **`src/paths.rs`**: die Form einer Art und eines Scopes, nie die
  Zugehörigkeit. Eine Liste der fünf bekannten Arten würde aus „eine sechste hinzufügen“ ein
  Update des Dienstes machen.
- **`src/identity.rs`**: wer der Dienst IST, in einer Form, die ein Mensch
  vergleichen kann, und was mit dem Schlüssel auf der Platte gegenüber dem geschieht, den der Speicher
  sich merkt.

Jede Einheit antwortet mit einem Urteil, das seinen Satz trägt. Eine Abweisung, die
nicht sagt, warum, wäre genau das Versagen, gegen das die Tore gebaut sind.

## Die Schale

- **`src/store.rs`**: SQLite, eine Zeile je Version. Die Rotation IST die
  Sicherung, und „gestern wiederherstellen“ braucht keine zweite Konstruktion. Das Lesen,
  die Entscheidung und das Einfügen geschehen innerhalb eines `BEGIN IMMEDIATE`, sonst
  wäre das Compare-and-Set eine Geschichte: zwei Geräte mit derselben Version
  bekämen beide gesagt, sie seien aktuell, und das zweite würde das
  erste überschreiben.
- **`src/wege.rs`**: die Wege und der Tunnel durch sie. `GET`/`PUT`
  `/v1/doc/<kind>[?scope=]` mit `ETag`/`If-Match`, die Auflistung, der Token-Tausch
  und `/v1/health`, jeder als Entscheidung ohne I/O.
- **`src/main.rs`**: die Leitung, wo die Entscheidung auf die Verbindung trifft;
  `src/aufstieg.rs`, `src/horcher.rs` und `src/listeners.rs` tragen den
  Changes-Socket unter `/v1/changes`, der jeden angenommenen Schreibvorgang ankündigt.
- **`src/handshake.rs`**, **`src/frames.rs`**, **`src/sessions.rs`** und
  **`src/sitzungsladen.rs`**: die verschlüsselte Sitzung, vom Handshake über
  den Rahmen darin bis zur Regel, welche Sitzung wann endet.
- **`src/befehle.rs`**: die Befehle für den Betrieb (`account`, `account-remove`,
  `backup`, `key`, `health`); **`src/protokoll.rs`**: was man im Betrieb
  im Log sieht; **`src/kennung.rs`**: der Schlüssel als Datei.

### Die Schnittstelle

| | |
|---|---|
| `GET /v1/doc/<kind>` | die Bytes, mit `ETag` und `X-Maer-Format` |
| `PUT /v1/doc/<kind>` | die Bytes; braucht `X-Maer-Format` und, für ein bestehendes Dokument, `If-Match` |
| `412` | du schaust auf eine alte Version: lesen, zusammenführen, wieder schreiben |
| `409` | abgewiesen, und der Body sagt, welches Tor es abgewiesen hat |
| `X-Maer-Declared-Shrink: 1` | „ich wollte es kleiner machen“ |
| `GET /v1/docs/<kind>` | die Scopes, die ein Dokument dieser Art halten, jeder mit seiner Version und seinem Format; nur Namen, nie Bodies |
| `GET /v1/changes` | WebSocket; eine Zeile je angenommenem Schreibvorgang: Art, Scope, Version |

Ein Schreibvorgang, der gar keine Version nennt, ist auch ein `412`. Schweigen ist kein Anspruch,
aktuell zu sein, und das blinde Überschreiben ist genau das, was iCloud nicht
abweisen kann und dies hier schon.

## Was der Dienst NICHT verstehen wird

Er hält Dokumente als Bytes und deutet sie nie. Das Zusammenführen bleibt auf
jedem Weg im Client, kein Transport kann es, gemessen (ein WebDAV-Server
behält bei einer echten Kollision den letzten Schreibvorgang und legt keine Konfliktdatei an), also
ist nichts zu gewinnen, wenn man dem Dienst beibringt, was ein Hörstand ist,
und jedes Stück Bedeutung, das er hielte, würde eine spätere Änderung
teurer machen.

Eine Sache prüft er an einem Schreibvorgang doch, und nur diese eine: **dass sich der Body
als JSON lesen lässt.** Was der Dienst nicht lesen kann, ersetzt nie, was er hält,
weil ein unlesbarer gespeicherter Body das Dokument jedem nimmt, der es liest,
auch dem Schreiber, der ihn geschickt hat. Die Prüfung läuft für **jede Art**,
und es gibt mit Absicht **keine Liste bekannter Arten**: ein neuer Träger in einer
neueren App muss reisen können, ohne dass zuerst ein neuerer Dienst nötig ist. Dieser
Dienst läuft bei seinem Besitzer, der ihn aktualisiert, wann er will, oder nie, also
würde eine Liste hier neue Schreibvorgänge genau bei den Leuten abweisen, die ihren
Dienst am wenigsten anfassen, und für sie sähe das Versagen wie ein Fehler der App aus.

**Die Grenze dieser Wahl, hier benannt, damit niemand sie bei einem Ausfall findet:** sie
hält nur, solange jedes Format dieses Protokolls JSON ist. Heute sind es alle.
Ein komprimiertes oder binäres Format würde von diesem Tor abgewiesen, und dann
muss zuerst der Dienst aktualisiert werden, und das ist genau die Kopplung, die die fehlende Liste
vermeidet. Die billigere Hälfte des Schutzes ist schon blind fürs Format: ein Body,
der kürzer ist als die `Content-Length`, die er angekündigt hat, wird vom Leser
vor dem Tor abgewiesen, und jeder Schreibvorgang der App kündigt eine Länge an. Also ist die
JSON-Prüfung das zweite Netz, sie deckt die Schreibvorgänge, die keine Länge ankündigen,
und ein späteres Format, das kein JSON ist, braucht dieses zweite Netz abhängig vom
Format, das der Schreibvorgang schon nennt, keine Liste von Arten.

Das eine, was er tut und kein anderer Weg kann: **eine Änderung ankündigen**,
statt zu warten, bis er gefragt wird. Die Übernahme muss ein gesperrtes, spielendes
Gerät erreichen, und ein stiller Push tut das messbar nicht.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
