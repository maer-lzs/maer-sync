# Mär Sync Service

A small, self-hosted place for Mär's own data: listening stands, listening
statistics, Mär playlists, the choice family, and the playback claim.

**Optional, never required.** Mär works completely without it. It stands
beside iCloud as a second way, and the user picks; it is not a replacement
and not a default.

**Status: in the app since 3.0.** The netless decision units, a SQLite
store, an HTTP service with its changes socket, bearer-token authentication,
the encrypted session, the account commands and the backup with its
rehearsal, all under test. The app reaches it through
`MaerSyncCarriers.swift`: one session carries the five document kinds
(`stands`, `claim`, `stats`, `playlists`, `choices`) behind the carrier
seams, so the runners in the app know nothing of it; the settings page
(Settings, Sync) signs in, tests the connection and shows a refused write
with the service's own sentence. It runs as one static binary on a host of
the operator's, behind a proxy that holds the certificate. **Since contract
version 6 the service never sees a password**: the device derives a proof from
it and signs in with that, and the key that seals the feed credentials never
leaves the device (see *Signing in* below and `CONTRACT.md`). The takeover end
to end (a locked, playing device stops when the other starts) needs two
devices and is measured in the device test.

## Quick start with Docker

**Steps 1 to 3 were run on a Debian host (x86_64) on 08.11.2026**, from a
fresh clone with its own project name and port, and the output beside each
command is the one that came back. Step 2 was run without `--no-log-prefix`,
which only drops the `maer-sync  | ` in front of the line. **The proxy and the
app (steps 4 to 6) were not run in that probe**; the proxy lines are written
from the service's requirements, and step 5 is how you prove them.

You need a Linux host with Docker (with Compose) and git, and a name that
points at it, here `sync.example.com`. Six steps from nothing to a signed in
device:

**1. Fetch it and start it.** The first start builds the image and needs the
network; it took 67 seconds for build and start on the measured host, and
takes longer on a slower one.

    git clone https://codeberg.org/LzS/maer-sync.git
    cd maer-sync
    docker compose up -d --build
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

**2. Note the key, and keep a copy of it.** The first start makes the key of
this service and says so in the log. Every device pins it on first contact;
losing the key means every device has to trust the service again.

    docker compose logs --no-log-prefix maer-sync | grep 'answers as'

    this service now answers as M2KZ-6KRX-4CTH-KR3K-65ZK-J3EM

    docker compose cp maer-sync:/var/lib/maer-sync/identity.key ./identity.key

     maer-sync Copying maer-sync:/var/lib/maer-sync/identity.key to ./identity.key
     maer-sync Copied maer-sync:/var/lib/maer-sync/identity.key to ./identity.key

The key file is 45 bytes. Put `identity.key` somewhere safe, away from this
host.

**3. Lay down an account.** The password comes in on stdin, never on the
command line. It only opens the first sign in: the first device sets a new
one in the same step.

    printf '%s' "$SECRET" | docker compose exec -T maer-sync /maer-sync account someone

    account someone laid down

    docker compose exec maer-sync /maer-sync devices someone

    account someone: form=secret set-by-operator=no moved-at=not yet
    no device has signed in since version 6

An account that is already there is refused, and the command exits 1:

    refused: an account named 'someone' is already here

**4. Put a proxy with TLS in front.** It has to pass the WebSocket upgrade
through, and it has to send `X-Real-IP`. With Caddy, which fetches the
certificate and passes the upgrade by itself:

    sync.example.com {
        reverse_proxy 127.0.0.1:8099 {
            header_up X-Real-IP {remote_host}
        }
    }

With nginx, inside the `server` block that already holds the certificate for
the name:

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

**5. Prove the socket through the proxy.** Without a token the service
answers the upgrade with `403`: the upgrade got through. A `404` means the
proxy dropped it, and the push will be missing while everything else looks
fine.

    curl -s -o /dev/null -w '%{http_code}\n' -N \
      -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
      -H 'Sec-WebSocket-Version: 13' \
      -H "Sec-WebSocket-Key: $(head -c16 /dev/urandom | base64)" \
      https://sync.example.com/v1/changes

    403

**6. Point the app at it.** In Mär: Settings, the Mär Sync Service section.
The address with its scheme (`https://sync.example.com`), the owner
(`someone`) and the password from step 3. The app asks for a new password at
once, at least twelve characters.

Backups, updates and the way back are below: *Back it up* (step 9),
*Updating a running host*, and *More on the container*. The way without
Docker, by hand, is *Setting it up*.

## One implementation, one contract

The service is written in Rust and lives under `src/`. `CONTRACT.md` beside
this file is what binds it, and the app carries the same file with the same
digest.

**Until 24.09.2026 there were two.** The first implementation was Python, and
the Rust one was written against it: the route table in `vektoren/wege.json`
was measured against both sides, 28 cases, 0 differing. The Python build left
this repository on 24.09.2026. What it measured stays as a record: the files in
`vektoren/` are frozen at that state, and the Rust suite still holds itself to
them case by case (see `vektoren/README.md`). The last commit that carries the
Python build is `6415e8c`.

**The way back is the previous binary and a backup of the store**, not a
second language. See *Updating a running host, and the way back*.

## Setting it up

Where a step quotes what came back, it was run on a Debian host on 24.09.2026
against a throwaway store, and the output is the one that came back. Where a
step was not run as written, it says so where it stands.

### 1. What it needs

**Nothing on the host.** The binary is linked statically against musl and
carries its own runtime: no interpreter, no virtual environment, no system
library. What you need is a machine that can build it, and that machine needs
Docker: the build runs inside a pinned image so that the toolchain cannot
drift. `werkzeug/README.md` explains why, and what the pinning covers. Do not
build with a locally installed cargo unless you are prepared to answer what
your toolchain differs in.

