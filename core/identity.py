# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Who this service IS, written so a person can compare it by eye.

The handshake in `core/handshake.py` gives the pattern `NK` its meaning: the
device knows the responder's static public key before it says anything, and
whoever does not hold the private half cannot answer. That only protects
anybody if the key STAYS THE SAME, and if a change is loud.

So there are two jobs here, both pure, both free of files and of the
cryptography package:

1. **A fingerprint** short enough to read out loud and compare across a room.
2. **The decision** about a key that was found against a key that was
   remembered, with its reason. The decision is pure so that the refusals can
   be tested without laying down files, and so that the reason a service
   refuses to start is the same sentence in the test and in the terminal.

**Why a fingerprint and not the key itself.** The raw key is 32 bytes and
reads as 43 characters of base64 with no grouping. Nobody compares that
honestly; they compare the first four characters and the last four, which is
exactly the comparison an attacker builds for. Twenty four characters in six
groups is the form this product already uses for the device link code, and
people have shown they will read it.

**The alphabet is Crockford's base32**, which is the same letters the link
code uses plus the digits. `I`, `L`, `O` and `U` are not in it: the first
three are the shapes people confuse with digits, and confusing two
fingerprints is the whole failure mode this form exists to prevent. It is a
published alphabet rather than one of ours, so the app's half can decode it
without a second table to keep in step.

**Fifteen bytes, not the whole digest.** 120 bits against a second preimage,
which is far past anything anyone can search, and 15 bytes is exactly 24
characters with nothing left over. A partial group at the end would be a
place for an off by one to hide.
"""

import hashlib

#: Crockford's base32. Not ours, on purpose: both ends decode it from a
#: published table instead of from each other.
ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"

#: Bytes of the digest that make the fingerprint. Exactly divisible into
#: five bit groups, so the last character is a whole one.
FINGERPRINT_BYTES = 15

#: Characters per group in the written form, as in the device link code.
GROUP = 4


def fingerprint(public_key_bytes):
    """The written fingerprint of a raw public key.

    `XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`, 24 characters. A key of the wrong
    length is refused rather than fingerprinted: every caller here has a
    32 byte X25519 key, and a shorter one means something was truncated on
    the way, which is precisely when a fingerprint must not look fine.
    """
    if len(public_key_bytes) != 32:
        raise ValueError(f"a static key is 32 bytes, this one is {len(public_key_bytes)}")
    digest = hashlib.sha256(public_key_bytes).digest()[:FINGERPRINT_BYTES]
    bits = int.from_bytes(digest, "big")
    letters = [ALPHABET[(bits >> shift) & 0x1F]
               for shift in range(FINGERPRINT_BYTES * 8 - 5, -1, -5)]
    written = "".join(letters)
    return "-".join(written[i:i + GROUP] for i in range(0, len(written), GROUP))


class Standing:
    """What to do about the key that is here, and why.

    `may_serve` is the only thing the caller branches on, and `reason` is
    the sentence the operator reads. They travel together so that a refusal
    can never reach a terminal without saying what to do about it.
    """

    __slots__ = ("name", "may_serve", "reason")

    def __init__(self, name, may_serve, reason):
        self.name = name
        self.may_serve = may_serve
        self.reason = reason

    def __repr__(self):
        return f"Standing({self.name!r}, may_serve={self.may_serve!r})"


#: What the operator types to say a change is wanted. Named once so that the
#: refusals and the help cannot drift apart.
ADOPT = "python3 -m service key --adopt"
MAKE_NEW = "python3 -m service key --new"


def decide(found, remembered):
    """The standing of the key on disk against the key the store remembers.

    Both arguments are fingerprints or None. Five cases, and the two that
    refuse are the reason this module exists: **a static key that changes
    silently is not a static key.** Every device that has pinned the old one
    would fail its handshake with no way to tell a rotation from somebody
    standing in the middle, which is the one thing pinning is supposed to
    answer.

    The case that is deliberately permissive is `remember`: a key is here and
    the store has never recorded one. That is a store restored from a backup
    onto the machine that still holds its own key, and refusing it would
    break the ordinary recovery it is meant to allow. It is safe because the
    store's memory is the only thing being written, never the key.
    """
    if found is None and remembered is None:
        return Standing("make_one", True,
                        "no static key here and none remembered, so this service is new")
    if found is not None and remembered is None:
        return Standing("remember", True,
                        f"the key {found} is here and this store has never recorded one, "
                        "so it is recorded now")
    if found == remembered:
        return Standing("carry_on", True, f"the key {found} is the one this store expects")
    if found is None:
        return Standing("gone", False,
                        f"this store expects the key {remembered} and there is no key file. "
                        "Making a new one would silently break every device that pinned the "
                        f"old one. Put the key file back, or run `{MAKE_NEW}` to say that a "
                        "new identity is wanted and every device should ask again.")
    return Standing("changed", False,
                    f"this store expects the key {remembered} but the key file is {found}. "
                    "Serving anyway would hand a different identity to devices that pinned "
                    f"the first. Put the expected key back, or run `{ADOPT}` to say that "
                    "this key is the right one now.")
