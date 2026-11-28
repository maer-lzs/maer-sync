# Von Hand einrichten

Der Weg ohne Docker, Schritt für Schritt, und die Umgebung, die der Dienst liest. Zurück zur [README](../README.de.md).

## Einrichten

Wo ein Schritt zitiert, was zurückkam, lief er am 24.09.2026 auf einem
Debian-Wirt gegen einen Wegwerf-Speicher, und die Ausgabe ist die, die
zurückkam. Wo ein Schritt nicht so lief, wie er dasteht, sagt er das an Ort
und Stelle.

### 1. Was es braucht

**Nichts auf dem Wirt.** Das Binary ist statisch gegen musl gelinkt und bringt
seine eigene Laufzeit mit: kein Interpreter, keine virtuelle Umgebung, keine
Systembibliothek. Was du brauchst, ist eine Maschine, die es bauen kann, und
diese Maschine braucht Docker: der Build läuft in einem festgenagelten Image,
damit die Toolchain nicht wegdriften kann. `werkzeug/README.md` erklärt,
warum, und was das Festnageln abdeckt. Bau nicht mit einem lokal installierten
cargo, außer du bist bereit zu beantworten, worin sich deine Toolchain
unterscheidet.

Build-Wirt und Ziel müssen zusammenpassen: das Werkzeug weigert sich, quer zu
kompilieren, also baut ein x86_64-Wirt das x86_64-Ziel und ein aarch64-Wirt
sein eigenes.

**Eine Sache in diesem Repository ist Python, und das ist nicht der Dienst.**
Zwei Tests fragen `git`, was das Repository verfolgt (Lizenzköpfe, Hygiene).
git lebt auf dem Wirt und nicht im Build-Image, also sind sie für den Wirt
geschrieben, in Python nur mit der Standardbibliothek. Du brauchst `python3`
und `git`, um sie auszuführen, nie, um den Dienst zu betreiben.

### 2. Holen und das Produkt bauen

    git clone https://github.com/maer-lzs/maer-sync.git
    cd maer-sync
    werkzeug/bau.sh build --release

**Der erste Build braucht das Netz**: er holt die Crates, die `Cargo.lock`
festnagelt, nach `ziel/cargo-home` im Projekt und nicht in dein Home.
Danach hält `--offline` jeden späteren Build genau an das, was geholt wurde:

    werkzeug/bau.sh build --release --offline

Das Ergebnis landet in
`ziel/abbau/<target>-unknown-linux-musl/release/maer-sync`, zum Beispiel
`ziel/abbau/x86_64-unknown-linux-musl/release/maer-sync`, etwa 4,7 MB.

Prüf, was du gebaut hast, denn ein Binary sagt noch nichts über sich selbst
(siehe [Einen laufenden Wirt betreiben, Abschnitt „Welcher Build läuft“](operation.de.md)):

    ls -l ziel/abbau/*/release/maer-sync
    sha256sum ziel/abbau/*/release/maer-sync

### 3. Die Tests laufen lassen, bevor du es irgendwohin trägst

    werkzeug/vor-dem-commit.sh

Das führt `git add -A` aus, dann die Zäune des Repositorys, dann die
Rust-Suite, und hält bei Rot an. `git add -A` kommt mit Absicht zuerst: der
Lizenz-Zaun fragt git, was verfolgt wird, also käme eine unverfolgte Datei
ungesehen durch. Ein grüner Lauf endet mit `frei zum Committen.` Die
Rust-Suite allein ist `werkzeug/bau.sh test --offline`.

**Ein Übersprung ist kein Bestehen, und der Lauf sagt, was was ist.** Ein Fall
wird auf einem einfachen Wirt übersprungen und nennt seinen Grund: er liest
einen Speicher, den der Python-Build angelegt hat, übergeben als
`MAER_SYNC_PYTHON_DB`. Ohne einen solchen enthält er sich, statt zu bestehen.

### 4. Auf den Wirt bringen

