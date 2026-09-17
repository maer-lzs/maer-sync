# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The shape of what travels INSIDE a session, pure.

A sealed request is one ordinary request of this service, whole: method,
target, headers and body, so that the wire inside the session is exactly
the wire the contract already describes and nothing about the five older
ways has to be relearned. The same the other way: a sealed reply is one
ordinary reply, status, headers and body.

**The frame is a two byte length, a small JSON head, then the raw body.**
The body is not put through JSON on purpose: documents are the bulk of the
traffic, and base64 would make every one of them a third larger for no
gain. The head is small by nature and stays JSON because it is a handful
of named strings.

Nothing here encrypts. The session does that around the frame, and this
module does not know a session exists; that is what keeps it testable at
its edges with bytes alone.
"""

import json

#: The head's length travels in this many bytes, big endian.
LENGTH_BYTES = 2
LONGEST_HEAD = (1 << (8 * LENGTH_BYTES)) - 1

#: The methods a sealed request may carry: what the contract's paths take.
METHODS = ("GET", "PUT", "POST")


class NotAFrame(ValueError):
    """The bytes are not a frame, or the head is not the head of a request
    or a reply. ONE error for every way of being wrong: the caller's answer
    is the same, and telling them apart would describe the parser to
    somebody probing it."""


def pack(head, body=b""):
    """A frame from a head (a dict of plain values) and a body."""
    encoded = json.dumps(head, separators=(",", ":"), sort_keys=True).encode("utf-8")
    if len(encoded) > LONGEST_HEAD:
        raise NotAFrame("the head is longer than the length field can say")
    return len(encoded).to_bytes(LENGTH_BYTES, "big") + encoded + bytes(body)


def unpack(frame):
    """`(head, body)` out of a frame, or `NotAFrame`."""
    if len(frame) < LENGTH_BYTES:
        raise NotAFrame("shorter than a length")
    length = int.from_bytes(frame[:LENGTH_BYTES], "big")
    end = LENGTH_BYTES + length
    if len(frame) < end:
        raise NotAFrame("the head is cut short")
    try:
        head = json.loads(frame[LENGTH_BYTES:end].decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        raise NotAFrame("the head is not readable")
    if not isinstance(head, dict):
        raise NotAFrame("the head is not a head")
    return head, bytes(frame[end:])


def _headers_of(head):
    headers = head.get("headers", {})
    if not isinstance(headers, dict):
        raise NotAFrame("the headers are not headers")
    out = {}
    for key, value in headers.items():
        if not isinstance(key, str) or not isinstance(value, str):
            raise NotAFrame("a header is not a pair of strings")
        out[key.lower()] = value
    return out


def request_of(head):
    """`(method, path, query, headers)` out of a request head.

    The target is split at its first `?` the way a server splits a request
    line; the query comes back as BYTES, which is the shape the rest of the
    service reads it in. Header names come back lowercased, as they do off
    the wire.
    """
    method = head.get("method")
    target = head.get("target")
    if method not in METHODS:
        raise NotAFrame("not a method this service takes")
    if not isinstance(target, str) or not target.startswith("/"):
        raise NotAFrame("not a target")
    path, _, query = target.partition("?")
    return method, path, query.encode("utf-8"), _headers_of(head)


def reply_of(head):
    """`(status, headers)` out of a reply head."""
    status = head.get("status")
    if not isinstance(status, int) or isinstance(status, bool) or not 100 <= status <= 599:
        raise NotAFrame("not a status")
    return status, _headers_of(head)
