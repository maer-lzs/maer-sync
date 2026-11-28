# Setting it up by hand

The way without Docker, step by step, and the environment the service reads. Back to the [README](../README.md).

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

    git clone https://github.com/maer-lzs/maer-sync.git
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
[Knowing which build is running](operation.md#knowing-which-build-is-running)):

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
see [The identity of this service](operation.md#the-identity-of-this-service).

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
again. The container sets this for you; see [the container](container.md).

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

**Or skip this step entirely and use the container** (see [the container](container.md)): `restart: unless-stopped` is this step, and it needs no
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
[Environment](#environment) below.

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
database backup does not go. See [The identity of this service](operation.md#the-identity-of-this-service).

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

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
