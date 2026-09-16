# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The service's static key pair: where it lives, and who may change it.

`core/identity.py` holds the form and the decision. This file holds the two
things that touch the world: a file on disk and a row in the store.

**The private key is a FILE, not a row in the database, and that is the
decision of this slice.** The store already refuses to hold live credentials
for a stated reason: a backup of it would carry them further. `python3 -m
service backup` writes a copy of the database to a path the operator names,
and that copy travels to wherever backups travel. A static private key in it
would mean that anybody holding an old backup can answer as this service to
every device that ever pinned it, and pinning would buy nothing.

Keeping the key beside the database instead makes the ordinary recovery say
the right thing by itself:

- Restore the database onto the SAME machine and the key is still there, so
  every device carries on and nothing is asked.
- Restore it onto a NEW machine and the key is not there. That is a different
  service now, and it says so instead of quietly pretending to be the old one.

**The store remembers the fingerprint, and that is what makes a change an
EVENT.** Without the memory, a missing key file would simply grow a new key
and the service would serve happily while every pinned device failed with
nothing to read. With it, the service stops and names the two ways out. The
memory is a fingerprint and never the key, so it may sit in a backup.

**What the operator does to WANT a change** is a command, not a file edit:
`key --adopt` accepts the key that is there, `key --new` makes one. Both say
out loud that every device will ask again.

