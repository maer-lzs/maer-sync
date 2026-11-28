# Im Container betreiben

Der Schnellstart mit Docker, so wie er gelaufen ist, und was das Abbild und die Compose-Datei entscheiden. Zurück zur [README](../README.de.md).

## Schnellstart mit Docker

**Gelaufen am 28.11.2026 auf einem Debian-Wirt (x86_64, Docker 29.9.3, Compose
5.7.1)**, aus einem frischen Klon von `github.com/maer-lzs/maer-sync` beim Tag
`v0.4.0`, mit eigenem Projektnamen und Port neben einem laufenden Dienst
(`COMPOSE_PROJECT_NAME=maer-sync-probe`, `MAER_SYNC_HOST_PORT=18199`). Die
Ausgabe unter jedem Befehl ist die, die zurückkam; wo der veröffentlichte
Port erscheint, ist es dieser Port. **In dieser Probe nicht gelaufen: der
Proxy und die App (Schritte 4 und 6).** Die Proxy-Zeilen sind aus den
Anforderungen des Dienstes geschrieben, Schritt 5 lief ohne Proxy gegen den
veröffentlichten Port, und die Anmeldung braucht ein Gerät mit der App. Der
frühere Lauf der Schritte 1 bis 3 am 08.11.2026 gab dieselben Antworten. `v0.4.1` ändert am Dienst nichts,
deshalb nennen die Befehle unten ihn.

Du brauchst einen Linux-Wirt mit Docker (mit Compose) und git, und einen
Namen, der auf ihn zeigt, hier `sync.example.com`. Sechs Schritte von nichts
bis zu einem angemeldeten Gerät:

**1. Holen und starten.** Der erste Start baut das Abbild und braucht das
Netz; Bau und Start dauerten auf dem gemessenen Wirt 68 Sekunden, auf einem
langsameren dauern sie länger.

    git clone https://github.com/maer-lzs/maer-sync.git
    cd maer-sync
    git checkout v0.4.1
    docker compose up -d --build
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

**2. Den Schlüssel notieren und eine Kopie davon behalten.** Der erste Start
erzeugt den Schlüssel dieses Dienstes und sagt das im Log. Jedes Gerät pinnt
ihn beim ersten Kontakt; geht der Schlüssel verloren, muss jedes Gerät dem
Dienst erneut vertrauen.

    docker compose logs --no-log-prefix maer-sync | grep 'answers as'

    this service now answers as TPNW-8BC8-Z8ZH-PRZ6-VEX9-XWZ0

    docker compose cp maer-sync:/var/lib/maer-sync/identity.key ./identity.key

     maer-sync Copying maer-sync:/var/lib/maer-sync/identity.key to ./identity.key
     maer-sync Copied maer-sync:/var/lib/maer-sync/identity.key to ./identity.key

Die Schlüsseldatei ist 45 Bytes groß. Leg `identity.key` an einen sicheren
Ort, fern von diesem Wirt. (Der Name oben gehörte zum Wegwerf-Dienst der
Probe, den es nicht mehr gibt.)

**3. Ein Konto anlegen.** Das Passwort kommt über stdin, nie über die
Befehlszeile. Es öffnet nur die erste Anmeldung: das erste Gerät setzt im
selben Schritt ein neues.

    printf '%s' "$SECRET" | docker compose exec -T maer-sync /maer-sync account someone

    account someone laid down

    docker compose exec maer-sync /maer-sync devices someone

    account someone: form=secret set-by-operator=no moved-at=not yet
    no device has signed in since version 6

Ein Konto, das schon da ist, wird abgewiesen, und der Befehl endet mit 1:

    refused: an account named 'someone' is already here

**4. Einen Proxy mit TLS davorsetzen.** Er muss das WebSocket-Upgrade
durchreichen, und er muss `X-Real-IP` senden. Mit Caddy, das das Zertifikat
selbst holt und das Upgrade von sich aus durchreicht:

    sync.example.com {
        reverse_proxy 127.0.0.1:8099 {
            header_up X-Real-IP {remote_host}
        }
    }