Das Binary braucht einen Ort, eine Datenbank und einen Schlüssel. Sonst nichts.

    sudo install -o <serviceuser> -g <serviceuser> -m 755 maer-sync /srv/maer-sync/maer-sync

Der Dienst-Benutzer braucht Schreibzugriff auf das Verzeichnis mit der
Datenbank und dem Schlüssel, und nichts darüber hinaus. **Hier nicht so
gelaufen, wie es dasteht**: der gemessene Wirt hält sein Binary unter einem
eigenen Pfad.

### 5. Der Schlüssel: der erste Start macht ihn, und danach ist er Pflicht

**Bei einem neuen Speicher macht der erste Start den Schlüssel selbst** und
sagt das:

    a static key was made: M2KZ-6KRX-4CTH-KR3K-65ZK-J3EM (every device that connects will pin it)
    this service now answers as M2KZ-6KRX-4CTH-KR3K-65ZK-J3EM
    serving on 127.0.0.1:18231

**Ab dann merkt sich der Speicher ihn**, den Fingerabdruck und nie die private
Hälfte, und eine Schlüsseldatei, die fehlt oder anders ist, hält den Dienst
an. Er beendet sich mit Code **3** und bindet keinen Port, statt sich eine neue
Identität zuzulegen, die jedes Gerät ablehnen würde. Genau darauf läuft eine
Wiederherstellung auf einer neuen Maschine auf; siehe
[Einen laufenden Wirt betreiben, Abschnitt „Die Identität dieses Dienstes“](operation.de.md).

Um den Schlüssel vor dem ersten Start zu machen oder ihn bewusst zu ersetzen:

    sudo -u <serviceuser> env \
      MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite \
      MAER_SYNC_KEY=/srv/maer-sync/data/identity.key \
      /srv/maer-sync/maer-sync key --new

Er gibt den Fingerabdruck aus. **Kopier diese Schlüsseldatei jetzt an einen
sicheren Ort.** Clients nageln den Fingerabdruck beim ersten Kontakt fest, also
heißt ein verlorener Schlüssel, dass jedes Gerät dem Dienst wieder von vorn
vertrauen muss.

### 6. Ein Konto anlegen

    printf '%s' "$SECRET" | sudo -u <serviceuser> env \
      MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite \
      /srv/maer-sync/maer-sync account someone

    account someone laid down

**Das Passwort kommt über stdin und nie über die Befehlszeile**, weil ein
Argument für jeden anderen Prozess auf der Maschine sichtbar ist und in der
Shell-History landet. Der Dienst speichert einen scrypt-Hash, nie das
Passwort.

**Dieses Passwort kennt der Betreiber, also öffnet es nur je einen Wechsel.**
Das Konto beginnt in der Form `secret`; das erste Gerät, das sich anmeldet,
liest das und setzt im selben Schritt ein neues Passwort, und ab dann meldet
nur noch das neue an, als Nachweis, den der Dienst nicht in ein Passwort
zurückverwandeln kann. Ein Passwort, das der Dienst oder der Betreiber einmal
kannte, wird nie zum Schlüssel der Feed-Zugangsdaten.

**Ein Konto, das schon da ist, wird abgelehnt**, und der Befehl endet mit 1:

    refused: an account named '<owner>' is already here

Ein bestehendes Konto wird nicht stillschweigend überschrieben.

Prüf es:

    $ maer-sync devices someone
    account someone: form=secret set-by-operator=no moved-at=not yet
    no device has signed in since version 6

**Eine Anmeldung lässt sich nicht mit curl ausprobieren, und genau darum geht
es.** Die drei Anmeldewege antworten nur innerhalb einer verschlüsselten
Sitzung, egal, was `MAER_SYNC_PLAIN_WAYS` sagt:

    $ curl -s -X POST http://127.0.0.1:18231/v1/token \
        -H 'Content-Type: application/json' -d '{"owner":"someone","secret":"x","device":"d"}'
    {"reason":"the sign-in paths speak only inside a session"}

