# The catalog index (optional)

What the fetch program asks, what it stores, and how to switch it on and off. Back to the [README](../README.md).

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

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
