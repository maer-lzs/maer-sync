# Operating a running host

Updates and the way back, which build is running, the identity of the service and the session. Back to the [README](../README.md).

## Updating a running host, and the way back

This is the one procedure where a mistake touches real data. The measured host
went through it on 2026-09-20, from the Python build to the Rust one; the same
steps carry every update since.

**Before you stop anything.** Build the product ([steps 2 and 3 of the setup](setup.md#2-fetch-it-and-build-the-product)), copy it to the
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

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