mit Status `426`. Gelaufen am 24.09.2026 auf dem freien Port `18231`. Die
ganze Anmeldung, vom Handshake über den Wechsel bis zur Versuchsgrenze, fährt
`werkzeug/geheimnisprobe.py` auf einem laufenden Build durch, gegen einen
Wegwerf-Speicher.

### 7. Einen Proxy davorsetzen und den Socket beweisen

**Das ist der Schritt, der leise schiefgeht.** Ein Proxy, der REST weiterreicht,
aber das WebSocket-Upgrade fallen lässt, hinterlässt einen Dienst, der völlig
gesund aussieht: die Anmeldung geht, Dokumente reisen, und nur der Push fehlt,
also kommt eine Änderung dreißig Sekunden zu spät statt sofort. Nichts
protokolliert einen Fehler.

**Gelaufen am 16.09.2026, auf nginx, vor dem Python-Build, der damals hier
stand:** mit den drei Upgrade-Zeilen antwortet der Handshake mit Token `101`
und ohne `403`. Mit auskommentierten Zeilen antwortet REST weiter `200`, und
das Upgrade liefert **`404`**: kein Hängen und kein 502, und genau deshalb
sieht nichts kaputt aus. Die Proxy-Zeilen sind für den Rust-Build dieselben,
und er beantwortet den Handshake genauso (`101` mit Token, `403` mit leerem
Body ohne).

Das Beispiel unten ist aus den Anforderungen des Dienstes selbst geschrieben,
nicht von einer Maschine, die dieser Autor eingerichtet hat; was zählt, ist der
Beweis danach.

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

**`X-Real-IP` ist keine Verzierung.** Raten hat eine Grenze: zehn
fehlgeschlagene Anmeldungen für einen Besitzernamen von einer Adresse
innerhalb von fünfzehn Minuten, und die nächste antwortet `429`. Der Dienst
nimmt die Adresse aus `X-Real-IP` **nur, wenn die Verbindung von einem Proxy
kommt, dem er vertraut**: von Haus aus sein eigener Wirt (`127.0.0.1` und
`::1`), denn dort sitzt der Proxy. Von überall sonst nimmt er die Gegenstelle
und übergeht den Header, also kann sich niemand die Adresse aussuchen, unter
der er gezählt wird. Ohne die Zeile ist jedes Gerät hinter dem Proxy eine
einzige Adresse, und eine Person, die einen Namen rät, sperrt alle aus, die
sich unter ihm anmelden.

**Ein Proxy anderswo muss genannt werden.** `MAER_SYNC_TRUSTED_PROXIES` ist
eine kommagetrennte Liste von Adressen, je eine Maschine und keine Bereiche.
Der Dienst gibt beim Start die Liste aus, der er vertraut
(`trusting X-Real-IP from: …`), und ein Eintrag, den er nicht lesen kann, hält den Start mit
Exit-Code 6 an und nennt den Eintrag, weil eine Liste, die still kürzer wurde,
wieder jedes Gerät als eines zählen würde. Der Container setzt das für dich;
siehe [Im Container betreiben](container.de.md).

**Der Beweis, und lass ihn gegen den PROXY laufen, nicht gegen 127.0.0.1:**

    $ curl -s -o /dev/null -w '%{http_code}\n' -N \
        -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
        -H 'Sec-WebSocket-Version: 13' \
        -H "Sec-WebSocket-Key: $(head -c16 /dev/urandom | base64)" \
        -H "Authorization: Bearer $TOKEN" \
        https://sync.example.com/v1/changes
    101

**101 heißt, das Upgrade ist durchgekommen.** Ein `403` hier heißt „nicht
berechtigt“ und nicht „falsches Netz“; siehe den Vertrag, der diesen
Unterschied ausbuchstabiert, weil man ihn leicht falsch herum liest.

**Nur hinter TLS.** Das Token reist in einem Header, und über einfaches http
ist ein Header für alles lesbar, was zwischen dem Gerät und dem Dienst liegt.

### 8. Es von selbst starten lassen

**Oder überspring diesen Schritt ganz und nimm den Container** (siehe [Im Container betreiben](container.de.md)): `restart: unless-stopped` ist dieser Schritt, und er braucht
keine Unit-Datei, keinen Systembenutzer und keine von Hand gerichteten Rechte.