Mit nginx, im `server`-Block, der das Zertifikat für den Namen schon hält:

    location /v1/changes {
        proxy_pass http://127.0.0.1:8099;
        proxy_http_version 1.1;
        proxy_set_header Upgrade    $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 3600s;
    }
    location / {
        proxy_pass http://127.0.0.1:8099;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

**5. Schlüssel und Socket durch den Proxy belegen.** `/v2/key` antwortet
mit der Vertragsversion in einem Header. Ohne Token beantwortet der Dienst
das Upgrade mit `403`: das Upgrade ist durchgekommen. Ein `404` heißt, der
Proxy hat es verworfen, und der Push wird fehlen, während alles andere gut
aussieht. In der Probe gingen beide ohne Proxy an den veröffentlichten Port:

    curl -s -D - -o /dev/null https://sync.example.com/v2/key | grep -i -E '^HTTP|x-maer'

    HTTP/1.1 200 OK
    x-maer-contract: 6

    curl -s -o /dev/null -w '%{http_code}\n' -N \
      -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
      -H 'Sec-WebSocket-Version: 13' \
      -H "Sec-WebSocket-Key: $(head -c16 /dev/urandom | base64)" \
      https://sync.example.com/v1/changes

    403

**6. Die App darauf zeigen lassen.** In Mär: Einstellungen, der Abschnitt
zum Mär-Sync-Dienst. Die Adresse mit ihrem Schema (`https://sync.example.com`),
der Besitzer (`someone`) und das Passwort aus Schritt 3. Die App fragt sofort
nach einem neuen Passwort, mindestens zwölf Zeichen.

**Eine Sicherung eines Speichers, in dem nichts steht, besteht nicht.** Direkt
nach Schritt 3 hat noch kein Gerät etwas geschrieben, und die Sicherung der
Probe sagte genau das und endete mit 1, wie sie soll (siehe [Schritt 9 in „Von Hand einrichten“](setup.de.md)):

    docker compose exec maer-sync /maer-sync backup /var/lib/maer-sync/probe-backup.sqlite

    not_measured: not measured, the backup held no documents

### Den Container aktualisieren, und der Rückweg

Gelaufen in derselben Probe am 28.11.2026. **Behalte das Abbild, das gerade
dient, unter einem eigenen Namen, bevor du das nächste baust**; es ist der
Rückweg.

    docker image tag maer-sync:local maer-sync:previous
    git fetch origin && git checkout <the new tag>
    docker compose up -d --build
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

Wenn sich der Dienst zwischen den beiden Versionen nicht geändert hat (von
`v0.4.0` zum `dev` desselben Tages änderte sich nur das Abrufprogramm),
ergibt der Bau dasselbe Abbild, und Compose lässt den Container laufen
(`Container maer-sync
Running`). Der Rückweg legt das behaltene Abbild unter den Namen, den Compose
startet, auf demselben Volume:

    docker image tag maer-sync:previous maer-sync:local
    docker compose up -d --no-build --force-recreate maer-sync
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

    docker compose exec maer-sync /maer-sync devices someone

    account someone: form=secret set-by-operator=no moved-at=not yet
    no device has signed in since version 6

**Ein älterer Tag baut womöglich nicht mehr, also verlass dich nicht darauf,
ihn neu zu bauen.** Gemessen in derselben Probe: `v0.3.0` pinnt das Baupaket
auf seine Revision (`musl-dev=1.2.5-r11`), Alpine führt inzwischen nur noch
`r12`, und der Bau bricht ab mit `breaks: world[musl-dev=1.2.5-r11]`. Ab
`v0.4.0` hält der Pin die Version und nicht die Revision (siehe *Was die
beiden Dateien entscheiden* unten). Das behaltene Abbild ist der Rückweg; das
Speicherschema entscheidet, ob es den Speicher wieder öffnen darf (siehe den
Abschnitt zum Aktualisieren und zum Rückweg in [Einen laufenden Wirt betreiben](operation.de.md)).

### Der Katalog-Index im Container

Gelaufen in derselben Probe am 28.11.2026 bei `v0.4.0`; der Bau des
Abrufprogramms und der Start beider dauerten 72 Sekunden:

    MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db \
      docker compose --profile katalog up -d --build
    docker compose logs --no-log-prefix maer-sync-abruf

    abruf: started, catalog /var/lib/maer-sync-katalog/katalog.db
    lauf quelle=archive-buch art=voll gelesen=0 version=0->0 dauer_ms=604 ausgang=abgewiesen grund=Echo { ... }

Die Zeile ist hier gekürzt. Ihre Abweisung ist ein Fehler von `v0.4.0`:
archive.org gibt seine Anfrage in anderer Schreibweise zurück, und `v0.4.0`
wies deshalb vier Quellen ab. Die Korrektur kam mit `v0.4.1`; auf dem Wirt, der diesen Dienst
trägt (unter systemd, nicht im Container), liefen danach am 28.11.2026 alle
vier Quellen ohne Abweisung. Was der Katalog tut und wie du ihn wieder
abschaltest: [Der Katalog-Index (optional)](catalog.de.md).

**Aufräumen nach einer Probe.** `docker compose --profile katalog down -v`
entfernt die Container, beide Volumes und das Netz; die Abbilder bleiben bis
`docker image rm maer-sync:local maer-sync-abruf:local`. Gezählt nach der
Probe: kein Container, kein Volume und kein Netz der Probe übrig, und kein
Abbild davon.

## Mehr zum Container

Die Schritte stehen oben im [Schnellstart mit Docker](#schnellstart-mit-docker). Dieser Abschnitt sagt, was das
Abbild und die Compose-Datei entscheiden, und warum.

**Alles in diesem Abschnitt wurde am 24.09.2026 auf einem Debian-Wirt gebaut
und betrieben**, gegen eine Kopie eines laufenden Speichers mit 78
Dokumenten, nicht aus der Dokumentation geschrieben. Wo etwas von einem
sauberen Wirt abweichen musste, steht dabei, was und warum.

Das Abbild hat zwei Stufen. Die erste baut die Binärdatei in derselben
gepinnten Umgebung wie `werkzeug/bau.sh`; die zweite ist leer (`FROM scratch`)
und bekommt die Binärdatei, das Datenverzeichnis, ein leeres `/tmp` und die
beiden Lizenztexte. Keine Shell, keine C-Bibliothek, kein Paketmanager, und
damit nichts unter der Binärdatei, das zu aktualisieren wäre. Gemessen am
24.09.2026: der Dateibaum im Abbild ist exportiert 4867584 Bytes groß, davon
die Binärdatei 4818584 und die beiden Lizenztexte 37718; Docker führt das
gespeicherte Abbild mit 7162711 Bytes.

Ein Container beantwortet mehrere der Schritte aus [Von Hand einrichten](setup.de.md) auf einmal: Schritt 1
war schon keine Frage, und `restart: unless-stopped` **ersetzt Schritt 8
vollständig**: keine Unit-Datei, kein Systembenutzer und keine Rechte, die am
Speicher von Hand zu richten wären. **Schritt 7 ersetzt er nicht.** Der Proxy
davor beendet weiterhin TLS und muss weiterhin das WebSocket-Upgrade
durchreichen, und das bleibt der Teil, der still scheitert.

    docker compose build
    docker compose up -d
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

**Wenn `8099` auf deinem Wirt belegt ist, setz den Port, statt die Datei zu bearbeiten:**

    MAER_SYNC_HOST_PORT=18099 docker compose up -d

Nur der veröffentlichte Port wandert; im Container bleibt der Dienst auf
`8099`, und ebenso alles, worauf der Proxy aus Schritt 7 eingestellt ist.

**Ein neues Volume erzeugt seinen Schlüssel beim ersten Start. Ein
wiederhergestellter Speicher nicht.** Gemessen mit einer Kopie eines
laufenden Speichers ohne Schlüsseldatei daneben: der Container verweigert und
startet in einer Schleife neu (`restarting`, Exit-Code 3, sieben Neustarts in
den ersten zwölf Sekunden, die Gesundheitsprüfung `unhealthy`), und jeder
Versuch schreibt denselben Satz mit dem Ausweg ins Log. Leg die
Schlüsseldatei des alten Wirts in das Volume, und für kein Gerät ändert sich
etwas. Oder erzeuge mit Absicht eine neue Identität, im Wissen, dass jedes
Gerät erneut fragen wird:

    docker compose run --rm maer-sync key --new
    docker compose up -d

### Was die beiden Dateien entscheiden, und warum

**Nichts unter der Binärdatei.** Das frühere Python-Abbild musste ein
Basis-Abbild bis auf den Patch pinnen, weil das Basis-Abbild die Laufzeit
war. Hier gibt es keine: die letzte Stufe ist leer, und die Baustufe ist per
Digest gepinnt und wird nie ausgeliefert.

**Was gepinnt ist und was nicht.** Das Basis-Abbild der Baustufe, mit seiner
Rust-Toolchain, ist per Digest gepinnt. Das eine Paket, das dazukommt,
`musl-dev`, ist auf seine Version gehalten, `1.2.5`, und nicht auf eine
Revision: Alpine behält von einer Version nur die neueste Revision, und ein
Pin auf eine Revision (`1.2.5-r11`) brach am 08.11.2026 jeden Bau, als der
Index zu `1.2.5-r12` weitergezogen war. Eine Revision ist eine Korrektur
derselben Version, also können zwei Wirte an zwei Tagen mit zwei Revisionen
bauen, nie mit zwei Versionen. Verlässt `1.2.5` den Index, bricht der Bau ab,
statt eine andere Version zu nehmen. `tests/abbild.rs` hält beide Dockerfiles
auf derselben Version.

**Der Speicher ist ein benanntes Volume, nie ein Pfad im Abbild.** Das ist
die Zeile, die Daten kostet, wenn sie falsch ist: ein Speicher im Abbild ist
mit dem nächsten Bau weg, und nichts davon ist beim Lesen der Datei zu sehen.
Belegt an der Kopie eines laufenden Speichers statt behauptet: die Sicherung
im Container sagte `fits: 78 documents, 3508517 bytes, came back
unchanged` nach dem ersten Start, wieder nach `docker compose restart` und
wieder nach einem vollen `down` und `up`, und `/v2/key` antwortete jedes Mal
mit demselben Fingerabdruck. Die Kopie war von einem Build der Version 5
geschrieben (Schema 2); der Build der Version 6 öffnete sie, hob sie
unterwegs auf Schema 3 (so sagt es sein Code; die Zahl in der Kopie wurde
nicht gelesen) und lieferte sie aus. Das sind die Zahlen, die die nächtliche
Sicherung des Wirts selbst für genau diese Datei geschrieben hatte.

**Der Benutzer hat eine feste numerische ID, 10001.** Das Volume gehört einer
UID, und diese UID muss Neubauten des Abbilds überstehen, sonst kommt der
Dienst wieder hoch und steht vor seinem eigenen Speicher, ohne das Recht, ihn
zu betreten. `docker top` zeigt beide Prozesse, das Init und den Dienst, als
10001 laufen.

**Innen lauscht der Dienst auf allen Adressen (`MAER_SYNC_BIND=0.0.0.0:8099`),
außen nur auf `127.0.0.1`.** In einem Container erreicht der veröffentlichte
Port den Dienst nur, wenn er über Loopback hinaus lauscht; was von außerhalb
des Wirts erreichbar ist, entscheiden die Port-Zeile und der Proxy aus
Schritt 7, der auch das Zertifikat hält.

**Hinter dem veröffentlichten Port ist die Gegenseite das Gateway des
Container-Netzes, nicht `127.0.0.1`,** also ist der Proxy auf dem Wirt für den
Dienst nicht „sein eigener Wirt“. Gemessen am 24.09.2026: ohne das Gateway in
`MAER_SYNC_TRUSTED_PROXIES` sperrten zehn fehlgeschlagene Anmeldungen von
einer Adresse auch eine zweite Adresse aus, weil beide unter dem Gateway
gezählt wurden. Die Compose-Datei legt ihr Netz deshalb auf ein festes
Subnetz (`10.89.12.0/29`, außerhalb der Bereiche, die Docker von sich aus
vergibt) und vertraut ihrem Gateway (`10.89.12.1`); damit gemessen, wurden
die beiden Adressen getrennt gezählt. Kollidiert dieses Subnetz mit einem
Netz auf deinem Wirt, setz `MAER_SYNC_SUBNET` und `MAER_SYNC_GATEWAY`
zusammen.

**Die Gesundheitsprüfung fragt die Binärdatei selbst.** Das Abbild hat kein
curl und keine Shell, also fragt `/maer-sync health` bei `/v1/health` nach und
endet mit 0 oder 1. Ihre rote Probe ist gelaufen: bei gestopptem Dienst sagte `docker compose run --rm
maer-sync health` `not healthy: nothing answers on 127.0.0.1:8099:
Connection refused (os error 111)` und endete mit 1; wieder gestartet, sagte
sie `healthy` und endete mit 0.

**Vor dem Dienst steht ein Init (`init: true`).** Ohne eines ist der Dienst
PID 1, und PID 1 hat für ein Signal keine Standardaktion: `SIGTERM` bewirkt
nichts, und jedes `stop` und jedes `restart` wartete Dockers zehn Sekunden ab
und endete in `SIGKILL`, Exit-Code 137. Damit gemessen: `stop` dauerte 409 ms
und `restart` 502 ms, Exit-Code 143. Das Init ist das, das Docker mitliefert
(`docker-init`), kein Paket im Abbild. **Wenn du das Abbild ohne Compose
startest, gib `--init` mit.**

**`mem_limit: 256m` ist absichtlich hoch.** Gemessen an der Kopie mit 78
Dokumenten: etwa 8 MB in Gebrauch, eine Spitze von 16,9 MB einschließlich
einer Sicherung. Die Grenze bleibt hoch, weil ein einzelner Schreibvorgang so
groß sein darf wie `MAER_SYNC_MAX_BODY_BYTES` (64 MiB) und im Arbeitsspeicher
gehalten wird, bevor er beurteilt wird. Eine Grenze nahe am gemessenen
Gebrauch würde den Dienst beim ersten großen Schreibvorgang anhalten, der
völlig berechtigt ist.

### Die Suite und das Abbild

Das Abbild enthält keine Suite und nichts, womit sich eine ausführen ließe.
Die Suite läuft im Bau-Container ([Schritt 3 in „Von Hand einrichten“](setup.de.md)), gegen dieselben
Quellen und dieselben Pins, und `tests/abbild.rs` hält die Pins in beiden
Dockerfiles und in `bau.sh` gleich, sodass das Abbild nicht still mit einer
anderen Toolchain bauen kann als der, auf der die Suite lief. **Und die
Binärdatei im Abbild ist die Binärdatei, die `bau.sh` baut**, Byte für Byte:
die Baustufe nutzt dieselben Pfade (`CARGO_HOME`, `CARGO_TARGET_DIR`), und am
24.09.2026 kamen beide als dieselben 4818584 Bytes mit demselben `sha256`
heraus. Ein Wirt, der per Kopie installiert, und ein Wirt, der den Container
betreibt, betreiben dasselbe Produkt.

### Warum der Port eine Variable ist und keine Override-Datei

Der erste Versuch, den Port zu verlegen, am 16.09.2026, nutzte eine
`compose.override.yaml`, und der Container startete nicht: **Compose führt
Listeneinträge zusammen, es ersetzt sie nicht**: die Override-Datei fügt einen
*zweiten* Port hinzu, der Container versucht, beide zu binden, und scheitert
an dem, der belegt war. Das ist genau der Port, den du meiden wolltest. In
beiden Varianten gemessen: mit der Override-Datei veröffentlicht
`compose config` zwei Ports, mit der Variable einen. `ports:
!override` würde ersetzen (Compose 2.24 und neuer), braucht aber trotzdem
eine zweite Datei für etwas, das eine Stelle in dieser einen erledigt.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
