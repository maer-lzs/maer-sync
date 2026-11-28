# Running it in a container

The quick start with Docker as it was run, and what the image and the compose file decide. Back to the [README](../README.md).

## Quick start with Docker

**Run on 28.11.2026 on a Debian host (x86_64, Docker 29.9.3, Compose
5.7.1)**, from a fresh clone of `github.com/maer-lzs/maer-sync` at the tag
`v0.4.0`, with its own project name and port beside a running service
(`COMPOSE_PROJECT_NAME=maer-sync-probe`, `MAER_SYNC_HOST_PORT=18199`). The
output under each command is the one that came back; where the published
port shows, it is that port. **Not run in that probe: the proxy and the app
(steps 4 and 6).** The proxy lines are written from the service's
requirements, step 5 was run against the published port without a proxy,
and signing in needs a device with the app. The earlier run of steps 1 to 3
on 08.11.2026 gave the same answers. `v0.4.1` changes nothing in the
service, so the commands below name it.

You need a Linux host with Docker (with Compose) and git, and a name that
points at it, here `sync.example.com`. Six steps from nothing to a signed in
device:

**1. Fetch it and start it.** The first start builds the image and needs the
network; build and start took 68 seconds on the measured host, and take
longer on a slower one.

    git clone https://github.com/maer-lzs/maer-sync.git
    cd maer-sync
    git checkout v0.4.1
    docker compose up -d --build
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

**2. Note the key, and keep a copy of it.** The first start makes the key of
this service and says so in the log. Every device pins it on first contact;
losing the key means every device has to trust the service again.

    docker compose logs --no-log-prefix maer-sync | grep 'answers as'

    this service now answers as TPNW-8BC8-Z8ZH-PRZ6-VEX9-XWZ0

    docker compose cp maer-sync:/var/lib/maer-sync/identity.key ./identity.key

     maer-sync Copying maer-sync:/var/lib/maer-sync/identity.key to ./identity.key
     maer-sync Copied maer-sync:/var/lib/maer-sync/identity.key to ./identity.key

The key file is 45 bytes. Put `identity.key` somewhere safe, away from this
host. (The name above belonged to the throwaway service of the probe, which
is gone.)

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

**5. Prove the key and the socket through the proxy.** `/v2/key` answers
with the contract version in a header. Without a token the service answers
the upgrade with `403`: the upgrade got through. A `404` means the proxy
dropped it, and the push will be missing while everything else looks fine.
In the probe both went to the published port, without a proxy:

    curl -s -D - -o /dev/null https://sync.example.com/v2/key | grep -i -E '^HTTP|x-maer'

    HTTP/1.1 200 OK
    x-maer-contract: 6

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

**A backup of a store with nothing in it does not pass.** Right after step
3 no device has written anything, and the probe's backup said so and exited
1, as it should (see [step 9 of the setup](setup.md#9-back-it-up-and-let-the-backup-prove-itself)):

    docker compose exec maer-sync /maer-sync backup /var/lib/maer-sync/probe-backup.sqlite

    not_measured: not measured, the backup held no documents

### Updating the container, and the way back

Run in the same probe on 28.11.2026. **Keep the image that is serving under
a name of its own before you build the next one**; it is the way back.

    docker image tag maer-sync:local maer-sync:previous
    git fetch origin && git checkout <the new tag>
    docker compose up -d --build
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

When the service did not change between the two versions (from `v0.4.0` to
the `dev` of the same day only the fetch program did), the build gives the
same image and Compose leaves the container running (`Container maer-sync
Running`). The way back puts the kept image under the name Compose starts,
on the same volume:

    docker image tag maer-sync:previous maer-sync:local
    docker compose up -d --no-build --force-recreate maer-sync
    docker compose exec maer-sync /maer-sync health

    healthy: 127.0.0.1:8099

    docker compose exec maer-sync /maer-sync devices someone

    account someone: form=secret set-by-operator=no moved-at=not yet
    no device has signed in since version 6

**An older tag may no longer build, so do not count on rebuilding it.**
Measured in the same probe: `v0.3.0` pins the build package to its revision
(`musl-dev=1.2.5-r11`), Alpine now carries only `r12`, and the build stops
with `breaks: world[musl-dev=1.2.5-r11]`. From `v0.4.0` on the pin holds the
version and not the revision (see *What the two files decide* below). The
kept image is the way back; the store schema decides whether it may open the
store again (see [Updating a running host](operation.md#updating-a-running-host-and-the-way-back)).

### The catalog index in the container

Run in the same probe on 28.11.2026 at `v0.4.0`; it took 72 seconds to build
the fetch program and start both:

    MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db \
      docker compose --profile katalog up -d --build
    docker compose logs --no-log-prefix maer-sync-abruf

    abruf: started, catalog /var/lib/maer-sync-katalog/katalog.db
    lauf quelle=archive-buch art=voll gelesen=0 version=0->0 dauer_ms=604 ausgang=abgewiesen grund=Echo { ... }

The line is cut short here. Its refusal is a fault of `v0.4.0`: archive.org
states its query back in another spelling, and `v0.4.0` refused four sources
for it. The fix came with `v0.4.1`; on the host that carries this service
(under systemd, not in a container) all four sources then ran without a
refusal, on 28.11.2026. What the catalog does, and how to switch it off again:
[the catalog index](catalog.md).

**Clearing up after a probe.** `docker compose --profile katalog down -v`
removes the containers, both volumes and the network; the images stay until
`docker image rm maer-sync:local maer-sync-abruf:local`. Counted after the
probe: no container, volume or network of the probe left, and no image of it.

## More on the container

The steps are in the [quick start](#quick-start-with-docker) above. This section says what the
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

A container answers several of the [setup steps](setup.md) at once: step 1 was already no
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
build container ([step 3 of the setup](setup.md#3-run-the-tests-before-you-carry-it-anywhere)), against the same sources and the same pins, and
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

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
