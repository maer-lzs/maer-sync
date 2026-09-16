# Mär Sync Service

A small, self-hosted place for Mär's own data: listening stands, listening
statistics, Mär playlists, the choice family, and the playback claim.

**Optional, never required.** Mär works completely without it. It stands
beside iCloud as a second way, and the user picks; it is not a replacement
and not a default.

**Status: in the app since 3.0.** The netless decision units, a SQLite
store, a bare ASGI application, bearer-token authentication, the account
commands and the backup with its rehearsal, all under test. The app reaches
it through `MaerSyncCarriers.swift`: one session carries the five document
kinds (`stands`, `claim`, `stats`, `playlists`, `choices`) behind the
carrier seams, so the runners in the app know nothing of it; the settings
page (Settings, Sync) signs in, tests the connection and shows a refused
write with the service's own sentence. It runs behind uvicorn on a host of
the operator's, behind TLS. The takeover end to end (a locked, playing
device stops when the other starts) needs two devices and is measured in
the device test.

## Setting it up

**Every command below was run before it was written here**, on macOS with
Python 3.14 against a throwaway store; the outputs quoted are the ones that
came back. The two blocks that were NOT run are marked as such where they
stand, and the section at the end says what that leaves unproven.

### 1. What it needs

**Python 3.10 or newer**, and one line is the whole reason:

    core/authorization.py:72:    owner: str | None

`X | None` is the union spelling of PEP 604, which is 3.10 and up. **One
occurrence is enough, and that is the part worth writing down rather than a
count.** It is a field annotation, annotations are evaluated when the module
is loaded, and there is no `from __future__ import annotations` anywhere in
this repository to defer them (measured: zero). So on 3.9 the import fails
with a type error before a single request is served.

Nothing here needs anything newer than 3.10.

**And `sqlite3`, which is standard library and still worth one command.** It
is a C extension binding the system's SQLite, so a self-compiled or
slimmed-down Python can be missing it, and then the import fails before
anything else does. That is not a theoretical case for people who build their
own Python on a small server:

    $ python3 -c "import sqlite3, sys; print(sys.version.split()[0], sqlite3.sqlite_version)"
    3.14.6 3.53.4

**No SQLite version floor beyond that.** Measured against the sources: no
`VACUUM INTO`, no `RETURNING`, no `json_` functions, no upsert, no `STRICT`
tables. The only thing with a floor at all is `Connection.backup`, which is
Python 3.7 and up, so the 3.10 above already covers it.

Nothing else from the language itself. **Two things come from outside**, and
both are named where they are installed: an ASGI server, because the
application is finished without one but cannot be reached without one, and
`cryptography`, because the encrypted session needs X25519 and
ChaCha20-Poly1305 and Python ships neither. That is step 2, and it now comes
BEFORE the tests: the suite tests the encrypted session too, so it needs what
that session is built from.

### 2. Fetch it and install what it needs

    git clone https://codeberg.org/LzS/maer-sync.git
    cd maer-sync
    python3 -m venv .venv
    .venv/bin/python -m pip install --require-hashes -r requirements.txt

The application is finished without a server; it needs one that speaks ASGI.

    python3 -m venv .venv
    .venv/bin/python -m pip install --require-hashes -r requirements.txt

`--require-hashes` is not decoration. Without it the pinning in
`requirements.txt` is a suggestion, and the point of pinning a runtime that
terminates TLS-protected traffic is that it cannot change under you.

What that installs, and why each one:

| package | why |
| --- | --- |
| `uvicorn` | the ASGI server |
| `wsproto` | the WebSocket implementation, for `/v1/changes` |
| `h11`, `click`, `typing_extensions` | what those two need |
| `cryptography` | X25519 and ChaCha20-Poly1305 for the encrypted session |
| `cffi`, `pycparser` | what `cryptography` needs |

**The crypto half is pinned PER PLATFORM, and that is a difference worth
knowing before it surprises you.** `cryptography` is compiled: 45 wheels for
one version, and `cffi` another 99. The file carries the hashes for macOS on
arm64 and x86_64 and for Linux on x86_64 and aarch64, glibc and musl.
Anywhere else, `--require-hashes` fails loudly with "no matching
distribution" rather than installing something unpinned, and the fix is to
add that platform's hashes to the file.

