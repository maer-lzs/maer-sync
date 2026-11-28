# Mär Sync Service

A small, self-hosted service for the data of the [Mär](https://maer.fm) audio app: listening positions, listening statistics, Mär playlists, your choices, and which device is playing right now.

**Optional, never required.** Mär works completely without it. It stands beside iCloud as a second way, and you pick one; it is not a replacement and not a default. It is for people who want this data on a host of their own, or who use devices without iCloud.

> „Mär“ is an old German word for a tale or story. The service is one statically linked Rust binary with one SQLite file, run behind a proxy that holds the certificate, or in a container.

> **Note:** This is a private hobby project without warranty.

**Home:** <https://maer.fm> · **Source:** <https://github.com/maer-lzs/maer-sync> · **App:** <https://github.com/maer-lzs/maer>

## What it keeps

One SQLite file holds everything the service knows, per account, in six kinds of documents:

- `stands`: listening positions
- `stats`: listening statistics
- `playlists`: Mär playlists
- `choices`: the choices you make in the app
- `claim`: which device is playing, so that the other one stops
- `feedauth`: feed credentials, sealed on the device with a key that never leaves it

The service never reads what is inside a document; it keeps it and hands it back. Every device that signs in keeps up through a socket that tells it when something changed. Audio never goes through the service.

## What travels, and where

- **Between your devices and your service, nothing else.** The service makes no requests of its own to anyone. It answers your devices and nothing more.
- **Only encrypted.** Behind a TLS proxy, as described below. Where a device reaches the service over plain `http`, every request goes inside an encrypted session (Noise `NK`) bound to the key of your service, which each device pins on first contact.
- **No password on the service.** Since contract version 6 a device derives a proof from the password and signs in with that; the service stores the proof stretched once more, never the password.
- **What the service writes down besides the documents:** per device, the form in which it last signed in (`maer-sync devices <owner>` shows the forms, never a value), and one log line per request with the source address, method, path and status.
- **The optional catalog index** is a second program that asks public catalogs (archive.org, Wikimedia Commons for Musopen, Radio Browser and a Project Gutenberg mirror), one request a second at most, as `Maer/1.0 (+https://maer.fm)`. It is off until you switch it on, and it never sees the user data.

Statement by statement, with the paths and the contract: [docs/design.md](docs/design.md) and [docs/accounts.md](docs/accounts.md).

## Requirements

- A Linux host with **Docker** (with Compose) and git, x86_64 or arm64; or, for the way without a container, a machine with Docker to build the binary and any Linux host to run it
- A **name** that points at the host, and a **proxy with TLS** in front (Caddy or nginx; examples in the quick start)
- **Mär** with a sync service setting that speaks contract version 6
- Nothing else on the host: no interpreter, no system library, no package manager inside the image

## Quick start

Run on 28.11.2026 from a fresh clone at `v0.4.0` on a Debian host; the full run with its output is in [docs/container.md](docs/container.md).

1. Fetch it and start it:

       git clone https://github.com/maer-lzs/maer-sync.git
       cd maer-sync
       git checkout v0.4.0
       docker compose up -d --build
       docker compose exec maer-sync /maer-sync health

       healthy: 127.0.0.1:8099

2. Keep a copy of the key of your service, away from the host. Every device pins it; losing it means every device has to trust the service again:

       docker compose cp maer-sync:/var/lib/maer-sync/identity.key ./identity.key

3. Lay down an account. The password comes in on stdin, never on the command line, and only opens the first sign in:

       printf '%s' "$SECRET" | docker compose exec -T maer-sync /maer-sync account someone

       account someone laid down

4. Put a proxy with TLS in front that passes the WebSocket upgrade through, and prove it: without a token the upgrade answers `403`; a `404` means the proxy dropped it. The lines for Caddy and nginx: [docs/container.md](docs/container.md#quick-start-with-docker).

If `8099` is taken on your host, start with `MAER_SYNC_HOST_PORT=18099` in front of `docker compose up`; the service inside stays on `8099`.

Without Docker, step by step, with a unit file and a backup that proves itself: [docs/setup.md](docs/setup.md).

## Log in inside the app

- In Mär: Settings, the Mär Sync Service section
- Address with its scheme, `https://sync.example.com`, the owner (`someone`) and the password from step 3
- The app asks for a new password at once, at least twelve characters; from then on the service holds only its proof

What a device does when it signs in, and how a forgotten password comes back: [docs/accounts.md](docs/accounts.md).

## Backups and updates

- `maer-sync backup <file>` copies the store, opens the copy as a restore would and exits 0 only when every document came back. A store with nothing in it says `not_measured` and exits 1, on purpose.
- The key file is not in that backup, on purpose; copy it separately.
- Before an update, keep the image or binary that is serving under a name of its own. An older tag may no longer build (measured: `v0.3.0` does not, since its build package left Alpine), so the kept image is the way back.

The whole procedure, and what the way back costs: [docs/operation.md](docs/operation.md).

## Catalog index (optional)

The service can carry an index of free catalogs, so devices read them from your host instead of asking every source themselves. Off unless you set `MAER_SYNC_KATALOG` and start the profile of the fetch program:

    MAER_SYNC_KATALOG=/var/lib/maer-sync-katalog/katalog.db \
      docker compose --profile katalog up -d

What it asks, what it stores and how to switch it off: [docs/catalog.md](docs/catalog.md).

## More documentation

- [Running it in a container](docs/container.md), the quick start as it was run, and what the image decides
- [Setting it up by hand](docs/setup.md), and the environment the service reads
- [Operating a running host](docs/operation.md): updates, the way back, the identity of the service
- [Accounts and signing in](docs/accounts.md)
- [The catalog index](docs/catalog.md)
- [How it is built](docs/design.md): the core, the shell, and what it will not understand
- [Tests and measurements](docs/testing.md)
- [The contract](CONTRACT.md), the same file in the app

Every page also in German: [README.de.md](README.de.md).

## License

GPL-3.0-or-later, see [LICENSE](LICENSE). Additional terms under GPL-3.0 section 7 apply, see [LICENSE-ADDITIONAL-TERMS](LICENSE-ADDITIONAL-TERMS). The fetch program carries the licence texts of the parts it is built from, listed in [abruf/FREMDTEILE.md](abruf/FREMDTEILE.md).

## About this project

The Mär sync service was created with support from AI: Anthropic Claude (Opus, Sonnet and Fable), plus locally hosted open-weight models (qwen3, devstral and OpenAI's open gpt-oss weights) run via Ollama for tooling.