**Not measured here:** nothing in this file has run against a live
deployment. The suite drives it with real files in temporary directories;
what the container does with a volume on first start is written and
unexercised, and it says so rather than letting a reader assume otherwise.
"""

import base64
import logging
import os
import pathlib
import time

from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey

from core.identity import decide, fingerprint

log = logging.getLogger("maer.sync")

#: The name beside the database. Not derived from the database's own name:
#: `store.sqlite.key` would be swept up by every backup glob that catches
#: `store.sqlite*`, which is exactly the file that must not travel with it.
KEY_NAME = "identity.key"

#: Owner read and write, nothing else. A key file anybody on the host can
#: read is the same loss as a key in a backup, one machine closer.
KEY_MODE = 0o600


class RefusedToServe(Exception):
    """The key on disk is not the key this store expects. Carries the
    reason from `core.identity`, because a refusal that reaches a terminal
    without saying what to do about it is only half a refusal."""


def key_path(db_path=None):
    """Where the key lives: `MAER_SYNC_KEY`, else beside the database."""
    named = os.environ.get("MAER_SYNC_KEY")
    if named:
        return pathlib.Path(named)
    db = pathlib.Path(db_path or os.environ.get("MAER_SYNC_DB", "maer-sync.sqlite"))
    return db.parent / KEY_NAME


def read_key(path):
    """The private key from its file, or None when there is no file.

    A file that is there and unreadable as a key is NOT treated as absence.
    Absence means "this service is new"; a damaged file means somebody has a
    key and it is not this one, and starting fresh on top of it would throw
    the real one away.
    """
    path = pathlib.Path(path)
    if not path.exists():
        return None
    text = path.read_text(encoding="utf-8").strip()
    try:
        raw = base64.b64decode(text, validate=True)
    except Exception as broken:
        raise RefusedToServe(
            f"the key file {path} is there but does not read as a key ({broken}). "
            "It is not being replaced, because a new key here would lose whatever "
            "the damaged one was. Restore the file or move it aside deliberately.")
    if len(raw) != 32:
        raise RefusedToServe(
            f"the key file {path} holds {len(raw)} bytes where a key is 32. "
            "It is not being replaced; restore the file or move it aside deliberately.")
    _warn_if_readable_by_others(path)
    return X25519PrivateKey.from_private_bytes(raw)


def write_key(path, private):
    """Lay the key down, owner only, without a window where it is wider.

    The mode is set on a fresh descriptor rather than with a chmod after the
    write: between an open and a chmod the file exists at the process umask,
    and a key that is world readable for a millisecond has been world
    readable.
    """
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = private.private_bytes_raw()
    handle = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, KEY_MODE)
    try:
        os.write(handle, base64.b64encode(raw) + b"\n")
    finally:
        os.close(handle)
    os.chmod(path, KEY_MODE)


def _warn_if_readable_by_others(path):
    """Say it, do not refuse over it.

    A key anybody can read is a real finding, and it is also the shape a
    bind mount or a restored archive can arrive in through nobody's fault.
    Refusing would turn a warning into an outage; the operator gets the line
    and decides.
    """
    try:
        mode = path.stat().st_mode & 0o777
    except OSError:
        return
    if mode & 0o077:
        log.warning("the key file %s is mode %o, wider than owner only", path, mode)


def public_of(private):
    """The raw public half, the thing a device pins."""
    return private.public_key().public_bytes_raw()


def standing(store, path=None):
    """What the key on disk is, against what the store remembers.

    Returns `(verdict, private_or_None, fingerprint_or_None)`. Reads only;
    nothing here writes a key or a memory, so a command that wants to SHOW
    the situation cannot change it by asking.
    """
    path = pathlib.Path(path) if path is not None else key_path()
    private = read_key(path)
    found = fingerprint(public_of(private)) if private is not None else None
    return decide(found, store.remembered_key()), private, found


def open_identity(store, path=None):
    """The key this service serves with, or a refusal.

    The one door the server uses. It writes in exactly two cases, both of
    which the pure decision has already called permissible: it makes the
    first key, and it records a key the store has never seen. It never
    replaces one.
    """
    path = pathlib.Path(path) if path is not None else key_path()
    verdict, private, found = standing(store, path)

    if not verdict.may_serve:
        raise RefusedToServe(verdict.reason)

    if verdict.name == "make_one":
        private = X25519PrivateKey.generate()
        write_key(path, private)
        found = fingerprint(public_of(private))
        log.warning("a static key was made: %s (every device that connects will pin it)", found)

    if verdict.name in ("make_one", "remember"):
        store.remember_key(found)
        log.warning("this service now answers as %s", found)
    else:
        log.info("serving as %s", found)

    return private, found


def adopt(store, path=None):
    """Say that the key which is there is the right one now.

    The operator's way to WANT the change after moving a service or
    restoring onto a machine that has its own key. It records; it never
    makes a key, because "adopt" that quietly generates would be the silent
    replacement this whole file is against.
    """
    path = pathlib.Path(path) if path is not None else key_path()
    private = read_key(path)
    if private is None:
        raise RefusedToServe(
            f"there is no key file at {path} to adopt. To make a new identity, "
            "which every device will have to accept again, run `key --new`.")
    found = fingerprint(public_of(private))
    was = store.remembered_key()
    store.remember_key(found)
    return found, was


def make_new(store, path=None):
    """Make a new identity, deliberately, and say what it costs.

    The old key file is kept beside the new one rather than overwritten. A
    rotation done by mistake is otherwise unrecoverable, and the one thing
    worse than an identity change is an identity change that cannot be
    undone.

    **The kept file is DATED, and the first version of this was wrong.** It
    used one fixed name, so a second rotation overwrote what the first had
    saved: after three rotations two files existed and the first two keys
    were gone, while the line printed underneath still said "not
    overwritten". A promise that holds only for the most recent caller is
    not a promise. The dated name is the shape `account-remove` already
    uses for the same job, and the write refuses rather than replaces if a
    name ever did collide.
    """
    path = pathlib.Path(path) if path is not None else key_path()
    was = store.remembered_key()
    if path.exists():
        stamp = time.strftime("%Y%m%d-%H%M%S")
        kept = path.with_name(f"{path.name}.was-{stamp}")
        # Within one second a second rotation would land on the same name,
        # and this function exists to not lose keys. Count up instead.
        serial = 2
        while kept.exists():
            kept = path.with_name(f"{path.name}.was-{stamp}-{serial}")
            serial += 1
        os.replace(path, kept)
    else:
        kept = None
    private = X25519PrivateKey.generate()
    write_key(path, private)
    found = fingerprint(public_of(private))
    store.remember_key(found)
    return found, was, kept