Build host and target must match: the tool refuses to cross compile, so an
x86_64 host builds the x86_64 target and an aarch64 host builds its own.

**One thing in this repository is Python, and it is not the service.** Two
tests ask `git` what the repository tracks (license headers, hygiene). git
lives on the host and not in the build image, so they are written for the host,
in Python with the standard library only. You need `python3` and `git` to run
them, never to run the service.

### 2. Fetch it and build the product

    git clone https://codeberg.org/LzS/maer-sync.git
    cd maer-sync
    werkzeug/bau.sh build --release

**The first build needs the network**: it fetches the crates that `Cargo.lock`
pins, into `ziel/cargo-home` inside the project and not into your home. After
that, `--offline` holds every later build to exactly what was fetched:

    werkzeug/bau.sh build --release --offline

The result lands in
`ziel/abbau/<target>-unknown-linux-musl/release/maer-sync`, for example
`ziel/abbau/x86_64-unknown-linux-musl/release/maer-sync`, about 4.7 MB.

Check what you built, because a binary says nothing about itself yet (see
*Knowing which build is running*):

    ls -l ziel/abbau/*/release/maer-sync
    sha256sum ziel/abbau/*/release/maer-sync

### 3. Run the tests before you carry it anywhere

    werkzeug/vor-dem-commit.sh

This runs `git add -A`, then the repository fences, then the Rust suite, and
stops on red. `git add -A` comes first on purpose: the license fence asks git
what is tracked, so an untracked file would pass unseen. A green run ends in
`frei zum Committen.` The Rust suite alone is `werkzeug/bau.sh test --offline`.

**A skip is not a pass, and the run says which is which.** One case is skipped
on a plain host, and it names its reason: it reads a store that the Python
build laid down, handed in as `MAER_SYNC_PYTHON_DB`. Without one it abstains
instead of passing.

### 4. Put it on the host

The binary needs a place, a database, and a key. Nothing else.

    sudo install -o <serviceuser> -g <serviceuser> -m 755 maer-sync /srv/maer-sync/maer-sync

The service user needs write access to the directory holding the database and
the key, and nothing beyond it. **Not run as written here**: the measured host
keeps its binary under a path of its own.

### 5. The key: the first start makes it, and after that it is required

**On a new store the first start makes the key by itself**, and says so:

    a static key was made: M2KZ-6KRX-4CTH-KR3K-65ZK-J3EM (every device that connects will pin it)
    this service now answers as M2KZ-6KRX-4CTH-KR3K-65ZK-J3EM
    serving on 127.0.0.1:18231

**From then on the store remembers it**, the fingerprint and never the private
half, and a key file that is missing or different stops the service. It exits
with code **3** and binds no port, rather than growing a new identity that
every device would refuse. That is what a restore onto a new machine runs into;
see *The identity of this service*.

To make the key before the first start, or to replace it on purpose:

    sudo -u <serviceuser> env \
      MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite \
      MAER_SYNC_KEY=/srv/maer-sync/data/identity.key \
      /srv/maer-sync/maer-sync key --new

It prints the fingerprint. **Copy that key file somewhere safe now.** Clients
pin the fingerprint on first contact, so losing the key means every device has
to trust the service again from scratch.

### 6. Lay down an account

    printf '%s' "$SECRET" | sudo -u <serviceuser> env \
      MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite \
      /srv/maer-sync/maer-sync account someone

    account someone laid down

**The password comes in on stdin and never in the command line**, because an
argument is visible to every other process on the machine and lands in the
shell history. The service stores a scrypt hash, never the password.

**This password is one the operator knows, so it only ever opens a change.**
The account starts in the form `secret`; the first device that signs in reads
that and sets a new password in the same step, and from then on only the new
one signs in, as a proof the service cannot turn back into a password. A
password that the service or the operator once knew never becomes the key of
the feed credentials.

**An account that is already there is refused**, and the command exits 1:

    refused: an account named '<owner>' is already here

An existing account is not silently overwritten.

Check it:

    $ maer-sync devices someone
    account someone: form=secret set-by-operator=no moved-at=not yet
    no device has signed in since version 6

**A sign in cannot be tried with curl, and that is the point.** The three
sign-in paths answer only inside an encrypted session, whatever
`MAER_SYNC_PLAIN_WAYS` says:

    $ curl -s -X POST http://127.0.0.1:18231/v1/token \
        -H 'Content-Type: application/json' -d '{"owner":"someone","secret":"x","device":"d"}'
    {"reason":"the sign-in paths speak only inside a session"}

with status `426`. Run on 24.09.2026 on the free port `18231`. The whole sign
in, from the handshake to the move and the attempt limit, is driven on a
running build by `werkzeug/geheimnisprobe.py`, against a throwaway store.

### 7. Put a proxy in front, and prove the socket

**This is the step that goes wrong quietly.** A proxy that forwards REST but
drops the WebSocket upgrade leaves a service that looks completely healthy:
sign-in works, documents travel, and only the push is gone, so a change
arrives thirty seconds late instead of instantly. Nothing logs an error.

**Run on 16.09.2026, on nginx, in front of the Python build that stood here
then:** with the three upgrade lines the handshake answers `101` with a token
and `403` without. With them commented out, REST still answers `200` and the
upgrade returns **`404`**: not a hang and not a 502, which is exactly why
nothing looks broken. The proxy lines are the same for the Rust build, and it
answers the handshake the same way (`101` with a token, `403` with an empty
body without one).

The example below is written from the service's own requirements, not from a
machine this author set up; the proof after it is what matters.

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