### 3. Run the tests

    ./run-tests.sh

The suite needs no server, no network and no configuration, so a red run here
means the machine is wrong and not the setup. It prints its case count and
takes about 11.5 seconds, nearly all of it scrypt (see "Running the tests").
It uses `.venv` if one is there, which is why step 2 comes first now: the
handshake cases are held against published test vectors, and those need the
same primitives the service uses.

**`wsproto` and not `websockets`, on purpose:** uvicorn without its
`[standard]` extra carries no WebSocket at all ("No supported WebSocket
library detected", measured), and `/v1/changes` needs one. `websockets`
ships a wheel per platform, so a hash-pinned file would carry dozens of
hashes or quietly stop working on the machine it was not generated on.
`wsproto` is one pure-python wheel and one hash.

### 4. Run it

    MAER_SYNC_DB=/var/lib/maer-sync/store.sqlite .venv/bin/python -m service

No subcommand runs the server: that is the common case, and an unknown
command is refused rather than silently serving. `python3 -m service serve`
says the same thing out loud.

It answers immediately:

    $ curl -s http://127.0.0.1:8099/v1/health
    {"ok": true}

And nothing else does, without a token:

    $ curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8099/v1/doc/stands
    401

The knobs, all optional:

    MAER_SYNC_DB=maer-sync.sqlite       # the store
    MAER_SYNC_HOST=127.0.0.1            # bind address
    MAER_SYNC_PORT=8099
    MAER_SYNC_MAX_BODY_BYTES=67108864   # host protection, 413 above it

### 5. Lay down an account

    printf '%s' 'the-secret' | .venv/bin/python -m service account someone

    account someone laid down

**The secret comes in on stdin and never in the command line**, because an
argument is visible to every other process on the machine and lands in the
shell history. The service stores a scrypt hash, never the secret.

Check it from the outside:

    $ curl -s -X POST http://127.0.0.1:8099/v1/token \
        -H 'Content-Type: application/json' \
        -d '{"owner":"someone","secret":"the-secret"}'
    {"token": "…64 characters…"}

    $ curl -s -o /dev/null -w '%{http_code}\n' \
        -X PUT 'http://127.0.0.1:8099/v1/doc/stands?scope=' \
        -H "Authorization: Bearer $TOKEN" -H 'X-Maer-Format: 1' -d '{"a":1}'
    200

### 6. Put a proxy in front, and prove the socket

**This is the step that goes wrong quietly.** A proxy that forwards REST but
drops the WebSocket upgrade leaves a service that looks completely healthy:
sign-in works, documents travel, and only the push is gone, so a change
arrives thirty seconds late instead of instantly. Nothing logs an error.

**RUN, 16.09.2026**, on nginx in front of this service: with the three upgrade
lines the handshake answers `101` with a token and `403` without. With them
commented out, REST still answers `200` and the upgrade returns **`404`**:
not a hang and not a 502, which is exactly why nothing looks broken.

The example below is written from the service's own
requirements, not from a machine this author set up; the proof after it is
what matters and that one WAS run.

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
    }

**The proof, and run it against the PROXY, not against 127.0.0.1:**

    $ curl -s -o /dev/null -w '%{http_code}\n' -N \
        -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
        -H 'Sec-WebSocket-Version: 13' \
        -H "Sec-WebSocket-Key: $(head -c16 /dev/urandom | base64)" \
        -H "Authorization: Bearer $TOKEN" \
        https://sync.example.com/v1/changes
    101

**101 means the upgrade got through.** Measured directly against the service:
with a token the handshake answers `101 Switching Protocols`, without one
`403`. A `403` here means "not authorized" and not "wrong network"; see the
contract, which spells that difference out because it is easy to read the
wrong way.

**Only behind TLS.** The token rides in a header, and over plain http a
header is readable by everything between the device and the service.

### 7. Let it start by itself

