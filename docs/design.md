# How it is built

Why it exists, the one implementation and its contract, the core, the shell, and what it will not understand. Back to the [README](../README.md).

## Status

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
leaves the device (see [Signing in](accounts.md#signing-in-version-6) and `CONTRACT.md`). The takeover end
to end (a locked, playing device stops when the other starts) needs two
devices and is measured in the device test.

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
second language. See [Updating a running host, and the way back](operation.md#updating-a-running-host-and-the-way-back).

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

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