**`X-Real-IP` is not decoration.** Guessing has a limit: ten failed sign ins
for one owner name from one address within fifteen minutes, and the next one
answers `429`. The service takes the address from `X-Real-IP` **only when the
connection comes from a proxy it trusts**: by default its own host
(`127.0.0.1` and `::1`), which is where the proxy sits. From anywhere else it
takes the peer and ignores the header, so nobody can choose the address they
are counted under. Without the line every device behind the proxy is one
address, and one person guessing a name locks out everyone who signs in under
it.

**A proxy somewhere else has to be named.** `MAER_SYNC_TRUSTED_PROXIES` is a
comma separated list of addresses, one machine each and no ranges. The service
prints the list it trusts when it starts (`trusting X-Real-IP from: …`), and
an entry it cannot read stops the start with exit code 6 and names the entry,
because a list that quietly grew shorter would count every device as one
again. The container sets this for you; see below.

**The proof, and run it against the PROXY, not against 127.0.0.1:**

    $ curl -s -o /dev/null -w '%{http_code}\n' -N \
        -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
        -H 'Sec-WebSocket-Version: 13' \
        -H "Sec-WebSocket-Key: $(head -c16 /dev/urandom | base64)" \
        -H "Authorization: Bearer $TOKEN" \
        https://sync.example.com/v1/changes
    101

**101 means the upgrade got through.** A `403` here means "not authorized" and
not "wrong network"; see the contract, which spells that difference out
because it is easy to read the wrong way.

**Only behind TLS.** The token rides in a header, and over plain http a
header is readable by everything between the device and the service.

### 8. Let it start by itself