**Der Speicher und sein Verzeichnis müssen dem Dienst-Benutzer gehören**,
sonst kommt der Dienst hoch und kann nicht schreiben, oder er kommt gar nicht
hoch. **Hier nicht so gelaufen, wie es dasteht**; es ist die Form der Unit,
die der gemessene Wirt fährt, mit ersetzten Pfaden.

    [Unit]
    Description=Maer sync service
    After=network-online.target
    Wants=network-online.target

    [Service]
    Type=simple
    User=<serviceuser>
    Group=<serviceuser>
    WorkingDirectory=/srv/maer-sync
    Environment=MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite
    Environment=MAER_SYNC_KEY=/srv/maer-sync/data/identity.key
    Environment=MAER_SYNC_BIND=127.0.0.1:8099
    ExecStart=/srv/maer-sync/maer-sync serve
    Restart=on-failure
    RestartSec=5
    NoNewPrivileges=true
    PrivateTmp=true
    ProtectSystem=strict
    ReadWritePaths=/srv/maer-sync/data
    ProtectHome=true

    [Install]
    WantedBy=multi-user.target

**`MAER_SYNC_BIND` ist eine Adresse, Wirt und Port zusammen.** Das ist die
Zeile, die eine für den früheren Python-Build geschriebene Unit falsch hat.
Siehe [Umgebung](#umgebung) unten.

### 9. Sichern, und die Sicherung sich selbst beweisen lassen

    sudo -u <serviceuser> env \
      MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite \
      /srv/maer-sync/maer-sync backup /backups/store-$(date +%F).sqlite

    fits: 1 documents, 7 bytes, came back unchanged

Es kopiert den laufenden Speicher über die Backup-API von SQLite, öffnet die
Kopie so, wie es eine Wiederherstellung täte, vergleicht jedes Dokument nach
Größe und endet nur dann mit 0, wenn die Kopie passend zurückkam. Plan es ein
und alarmier auf den Exit-Code: **eine Sicherung, die nie geöffnet wurde, ist
eine Datei, keine Sicherung.**

**Ein Speicher, in dem nichts liegt, sichert nicht als passend.** Er sagt

    not_measured: not measured, the backup held no documents

und endet mit 1, weil eine Probe, die nichts verglichen hat, nichts bewiesen
hat.

**Es kopiert den Speicher und NICHT die Schlüsseldatei des Dienstes**, mit
Absicht: ein privater Schlüssel in einer Sicherung ließe jeden, der eine alte
Kopie hat, als dieser Dienst antworten. Kopier `identity.key` getrennt und
bewahr sie dort auf, wo keine Datenbanksicherung hingeht. Siehe
[Einen laufenden Wirt betreiben, Abschnitt „Die Identität dieses Dienstes“](operation.de.md).

**Lass es mit dem Binary laufen, das gerade dient, nicht mit einem neueren.**
Das Öffnen eines Speichers darf sein Schema anheben, und die Sicherung öffnet
den Speicher. Ein neuerer Build, der den laufenden Speicher vor einem Wechsel
sichert, hat ihn schon verändert.

### 10. Beweisen, dass das Richtige läuft

Nicht, dass irgendetwas antwortet, sondern dass es deines ist:

    $ maer-sync health
    healthy: 127.0.0.1:18231

    $ curl -s -D - http://127.0.0.1:18231/v2/key
    HTTP/1.1 200 OK
    x-maer-contract: 6
    …
    {"fingerprint":"7TVS-52VV-2EKJ-8F3E-HP4A-FHMR","key":"…"}

(`18231` war der freie Port der Läufe, aus denen diese Zeilen stammen.)

`health` fragt `/v1/health` an der Adresse in `MAER_SYNC_BIND` (eine Adresse
wie `0.0.0.0` wird über Loopback gefragt) und endet mit 0, wenn die Antwort
`200` mit `{"ok":true}` ist, sonst mit 1:

    not healthy: nothing answers on 127.0.0.1:18231: Connection refused (os error 111)

Es beweist, dass ein Dienst läuft und ordentlich antwortet, nicht welcher.
`/v2/key` antwortet mit dem Fingerabdruck, den du dir in Schritt 5 notiert
hast, und nennt die Version des Vertrags, die er spricht, und erst der
Vergleich dieser beiden beweist, dass der Dienst der ist, den du angelegt hast,
und kein Fremder auf einem Port, den du für frei gehalten hast.

Den Rest sagt dir das Log. Der Dienst schreibt eine Zeile pro Anfrage, mit dem
inneren Pfad, dem Status und der Dauer:

    2026-09-23T22:23:28.219Z INFO maer.sync access from=127.0.0.1:39738 session=- inner=false method=GET path=/v1/health scope=- status=200 ms=0

Ein Dienst, der unter Last gar nichts protokolliert, ist nicht untätig, er ist
ein Build von vor `v0.2.0`.

### 11. Die App darauf zeigen lassen

In Mär: Einstellungen, der Abschnitt Mär-Sync-Dienst. Drei Felder: die Adresse
mit ihrem Schema (`https://sync.example.com`), der Besitzer und das Passwort
aus Schritt 6. Das erste Gerät fragt sofort nach einem neuen Passwort (Schritt
6 sagt, warum); die Regel dafür, mindestens zwölf Zeichen, lebt in der App,
weil der Dienst nie ein Passwort zu sehen bekommt, das er beurteilen könnte.
Die Anmeldung tauscht einen Nachweis gegen ein Token, das das Gerät in seinem
Schlüsselbund behält.

Der Schalter darüber entscheidet, ob überhaupt etwas reist. Eingerichtet und
abgeschaltet heißt, nichts bewegt sich, und die Seite sagt das.

## Umgebung

Gemessen durch Zählen der Vorkommen im Quelltext, nicht aus dem Gedächtnis.

| Variable | Vorgabe |
|---|---|
| `MAER_SYNC_DB` | `maer-sync.sqlite` |
| `MAER_SYNC_KEY` | `identity.key` neben dem Speicher |
| **`MAER_SYNC_BIND`** | **`127.0.0.1:8081`** |
| `MAER_SYNC_PLAIN_WAYS` | an |
| `MAER_SYNC_MAX_BODY_BYTES` | 67108864 (64 MiB) |
| `MAER_SYNC_TRUSTED_PROXIES` | `127.0.0.1,::1`: wessen `X-Real-IP` geglaubt wird, siehe Schritt 7 |
| `MAER_SYNC_KATALOG` | leer: kein Katalog, siehe „Der Katalog-Index (optional)“ |

**`MAER_SYNC_BIND` ist die Falle.** Der frühere Python-Build las
`MAER_SYNC_HOST` und `MAER_SYNC_PORT`; dieser liest keine von beiden, null
Vorkommen in `src/`. Eine Unit- oder Compose-Datei, die für den alten Build
geschrieben wurde, setzt also zwei Variablen, die nichts liest, der Dienst
fällt auf seine Vorgabe `127.0.0.1:8081` zurück, startet sauber, protokolliert
nichts Ungewöhnliches und antwortet auf einem Port, den niemand fragt. Es gibt
keinen Fehler zu finden, weil aus Sicht des Dienstes nichts schiefging.

`MAER_SYNC_PLAIN_WAYS` verdient noch einen Satz, weil es ebenfalls leise
scheitert. Es liest `off`, `0`, `no` und `false`, ohne Rücksicht auf Groß- und
Kleinschreibung, und **es schneidet keine Leerzeichen ab**. Ein Wert `off `
mit einem Leerzeichen am Ende fällt auf die Vorgabe zurück, und die ist an.
Prüf nicht die Unit-Datei, um zu sehen, ob es gewirkt hat, prüf das Verhalten:
mit abgeschalteten einfachen Wegen antwortet `/v1/doc/...` mit **426** statt
401, und das sogar mit einem gültigen Token, weil die Sperre vor der
Token-Prüfung sitzt.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