**Or skip this step entirely and use the container** (see "Or run it in a
container" below): `restart: unless-stopped` is this step, and it needs no
unit file, no system user and no permissions fixed by hand.

**One thing this step does not say, and it costs an hour:** step 4 runs the
service as whoever you are, and this step runs it as `maer-sync`. **The store
and its directory have to belong to that user**, or systemd stops at
`status=200/CHDIR`, or the service comes up and opens a readonly database.
Measured on a Debian host, 16.09.2026.

**NOT RUN HERE**, for the same reason as the proxy: this author has no host
to install it on. It follows from what the service needs and nothing more.

    [Unit]
    Description=Maer sync service
    After=network.target

    [Service]
    User=maer-sync
    WorkingDirectory=/opt/maer-sync
    Environment=MAER_SYNC_DB=/var/lib/maer-sync/store.sqlite
    ExecStart=/opt/maer-sync/.venv/bin/python -m service
    Restart=on-failure

    [Install]
    WantedBy=multi-user.target

### 8. Back it up, and let the backup prove itself

    .venv/bin/python -m service backup /backups/store-$(date +%F).sqlite

    fits: 1 documents, 7 bytes, came back unchanged

It copies the live store through SQLite's backup API, opens the copy the way
a restore would, compares every document by size, and exits 0 only when the
copy came back fitting. Schedule it and alarm on the exit code: **a backup
that was never opened is a file, not a backup.**

### 9. Point the app at it

In Mär: Settings, the Mär Sync Service section. Three fields, the address
with its scheme (`https://sync.example.com`), the owner and the secret from
step 5. Signing in trades the secret for a token the device keeps in its
keychain; the secret itself is not kept.

The switch above it decides whether anything travels at all. Set up and
switched off means nothing moves, and the page says so.

## Or run it in a container

**Everything in this section was built and run on a Debian host on 16.09.2026**,
not written from the documentation: the image was built, started, restarted, and the
store was proven to survive a rebuild. Where something had to differ from a clean
host, it says so and why.

A container answers several of the steps above at once. It carries its own Python,
so step 1 stops being a question; it carries the pinned runtime from step 3; and
`restart: unless-stopped` **replaces step 7 entirely**: no unit file, no system
user, and no permissions to fix by hand on the store.

**It does not replace step 6.** The proxy in front still terminates TLS and still
has to pass the WebSocket upgrade through, and that is still the part that fails
quietly.

    docker compose build
    docker compose up -d

    $ curl -s http://127.0.0.1:8099/v1/health
    {"ok": true}

**If `8099` is taken on your host, set the port instead of editing the file:**

    MAER_SYNC_HOST_PORT=18099 docker compose up -d

Only the published port moves; inside the container the service stays on `8099`,
and so does everything the proxy in step 6 is configured against.

### What the two files decide, and why

**The base image is pinned to the patch, not to `3.13`.** A drifting base image
turns `--require-hashes` in `requirements.txt` into half a promise: the point of
pinning the runtime under a TLS-terminated service is that it cannot change under
you, and the base image *is* runtime.

**The store is a named volume, never a path inside the image.** This is the line
that costs data if it is wrong: a store inside the image is gone with the next
build, and nothing about that is visible while reading the file. Proven rather than
asserted: an account and a document were laid down, the container was removed, **the
image was deleted**, both were rebuilt, and the account and the document were still
there.

**The user has a fixed numeric id.** The volume belongs to a uid, and that uid has
to survive rebuilds of the image, or the service comes back up standing in front of
its own store without permission to enter it.

**It does not run as root**, and `docker exec … id` says so: `uid=10001(maersync)`.

**The port is bound to `127.0.0.1` only.** What is reachable from outside is decided
by the proxy from step 6, which is also what holds the certificate.

### Running the suite in the image

    docker compose exec maer-sync /opt/maer-sync/.venv/bin/python -m unittest discover -s tests -t .

**The suite is green in the image: `Ran 193 tests … OK (skipped=4)`.** Measured on
16.09.2026 in the built image, as the unprivileged service user, running exactly that
module invocation in the built image.

**That number moves with the encrypted session**: seven cases came with it, so the
next run in the image should say 200. It is left as measured rather than predicted,
because a number in this file that nobody ran is the thing this file keeps warning
about; whoever builds the image next writes what it actually said.

The four skips are the questions this copy cannot put. Three hygiene tests and one
fence ask `git` what the repository tracks; an image carries the sources without a
repository and without a git program, so they report a skip **with its reason**
instead of a result. Nothing is declared expected here, and that is deliberate.

**Two earlier versions of this section did declare failures expected, and each time
the sentence hid a real one.** First it said three, and the fourth was a fence that
read `.venv` as though it were this service. Then it said four, and three of those
were not abstentions at all but a hole: a missing git program made the call *raise*
where a missing repository only made it return, and the raise came back as an error.
A sentence that declares failures normal is a licence to stop looking, and a real
failure then disappears among the correct ones. If a number appears here again, it
belongs to a measured run and says what it counted.

### What was run here, and what is the host's problem

**Run against the container** (16.09.2026, this host): `docker compose build`,
`docker compose up -d` with the port variable set, `/v1/health` answering `{"ok": true}`,
the healthcheck reaching **healthy**, an account laid down and still present after a
full `down` and `up` (read back out of the database, not inferred from the file being
there), and a clean `down` afterwards. The suite inside the image: `Ran 193 tests … OK
(skipped=4)`.

**Run against the service from step 4, not against the container:** the token exchange,
`401` without a token, and the WebSocket upgrade through the proxy (`101` with a token,
`403` without). They are listed apart on purpose. An earlier version of this section
claimed them for the container, and that claim had no run behind it: the container part
had failed on the port collision at the time, which is what the paragraph below is about.

**The host's, not the image's:** this machine already runs a service on `8099`, so the
run above set `MAER_SYNC_HOST_PORT=18099`. The delivered default stays `8099`, which is
what a host without that collision wants.

**Why the port is a variable and not an override file.** The first attempt here did use
a `compose.override.yaml`, and the container did not start: **Compose merges list
entries, it does not replace them**: the override adds a *second* port, the container
tries to bind both, and it fails on the one that was taken. That is the very port you
were avoiding. Measured both ways on this host: with the override, `compose config`
publishes two ports (`8099` and `18099`); with the variable, one. `ports: !override`
would replace (Compose 2.24 and newer), but it still needs a second file for something
one place in this one settles.

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
stands here on purpose:** the sentence said "four" while listing six, and there
are seven. A number in prose ages with every unit added, and the list beside it
is the thing that is true. They exist as units
and not as lines inside request handlers because these are the decisions
that lose people's data when they are wrong.

- **`core/admission.py`**: may this state replace the stored one? Refuses
  an empty state over a full one, and a collapse below a floor, unless the
  client says it meant it. The known loss case is not a broken update; it
  is a device that writes an empty state at start-up before it has loaded.
- **`core/versions.py`**: which stored versions survive a write. Keeping
  the last N covers a broken update and a client that wrote nonsense with
  one construction, and it is what "restore yesterday" is built on.
- **`core/restore.py`**: did a rehearsed restore actually come back? Three
  outcomes, and the third is the point: a rehearsal that could not run is
  not a passing rehearsal.
- **`core/formats.py`**: the two version numbers that make a rollback
  safe, kept apart: the document format (the client's) and the store schema
  (the service's).

- **`core/writes.py`**: the three gates in an order, which is a decision
  of its own: precondition, then format, then size. A client behind the
  current version usually has a SMALLER state, so telling it "too small"
  before "read again" would send it to fix the wrong thing.
- **`core/authorization.py`**: who is asking, and whether they may. **The
  requirements set this one apart from the rest**, and its own header quotes
  why: a mistake here opens other people's listening stands, and that is the
  kind you do not repair with an update. Three decisions live in it, each one
  an easy mistake: a missing account and a wrong secret give the SAME answer
  (anything else turns the endpoint into a list of who has an account here),
  every secret is compared in constant time, and the owner comes from the
  store's answer and never from anything the caller said. It was missing from
  this list while the sentence above it claimed to count the units.
- **`core/paths.py`**: the shape of a kind and a scope, never the
  membership. A list of the five known kinds would make "add a sixth" a
  service update.

Every unit answers with a verdict that carries its sentence. A refusal that
does not say why would be the failure the gates are built to prevent.

## The shell

- **`service/store.py`**: SQLite, one row per version. The rotation IS the
  backup, and "restore yesterday" needs no second construction. The read,
  the decision and the insert happen inside one `BEGIN IMMEDIATE`, or the
  compare-and-set would be a story: two devices holding the same version
  would both be told they are current and the second would overwrite the
  first.
- **`service/app.py`**: a bare ASGI application. `GET`/`PUT`
  `/v1/doc/<kind>[?scope=]` with `ETag`/`If-Match`, a WebSocket at
  `/v1/changes` that announces every accepted write, and `/v1/health`.
- **`service/__main__.py`**: binds the application to an application
  server and carries the operator commands (`account`, `account-remove`,
  `backup`).

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

Every path but `/v1/health` and `/v1/token` needs `Authorization: Bearer
<token>`, and the account the request acts on comes from the store's answer
to that token, never from anything the caller wrote.

    python3 -m service account <owner>            # secret on stdin, never in argv
    python3 -m service account-remove <owner>     # the account and its documents leave,
                                                  # after a backup that rehearsed its restore

`POST /v1/token` with `{"owner", "secret", "device"}` hands back a token for
one device. A wrong secret and an account that is not here get the same
answer and take the same time, so the path cannot be used to find out who
has an account. Secrets are stored with scrypt at parameters carried in the
stored form; tokens are stored as digests, so a leaked store hands out no
working credential.

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
    MAER_SYNC_HOST=127.0.0.1            # bind address
    MAER_SYNC_PORT=8099
    MAER_SYNC_MAX_BODY_BYTES=67108864   # host protection, 413 above it

**The runtime is uvicorn plus a WebSocket library, both pinned.** The
service code and its tests are standard library only, but serving it is
not: uvicorn without its `[standard]` extra carries no WebSocket ("No
supported WebSocket library detected", measured), and `/v1/changes` needs
one, so the deployment installs `websockets` (or `wsproto`) alongside
uvicorn. One package, not the whole `[standard]` set. No web framework was
taken: the interface is "fetch bytes, put bytes, announce a change", and a
framework buys routing sugar for that and little else; Flask was never a
candidate, because WSGI cannot carry a WebSocket. Pin both with hashes.

**An unauthorized socket is refused in the handshake.** The service closes
a token-less `/v1/changes` with WebSocket code 4401 before it accepts the
connection; uvicorn surfaces that pre-accept close to the client as an
HTTP 403 on the handshake, not as a close frame. A client reads that as
"not authorized" the same as a REST 401, never as "unreachable"; its REST
calls carry the same token and answer 401 first, so the authorization
state is driven there and the socket is only a silently reopened
accelerator.

**Every refusal leaves a line** on stderr (`maer.sync`): the source
address, the owner, the document, the status and the gate's sentence; a
failed sign-in leaves the source address and the owner. Never a token,
never a secret, never a body, and a case in the suite reads this file to
hold that. Without those lines the case the service is built for, a device
that writes empty at start-up and a client that swallows the 409, would be
invisible for as long as it lasts.

**The body limit is host protection, not a data ceiling.** A document
larger than `MAER_SYNC_MAX_BODY_BYTES` is refused with 413 before it is
held in memory. It is generous on purpose and set per deployment; the
shrink gate protects the data, this protects the machine.

**The backup rehearses its own restore:**

    python3 -m service backup /backups/maer-sync-$(date +%F).sqlite

copies the live store through SQLite's backup API, opens the copy in a
throwaway directory the way a restore would, compares every document
version by size, and exits 0 only when the copy came back fitting. The
operator schedules it and alarms on the exit code; a backup that was
never opened is a file, not a backup.

## Running the tests

    ./run-tests.sh

No dependencies: the whole service is standard library only, and the tests
run under `unittest`. The run prints its case count on purpose.

It takes about 11.5 seconds rather than a few milliseconds (measured 11.3 to
11.6 over several runs, 182 cases before the contract fence joined them), and
all of that is
scrypt: the account cases run the real password hashing at the real
parameters. Turning those down for the suite would make it quick and would
stop it testing the thing that matters.

## What is measured, and what is not

- The service runs behind uvicorn and a TLS proxy on a host of the
  operator's; the app's sign-in, the document round trip and the changes
  socket are exercised there.
- The takeover end to end (a locked, playing device stops within the
  claim's cadence when the other starts) needs two devices and belongs to
  the device test.
- The fallback cadence when the changes socket is down (the app polls every
  thirty seconds until it reopens) is chosen, not measured; the measurement
  belongs to the same device test.