**Or skip this step entirely and use the container** (see "Or run it in a
container" below): `restart: unless-stopped` is this step, and it needs no
unit file, no system user and no permissions fixed by hand.

**The store and its directory have to belong to the service user**, or the
service comes up and cannot write, or does not come up at all. **Not run as
written here**; it is the shape of the unit the measured host runs, with its
paths replaced.

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

**`MAER_SYNC_BIND` is one address, host and port together.** This is the line
that a unit written for the earlier Python build gets wrong. See
*Environment*.

### 9. Back it up, and let the backup prove itself

    sudo -u <serviceuser> env \
      MAER_SYNC_DB=/srv/maer-sync/data/sync.sqlite \
      /srv/maer-sync/maer-sync backup /backups/store-$(date +%F).sqlite

    fits: 1 documents, 7 bytes, came back unchanged

It copies the live store through SQLite's backup API, opens the copy the way
a restore would, compares every document by size, and exits 0 only when the
copy came back fitting. Schedule it and alarm on the exit code: **a backup
that was never opened is a file, not a backup.**

**A store with nothing in it does not back up as fitting.** It says

    not_measured: not measured, the backup held no documents

and exits 1, because a rehearsal that compared nothing has proven nothing.

**It copies the store and NOT the service's key file**, on purpose: a
private key inside a backup would let anybody holding an old copy answer as
this service. Copy `identity.key` separately, and keep it somewhere a
database backup does not go. See "The identity of this service" below.

**Run it with the binary that is serving, not with a newer one.** Opening a
store is allowed to raise its schema, and the backup opens the store. A newer
build that backs up the live store before a switch has already changed it.

### 10. Prove that the right thing is running

Not that something answers, but that it is yours:

    $ maer-sync health
    healthy: 127.0.0.1:18231

    $ curl -s -D - http://127.0.0.1:18231/v2/key
    HTTP/1.1 200 OK
    x-maer-contract: 6
    …
    {"fingerprint":"7TVS-52VV-2EKJ-8F3E-HP4A-FHMR","key":"…"}

(`18231` was the free port of the runs these lines come from.)

`health` asks `/v1/health` at the address in `MAER_SYNC_BIND` (an address
like `0.0.0.0` is asked on loopback) and exits 0 when the answer is `200` with
`{"ok":true}`, and 1 otherwise:

    not healthy: nothing answers on 127.0.0.1:18231: Connection refused (os error 111)

It proves that a service is up and answering well, not which one. `/v2/key`
answers with the fingerprint you wrote down in step 5, and names the contract
version it speaks, and comparing those two
is what proves the service is the one you laid down rather than a stranger on
a port you assumed was free.

The log tells you the rest. The service writes one line per request, with the
inner path, the status and how long it took:

    2026-09-23T22:23:28.219Z INFO maer.sync access from=127.0.0.1:39738 session=- inner=false method=GET path=/v1/health scope=- status=200 ms=0

A service that logs nothing at all under traffic is not idle, it is a build
from before `v0.2.0`.

### 11. Point the app at it

In Mär: Settings, the Mär Sync Service section. Three fields, the address
with its scheme (`https://sync.example.com`), the owner and the password from
step 6. The first device asks for a new password at once (step 6 says why);
the rule for it, at least twelve characters, lives in the app, because the
service never sees a password to judge. Signing in trades a proof for a token
the device keeps in its keychain.

The switch above it decides whether anything travels at all. Set up and
switched off means nothing moves, and the page says so.

## Environment

Measured by counting occurrences in the source, not from memory.

| Variable | Default |
|---|---|
| `MAER_SYNC_DB` | `maer-sync.sqlite` |
| `MAER_SYNC_KEY` | `identity.key` beside the store |
| **`MAER_SYNC_BIND`** | **`127.0.0.1:8081`** |
| `MAER_SYNC_PLAIN_WAYS` | on |
| `MAER_SYNC_MAX_BODY_BYTES` | 67108864 (64 MiB) |
| `MAER_SYNC_TRUSTED_PROXIES` | `127.0.0.1,::1`: whose `X-Real-IP` is believed, see step 7 |
| `MAER_SYNC_KATALOG` | empty: no catalog, see "Catalog index (optional)" |

**`MAER_SYNC_BIND` is the trap.** The earlier Python build read
`MAER_SYNC_HOST` and `MAER_SYNC_PORT`; this one reads neither, zero
occurrences in `src/`. A unit or compose file written for the old build
therefore sets two variables that nothing reads, the service falls back to its
default `127.0.0.1:8081`, starts cleanly, logs nothing unusual, and answers on
a port nobody is asking. There is no error to find, because nothing went wrong
from the service's point of view.

`MAER_SYNC_PLAIN_WAYS` deserves one more sentence, because it also fails
quietly. It reads `off`, `0`, `no` and `false`, case insensitive, and **it does
not trim whitespace**. A value of `off ` with a trailing space falls back to
the default, which is on. Do not check the unit file to see whether it worked,
check the behaviour: with plain ways off, `/v1/doc/...` answers **426** instead
of 401, and it does so even with a valid token, because the gate sits in front
of the token check.

## Catalog index (optional)

**Off unless you switch it on.** A host can carry a catalog index of public
sources, so devices read the free catalogs (titles, rights, covers later)
from their own server instead of asking every source themselves. The audio
file always comes from the source, never through this service.

**What it fetches.** A second program, `maer-sync-abruf`, in its own
container, asks seven sources: archive.org for LibriVox, its audiobooks, the
netlabels, the theme shelves of the 78 rpm collection and old time radio,
Wikimedia Commons for the recordings of Musopen, and Radio Browser for its
stations. One source at a time, one request a second at most, as
`Maer/1.0 (+https://maer.fm)`. Each source gets a full pass once a week and
a daily look at what was added; Musopen and Radio Browser cannot be asked
for that, so they take a full pass every day. Measured on 25.11.2026 for the
five archive.org sources: about 35 requests and 20 MB a day, and once a week
a full pass of about 165 MB. Project Gutenberg comes from its mirror
`gutenberg.pglaf.org`: once a week the catalog dump (about 21 MB) and the
record of every work of type Sound (about 1100, a few KB each, one a
second); a work whose record states copyright stays out. The radio play
feeds follow as their reader is built.

**The popularity step.** A full pass also asks each source for its number
(archive.org `downloads`, also in the daily look, where it goes unused;
Radio Browser `votes`; the Gutenberg record's downloads) and gives every row
the tenth of its source, `beliebt_stufe` 1 to 10; Musopen states no number.
Only a full pass sets it, so a row changes only when its tenth does. The
line of a run names `stufen_gewechselt=N`, the rows whose step moved.

**What it stores.** Records only (title, author, year, rights, shelves) in
one file, `katalog.db`, on its own volume; no audio, no account, no user
data. The fetch program has no port and never sees the user store; the
service mounts the catalog read only and holds no write right on it. Without
the file, or with it unreadable, the catalog paths answer 404 or 503 and
everything else works as before.

**Switch it on:**

    MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db \
      docker compose --profile katalog up -d

Set `MAER_SYNC_KATALOG` in the environment of compose (or in `.env`) so
the service reads the file, and start the profile so the fetch program runs.
Its log has one line per run.

**When a source shrinks.** The catalog refuses a full pass that would
empty a source, or delete more than a fifth of its live rows once it holds
50 or more; nothing is written, and the run is tried again later. Its line
then reads `ausgang=abgewiesen grund=Gesperrt(...)` with the rows read
(`gelesen`), the live rows the catalog holds (`lebend`) and the command
that confirms it. If the source really did shrink, confirm that one run:

    docker exec maer-sync-abruf /maer-sync-abruf --bestaetigt <quelle>

It runs one full pass of that source, writes it, prints its line with
`bestaetigt=ja` and ends with 0 (1 if the run failed). The confirmation
holds for that run only; the next large loss is refused again. While the
program is in a run of its own, the command ends at once with 1 and
`grund=Belegt`, nothing fetched or written; run it again once the running
run has ended.
One run at a time holds the file `katalog.db.lauf` beside the catalog.

**When the pages of a source do not end.** A pass reads at most the pages
its count calls for and two more, and the one that is refused. A source that answers past that, or names
a cursor a second time, is refused with `grund=Seiten(...)`; nothing is
written, and the run is tried again later.

**Switch it off:** `docker compose --profile katalog stop maer-sync-abruf`,
unset `MAER_SYNC_KATALOG`, and `docker compose up -d`; the catalog paths
answer 404 again. The volume `maer-sync-katalog` can be removed; it is
rebuilt on the next run.

## Updating a running host, and the way back

This is the one procedure where a mistake touches real data. The measured host
went through it on 2026-09-20, from the Python build to the Rust one; the same
steps carry every update since.

**Before you stop anything.** Build the product (steps 2 and 3), copy it to the
host, and rehearse against a **copy** of the live store, never against the live
one. The backup from step 9, run with the binary that is serving, is that copy.
Then run the new build against the copy on a free port and read the documents
back. If the new build raises the store schema, it does so on that copy, and
the run is what shows it rather than the reading.

Rehearse one more thing that no checklist mentions: **whether existing tokens
still work.** Present a token the running service issued to the new build on
the copy. If it answers 200, no device needs to sign in again.

**The switch itself.**

1. Stop the service and **measure** that the port is free, do not assume it.
2. Back up the store while it is stopped, with the binary that was serving,
   and check that the backup says `fits`.
3. Put the new binary in place and keep the previous one aside, under its own
   name.
4. Keep the key file where it is. A new binary is not a new identity.
5. `systemctl daemon-reload` if the unit changed, start, and check
   `is-active`.
6. Prove it with `health` **and** `/v2/key`, locally and through the proxy.
7. Count the documents with a backup and compare against step 2.

**The way back** is the previous binary and the backup from step 2: stop, put
the previous binary back, restore the backup over the store, start, prove.
**Everything written after that backup is lost**, and that is the price of the
way back rather than an accident of it. A newer build may have raised the
schema, and an older build refuses a store written by a newer one; it says so
and does not serve. So the backup goes back, not the file that the newer build
touched.

**Moving to version 6 has two conditions that no rehearsal on the host
shows.** Version 6 raises the store schema to 3, so an older build refuses the
store afterwards, and the way back is the backup. And `/v1/key` answers `404`
in version 6, which an app of version 5 reads as "the service is older than
me" and stops syncing, before it sends a password. **Every device of every
account speaks version 6 before the service does.** After the switch,
`maer-sync devices <owner>` says, per device, how it last signed in.

**The limit of that way back, said out loud:** it carries as long as the
clients still speak what the previous build answers. Once a client relies on
something only the newer contract has, the previous build cannot answer it,
and the way back becomes one directional. That is not an argument against the
order of deployment, it is the reason for it: service first, clients second.

## Knowing which build is running

The binary does not yet carry its version: `Cargo.toml` says `0.0.0`, and the
service does not print a version at startup. Until that is fixed, the honest
handles are indirect:

    systemctl show maer-sync.service -p ExecStart --value
    sha256sum /srv/maer-sync/maer-sync

and, for the question that actually matters in practice, whether the running
build carries the request log:

    grep -c 'access from=' /srv/maer-sync/maer-sync

A build from `v0.2.0` or later contains that string, an older one does not.
This is a workaround, not a feature, and it is listed here because the
alternative, extending `/v1/health`, is forbidden by the contract: that route
answers a status and nothing else.

## More on the container

The steps are in the *Quick start* at the top. This section says what the
image and the compose file decide, and why.

**Everything in this section was built and run on a Debian host on
24.09.2026**, against a copy of a live store with 78 documents, not written
from the documentation. Where something had to differ from a clean host, it
says so and why.

The image has two stages. The first builds the binary in the same pinned
environment as `werkzeug/bau.sh`; the second is empty (`FROM scratch`) and
receives the binary, the data directory, an empty `/tmp` and the two license
texts. No shell, no C library, no package manager, and so nothing underneath
the binary to update. Measured on 24.09.2026: the file
tree inside the image is 4867584 bytes as exported, of which the binary is
4818584 and the two license texts 37718; Docker lists the stored image at
7162711 bytes.

A container answers several of the steps above at once: step 1 was already no
question, and `restart: unless-stopped` **replaces step 8 entirely**: no unit
file, no system user, and no permissions to fix by hand on the store. **It
does not replace step 7.** The proxy in front still terminates TLS and still
has to pass the WebSocket upgrade through, and that is still the part that
fails quietly.

    docker compose build
    docker compose up -d
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

**If `8099` is taken on your host, set the port instead of editing the file:**

    MAER_SYNC_HOST_PORT=18099 docker compose up -d

Only the published port moves; inside the container the service stays on
`8099`, and so does everything the proxy in step 7 is configured against.

**A new volume makes its key on the first start. A restored store does not.**
Measured with a copy of a live store and no key file beside it: the container
refuses and restarts in a loop (`restarting`, exit code 3, seven restarts in
the first twelve seconds, the health check `unhealthy`), and every attempt
logs the same sentence with the way out. Put the key file of the old host into
the volume, and nothing changes for any device. Or make a new identity on
purpose, knowing that every device will ask again:

    docker compose run --rm maer-sync key --new
    docker compose up -d

### What the two files decide, and why

**Nothing underneath the binary.** The earlier Python image had to pin a base
image to the patch, because the base image was runtime. Here there is none:
the last stage is empty, and the build stage is pinned by digest and never
shipped.

**What is pinned, and what is not.** The build stage's base image, with its
Rust toolchain, is pinned by digest. The one package added to it, `musl-dev`,
is held to its version, `1.2.5`, and not to a revision: Alpine keeps only the
newest revision of a version, and a pin on one revision (`1.2.5-r11`) broke
every build on 08.11.2026, when the index had moved on to `1.2.5-r12`. A
revision is a fix of the same version, so two hosts on two days may build with
two revisions, never with two versions. If `1.2.5` leaves the index, the build
stops instead of taking another version. `tests/abbild.rs` holds both
Dockerfiles to the same version.

**The store is a named volume, never a path inside the image.** This is the
line that costs data if it is wrong: a store inside the image is gone with the
next build, and nothing about that is visible while reading the file.
Proven against the copy of a live store rather than asserted: the backup
inside the container said `fits: 78 documents, 3508517 bytes, came back
unchanged` after the first start, again after `docker compose restart`, and
again after a full `down` and `up`, and `/v2/key` answered with the same
fingerprint each time. The copy was written by a build of version 5 (schema
2); the build of version 6 opened it, raised it to schema 3 on the way (so
its code says; the number in the copy was not read), and served it. Those are the numbers the host's own nightly backup had
written for that very file.

**The user has a fixed numeric id, 10001.** The volume belongs to a uid, and
that uid has to survive rebuilds of the image, or the service comes back up
standing in front of its own store without permission to enter it. `docker top` shows both processes, the init and the service, running as
10001.

**Inside, the service listens on all addresses (`MAER_SYNC_BIND=0.0.0.0:8099`),
and outside only on `127.0.0.1`.** Inside a container the published port
reaches the service only if it listens beyond loopback; what is reachable from
outside the host is decided by the port line and by the proxy from step 7,
which is also what holds the certificate.

**Behind the published port the peer is the gateway of the container's
network, not `127.0.0.1`,** so the proxy on the host is not "its own host" to
the service. Measured on 24.09.2026: without the gateway in
`MAER_SYNC_TRUSTED_PROXIES`, ten failed sign ins from one address locked a
second address out too, because both were counted under the gateway. The
compose file therefore puts its network on a fixed subnet (`10.89.12.0/29`,
outside the ranges Docker hands out by itself) and trusts its gateway
(`10.89.12.1`); measured with it, the two addresses counted apart. If that
subnet collides with a network on your host, set `MAER_SYNC_SUBNET` and
`MAER_SYNC_GATEWAY` together.

**The health check asks the binary itself.** The image carries no curl and no
shell, so `/maer-sync health` asks `/v1/health` and exits 0 or 1. Its red probe was run: with the service stopped, `docker compose run --rm
maer-sync health` said `not healthy: nothing answers on 127.0.0.1:8099:
Connection refused (os error 111)` and exited 1; started again, it said
`healthy` and exited 0.

**An init stands in front of the service (`init: true`).** Without one the service is PID 1, and PID 1 has no
default action for a signal: `SIGTERM` does nothing, and every `stop` and
every `restart` waited out Docker's ten seconds and ended in `SIGKILL`, exit
code 137. Measured with it: `stop` took 409 ms and `restart` 502 ms, exit code
143. The init is the one Docker ships (`docker-init`), not a package in the
image. **If you run the image without Compose, pass `--init`.**

**`mem_limit: 256m` is high on purpose.** Measured against the copy with 78 documents:
about 8 MB in use, a peak of 16.9 MB including a backup. The limit stays high
because a single write may be as large as `MAER_SYNC_MAX_BODY_BYTES` (64 MiB)
and is held in memory before it is judged. A limit near the measured use
would stop the service on the first large write that is entirely legitimate.

### The suite and the image

The image carries no suite and nothing to run one with. The suite runs in the
build container (step 3), against the same sources and the same pins, and
`tests/abbild.rs` holds the pins in both Dockerfiles and in `bau.sh` equal, so
the image cannot quietly build with a different toolchain than the one the
suite ran on. **And the binary in the image is the binary `bau.sh` builds**, byte for byte:
the build stage uses the same paths (`CARGO_HOME`, `CARGO_TARGET_DIR`), and
on 24.09.2026 both came out as the same 4818584 bytes with the same
`sha256`. A host that installs by copy and a host that runs the container run
the same product.

### Why the port is a variable and not an override file

The first attempt at moving the port, on 16.09.2026, used a
`compose.override.yaml`, and the container did not start: **Compose merges
list entries, it does not replace them**: the override adds a *second* port,
the container tries to bind both, and it fails on the one that was taken. That
is the very port you were avoiding. Measured both ways: with the override,
`compose config` publishes two ports; with the variable, one. `ports:
!override` would replace (Compose 2.24 and newer), but it still needs a second
file for something one place in this one settles.

## Why it exists

Five kinds of data travel between a person's devices, and only one of them
(podcast positions) is redundant with a media server. The other four are
Mär's own and no media server knows them. There is no server that all Mär
users have, and iCloud blocks Android.

The full reasoning, and the requirements this is built against, live in the
deliveries repo, `kontext/sync-dienst-anforderungen.md`, **which a reader of
this repository does not have.** It binds the people who wrote this service;
it cannot bind a reader who cannot open it, and calling it binding here would
be a promise without cover. **What binds anyone reading THIS repository is
`CONTRACT.md` beside this file, and this file.** The requirements document is
named for the record, not as an instruction.

## What the core decides

The units below are pure and netless, each with its own tests. **No count
stands here on purpose:** a number in prose ages with every unit added, and
the list beside it is the thing that is true. They exist as units and not as
lines inside request handlers because these are the decisions that lose
people's data when they are wrong.

- **`src/admission.rs`**: may this state replace the stored one? Refuses
  an empty state over a full one, and a collapse below a floor, unless the
  client says it meant it. The known loss case is not a broken update; it
  is a device that writes an empty state at start-up before it has loaded.
- **`src/versions.rs`**: which stored versions survive a write. Keeping
  the last N covers a broken update and a client that wrote nonsense with
  one construction, and it is what "restore yesterday" is built on.
- **`src/restore.rs`**: did a rehearsed restore actually come back? Three
  outcomes, and the third is the point: a rehearsal that could not run is
  not a passing rehearsal.
- **`src/formats.rs`**: the two version numbers that make a rollback
  safe, kept apart: the document format (the client's) and the store schema
  (the service's).
- **`src/writes.rs`**: the three gates in an order, which is a decision
  of its own: precondition, then format, then size. A client behind the
  current version usually has a SMALLER state, so telling it "too small"
  before "read again" would send it to fix the wrong thing.
- **`src/authorization.rs`**: who is asking, and whether they may. **The
  requirements set this one apart from the rest**, and its own header says
  why: a mistake here opens other people's listening stands, and that is the
  kind you do not repair with an update. Three decisions live in it, each one
  an easy mistake: a missing account and a wrong secret give the SAME answer
  (anything else turns the endpoint into a list of who has an account here),
  every secret is compared in constant time, and the owner comes from the
  store's answer and never from anything the caller said.
- **`src/versuche.rs`**: how many failed sign ins one owner name has had
  from one address, and whether the next one is answered at all. Unknown
  names count the same, or the limit would say which names exist.
- **`src/paths.rs`**: the shape of a kind and a scope, never the
  membership. A list of the five known kinds would make "add a sixth" a
  service update.
- **`src/identity.rs`**: who the service IS, in a form a person can
  compare, and what to do with the key on disk against the one the store
  remembers.

Every unit answers with a verdict that carries its sentence. A refusal that
does not say why would be the failure the gates are built to prevent.

## The shell

- **`src/store.rs`**: SQLite, one row per version. The rotation IS the
  backup, and "restore yesterday" needs no second construction. The read,
  the decision and the insert happen inside one `BEGIN IMMEDIATE`, or the
  compare-and-set would be a story: two devices holding the same version
  would both be told they are current and the second would overwrite the
  first.
- **`src/wege.rs`**: the ways, and the tunnel through them. `GET`/`PUT`
  `/v1/doc/<kind>[?scope=]` with `ETag`/`If-Match`, the listing, the token
  exchange and `/v1/health`, each as a decision without I/O.
- **`src/main.rs`**: the wire, where the decision meets the connection;
  `src/aufstieg.rs`, `src/horcher.rs` and `src/listeners.rs` carry the
  changes socket at `/v1/changes` that announces every accepted write.
- **`src/handshake.rs`**, **`src/frames.rs`**, **`src/sessions.rs`** and
  **`src/sitzungsladen.rs`**: the encrypted session, from the handshake to
  the frame inside it to the rule for which session ends when.
- **`src/befehle.rs`**: the operator commands (`account`, `account-remove`,
  `backup`, `key`, `health`); **`src/protokoll.rs`**: what the operator sees
  in the log; **`src/kennung.rs`**: the key as a file.

### The interface

| | |
|---|---|
| `GET /v1/doc/<kind>` | the bytes, with `ETag` and `X-Maer-Format` |
| `PUT /v1/doc/<kind>` | the bytes; needs `X-Maer-Format` and, for an existing document, `If-Match` |
| `412` | you are looking at an old version: read, merge, write again |
| `409` | refused, and the body says which gate refused it |
| `X-Maer-Declared-Shrink: 1` | "I meant to make it smaller" |
| `GET /v1/docs/<kind>` | the scopes that hold a document of that kind, each with its version and format; names only, never bodies |
| `GET /v1/changes` | WebSocket; a line per accepted write: kind, scope, version |

A write that names no version at all is a `412` too. Silence is not a claim
to be current, and the blind overwrite is precisely what iCloud cannot
refuse and this can.

## Accounts

Every path but `/v1/health`, `/v2/key`, `/v1/session` and the three sign-in
paths needs `Authorization: Bearer <token>`, and the account the request acts on comes
from the store's answer to that token, never from anything the caller wrote.

    maer-sync account <owner>            # password on stdin, never in argv
    maer-sync account-remove <owner>     # the account and its documents leave,
                                         # after a backup that rehearsed its restore
    maer-sync reset <owner>              # a one-time password that only changes the password
    maer-sync devices <owner>            # how each device of an account signed in last

### Signing in (version 6)

The exact wire stands in `CONTRACT.md`; this is what an operator needs to
know about it.

- **Three paths, only inside a session.** `/v1/salt` tells a device the salt
  and the form of an account, `/v1/token` trades a proof for a token,
  `/v1/password` changes the password. A plain request gets `426`.
- **The service holds a proof, stretched once more with scrypt**, never a
  password. A thief of the store can still guess passwords, at the cost of
  both stretchings per guess; the running service learns nothing it could open
  a sealed document with. Tokens are stored as digests, so a leaked store
  hands out no working credential.
- **A wrong proof and an owner that is not here get the same answer**, and
  `/v1/salt` answers every name with a salt of the same shape, so the paths
  cannot be used to find out who has an account.
- **A password change withdraws every other token of the owner**, and a
  device holding one is told why (`X-Maer-Refusal: password-changed`).
- **Ten failed attempts per owner name and address in fifteen minutes**, and
  the next one is `429` with `Retry-After`. See step 7 for the address.
- **`reset <owner>` makes a one-time password and shows it once.** It signs
  nobody in (`428`); it only lets a device set a new password, and every
  token of the account is withdrawn. The operator knows it, which is exactly
  why it can never become the key of the feed credentials.

The token rides in a header, never in the address: a query string is copied
into every access log and proxy record between the device and the service.
The WebSocket handshake takes headers too: a native client sets them, and
it is only browsers that cannot, which is why so many services end up
putting tokens in URLs.

## What the service will NOT understand

It holds documents as bytes and never interprets them. The merge stays in the
client on every path, no transport can do it, measured (a WebDAV server
under a real collision keeps the last write and makes no conflict file), so
there is nothing to gain by teaching the service what a listening stand is,
and every piece of meaning it held would make a later change more
expensive.

It does check one thing about a write, and only this one: **that the body
reads as JSON.** What the service cannot read never replaces what it holds,
because storing an unreadable body takes the document away from every
reader, including the writer that sent it. The check runs for **every kind**,
and there is deliberately **no list of known kinds**: a new carrier in a
newer app has to be able to travel without a newer service first. This
service runs at its owner's, who updates it when they choose or never, so a
list here would refuse new writes exactly for the people who touch their
service least, and the failure would look to them like a fault of the app.

**The limit of that choice, named here so nobody finds it at a failure:** it
holds only while every format of this protocol is JSON. Today all of them
are. A compressed or binary format would be refused by this gate, and then
the service has to be updated first, which is the coupling the missing list
avoids. The cheaper half of the protection is already format blind: a body
shorter than the `Content-Length` it announced is refused by the reader
before the gate, and every write the app makes announces a length. So the
JSON check is the second net, it covers the writes that announce no length,
and a later non-JSON format needs that second net made conditional on the
format the write already names, not a list of kinds.

The one thing it will do that no other path can: **announce a change**
instead of waiting to be asked. The takeover has to reach a locked, playing
device, and a silent push measurably does not.

## Operation

**Only behind TLS.** The token rides in a header, and over plain http a
header is readable by everything between the device and the service. The
service itself binds to `127.0.0.1` and a proxy with a certificate stands
in front of it; that proxy has to carry the WebSocket upgrade on
`/v1/changes`.

    MAER_SYNC_DB=maer-sync.sqlite       # the store
    MAER_SYNC_KEY=<beside the store>    # the static key file, default identity.key
    MAER_SYNC_BIND=127.0.0.1:8081       # one address, host and port together
    MAER_SYNC_MAX_BODY_BYTES=67108864   # host protection, 413 above it
    MAER_SYNC_PLAIN_WAYS=on             # off: the five older ways answer only inside a session
    MAER_SYNC_KATALOG=                  # empty: no catalog; a path: read this catalog file

### The identity of this service

**`/v2/key` and `/v1/session` carry no token, and that is the pattern rather
than an omission.** `/v2/key` hands out the public half so a device can pin
it, and `/v1/session` trades a Noise handshake for an encrypted session. The caller proves who it is INSIDE the session,
which is what lets a sign in be protected: a sign in carries the account
secret and happens before there is any token to prove anything with.

**`/v2/key` is the trust on first use channel, and over a plain connection
whoever sits in the middle can answer it with their own key.** That is the
chosen migration: trust the first time, hold on afterwards. The holding
happens on the device, and it is what turns a later swap into an event.

On its first start the service makes itself a static key pair and prints the
fingerprint. From then on that fingerprint is who it is, and devices hold on
to it.

    $ maer-sync key
    key file: /var/lib/maer-sync/identity.key
    fingerprint: <fingerprint>
    this store expects: <fingerprint>
    the key <fingerprint> is the one this store expects

(inside the container, the fingerprint left out.)

**The private half is a file beside the store, and deliberately not in the
database.** A copy of the database travels to wherever backups travel, and a
private key in it would let anybody holding an old copy answer as this
service to every device that ever pinned it. The store remembers only the
fingerprint, which names without proving and may sit in a backup safely.

**So the key file belongs in your backup separately**, and a restore then
says the right thing by itself. Restored onto the same machine the key is
still there and nothing is asked. Restored onto a new one the key is not
there, and this is a different service now.

**A key that changed or went missing stops the service instead of growing a
new one.** The refusal names the fingerprint it expected, the one it found,
and the way out. The service exits 3 and binds no port.

Two commands say that a change is wanted, and nothing else in the service
will ever change an identity:

    maer-sync key --adopt   # the key here is the right one now
    maer-sync key --new     # make a new identity

`--adopt` is for moving a service onto a machine that keeps its own key.
`--new` is a deliberate rotation: **every device that pinned the old key
will refuse this service until somebody accepts the new one.** It keeps the
previous key file beside the new one as `identity.key.was-<date>-<time>`,
because a rotation done by mistake must not be unrecoverable. **Every
rotation keeps its own file**, so the second one does not write over what
the first put aside; the command prints the name it used.

### Inside a session

Once a device holds the key it opens a handshake on `/v1/session` and from
then on sends every ordinary request SEALED: `POST /v1/session` with the
session's name in `X-Maer-Session` and a sealed frame as the body. The frame
is the ordinary wire, whole (method, target, headers, body), the service runs
it through the same doors a plain request goes through, and the reply comes
back sealed. Nobody between the device and this process reads the token, the
path or the document. The exact shape stands in `CONTRACT.md`.

**The five older ways keep answering in the clear until you turn them off.**
A device that was not updated speaks in the clear, and it must keep syncing
until it is. When every device speaks inside, set `MAER_SYNC_PLAIN_WAYS=off`:
a plain request then gets a `426` with a sentence, and `/v1/health`,
`/v2/key` and `/v1/session` keep answering. The suite drives both settings;
**which one your devices need is measured on them, not here.**

**A restart ends every session, on purpose.** The keys live in memory and
nowhere else, so a session that survived a restart would be one whose keys
were on disk. The device answers a `410` with one new handshake and one
retry, and nobody sees a sentence.

**A socket that may not open is refused before the upgrade, with a `403` and
an empty body**, whatever the reason: a wrong path, no token, an unknown
token, or a plain socket while the plain ways are off. The reason is not lost:
the log line names it, with the close code it stands for (for example 4401
for a missing token, 4426 for a plain socket).
A client reads the `403` as "not authorized" the same as a REST 401, never as
"unreachable"; its REST calls carry the same token and answer first, so the
authorization state is driven there and the socket is only a silently
reopened accelerator.

**Every refusal leaves a line** in the log (`maer.sync`): the source address,
the owner, the document, the status and the gate's sentence; a failed sign-in
leaves the source address and the owner. Never a token, never a secret, never
a body, and a case in the suite reads the log to hold that. Without those
lines the case the service is built for, a device that writes empty at
start-up and a client that swallows the 409, would be invisible for as long
as it lasts.

**The body limit is host protection, not a data ceiling.** A document
larger than `MAER_SYNC_MAX_BODY_BYTES` is refused with 413 before it is
held in memory. It is generous on purpose and set per deployment; the
shrink gate protects the data, this protects the machine.

## Running the tests

    werkzeug/vor-dem-commit.sh          # everything, and stops on red
    werkzeug/bau.sh test --offline      # the Rust suite alone

The Rust suite runs in the pinned build image and prints its counts per file.
The repository fences run on the host, because they ask git. A case that
cannot put its question on this copy skips with its reason instead of passing.

## What is measured, and what is not

- The service runs as a static binary behind a TLS proxy on a host of the
  operator's; the app's sign-in, the document round trip and the changes
  socket are exercised there.
- The takeover end to end (a locked, playing device stops within the
  claim's cadence when the other starts) needs two devices and belongs to
  the device test.
- The fallback cadence when the changes socket is down (the app polls every
  thirty seconds until it reopens) is chosen, not measured; the measurement
  belongs to the same device test.

## About this project

The Mär sync service was created with support from AI: Anthropic Claude (Opus, Sonnet and Fable), plus locally hosted open-weight models (qwen3, devstral and OpenAI's open gpt-oss weights) run via Ollama for tooling.
