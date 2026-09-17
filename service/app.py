# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The service as a bare ASGI application.

No web framework. The whole interface is "fetch bytes, put bytes, announce
a change", and against the written selection criteria a framework buys
routing sugar for that and little else: fewest transitive dependencies
wins outright, ASGI is the standard interface that keeps the application
server swappable, and a dispatch table this small is easier to read than
anyone's routing decorators. Flask was never a candidate — WSGI cannot
carry a WebSocket, and announcing a change is an invariant, not a wish.

It also means this file is testable with nothing installed at all: an ASGI
application is a callable taking three arguments, so the tests drive it
directly. A framework would have brought a test client and the dependency
that carries it.

Every request names itself with a bearer token and the OWNER comes from
the store's answer to it, never from anything the caller wrote. The one
exception is the path that hands out a token in the first place, and it
gives the same answer for a wrong secret and for an account that is not
here.

The token rides in a header and never in the address. A query string is
copied into every access log and proxy record between the device and the
service, so an address that carries a credential leaks it to places
nobody thought about. The WebSocket handshake takes headers too — a
native client sets them, and it is only browsers that cannot, which is
why so many services end up putting tokens in URLs.
"""

import json
import logging
import base64
import os
from urllib.parse import unquote

from core.authorization import DENIED, caller_for
from core.frames import NotAFrame, pack, request_of, unpack
from core.paths import parse_document_path, parse_listing_path, valid_scope

from .identity import RefusedToServe, key_path, open_identity, public_of
from .sessions import SessionStore, UnknownSession
from .store import Store

# Every refusal and every failed sign-in leaves a line here, with the
# source address, the owner, the document and the sentence. Never a token,
# never a secret, never a body: a fence in the suite reads this file and
# holds that. Without the lines the case the service is built for (a device
# writes empty at start-up, is refused, and the client swallows the 409) is
# invisible to the operator for as long as it lasts.
log = logging.getLogger("maer.sync")

# Host protection, NOT a data ceiling: a body larger than this is refused
# with 413 before it is held in memory. The shrink gate protects the data;
# this protects the machine from a broken carrier or a token holder that
# sends a gigabyte, tenfold over the kept versions. Generous on purpose and
# set per deployment; it is not the iCloud megabyte in another coat.
DEFAULT_MAX_BODY_BYTES = 64 * 1024 * 1024


class BodyTooLarge(Exception):
    pass


class BodyIncomplete(Exception):
    """The client went away mid-body, or sent fewer bytes than it
    announced. Measured on 15.09.2026: a write of 192 KiB (three 64 KiB
    reads) was stored as a complete document because a disconnect carries
    no `more_body`, and the loop read that as "the body ends here". The
    truncated document was unreadable, and the device that wrote it could
    not get past it: it kept naming the version it had read, the service
    kept answering 412, and for hours no listening position was stored."""
    pass


class Listeners:
    """Who is waiting to hear that something changed.

    The announcement is what makes a service of our own worth having: the
    takeover has to reach a locked, playing device, and a silent push
    measurably does not (median 1050 ms in the foreground; on the locked
    device it did not arrive at all). Every accepted write tells everyone
    listening for that owner.

    A listener that has gone away is dropped rather than retried: an
    announcement is an ACCELERATION, never the only source of truth, and a
    device that missed one catches up at its next occasion. Holding a
    queue for an absent device would turn a nicety into a liability.
    """

    def __init__(self):
        self._by_owner = {}

    def add(self, owner, send):
        self._by_owner.setdefault(owner, []).append(send)

    def remove(self, owner, send):
        waiting = self._by_owner.get(owner)
        if waiting and send in waiting:
            waiting.remove(send)
        if waiting is not None and not waiting:
            del self._by_owner[owner]

    def count(self, owner):
        return len(self._by_owner.get(owner, []))

    async def announce(self, owner, message):
        text = json.dumps(message)
        gone = []
        for send in list(self._by_owner.get(owner, [])):
            try:
                await send({"type": "websocket.send", "text": text})
            except Exception:
                # An announcement is best effort by design; a dead socket
                # is not an error the writer needs to hear about.
                gone.append(send)
        for send in gone:
            self.remove(owner, send)


#: Whether the five older ways still answer OUTSIDE a session. On by default,
#: because devices that speak only the plain ways keep syncing until they are
#: updated; an operator turns it off once every device speaks inside, and
#: from then on a plain request is told to come back inside a session. The
#: two paths of the session itself and the health route are never behind it.
PLAIN_WAYS = "MAER_SYNC_PLAIN_WAYS"

#: The marker an INNER request carries in its scope, so the plain ways gate
#: knows it came through a session and not over the wire in the clear.
INSIDE = "maer.inside"


class App:
    def __init__(self, store, max_body_bytes=None, identity=None, sessions=None, plain_ways=None):
        self.store = store
        self.plain_ways = plain_ways if plain_ways is not None else (
            os.environ.get(PLAIN_WAYS, "on").lower() not in ("off", "0", "no", "false"))
        # The service's static private key, and the sessions built from it.
        # BOTH optional, and the two paths that need them answer 503 when
        # they are absent rather than pretending: an application assembled
        # without an identity is a real thing (every witness that only wants
        # documents builds one), and a service that answered a handshake
        # without a key would be answering as nobody.
        self.identity = identity
        self.sessions = sessions if sessions is not None else SessionStore()
        self.listeners = Listeners()
        self.max_body_bytes = max_body_bytes or int(
            os.environ.get("MAER_SYNC_MAX_BODY_BYTES", DEFAULT_MAX_BODY_BYTES))

    def _caller(self, scope):
        # A header that is not UTF-8 is a bad request, not a crash of the
        # worker: replaced characters make an unknown token, and that is
        # a 401 like any other.
        headers = {k.decode("utf-8", "replace").lower(): v.decode("utf-8", "replace")
                   for k, v in scope.get("headers", [])}
        return caller_for(headers.get("authorization"), self.store.owner_of_token), headers

    async def __call__(self, scope, receive, send):
        if scope["type"] == "http":
            await self._http(scope, receive, send)
        elif scope["type"] == "websocket":
            await self._websocket(scope, receive, send)
        elif scope["type"] == "lifespan":
            await self._lifespan(receive, send)

    # MARK: - HTTP

    async def _http(self, scope, receive, send):
        path = scope["path"]
        method = scope["method"]

        # The only path that answers without a token, because it is the one
        # that hands tokens out. It says nothing about the store.
        if path == "/v1/health":
            return await _json(send, 200, {"ok": True})
        if path == "/v1/key":
            if method != "GET":
                return await _json(send, 405, {"reason": f"{method} is not what this path is for"})
            return await self._key(send)
        if path == "/v1/session":
            if method != "POST":
                return await _json(send, 405, {"reason": f"{method} is not what this path is for"})
            return await self._session(scope, receive, send)
        # The two paths of the encrypted session, and neither asks for a
        # token. That is the point of the pattern: the caller proves nothing
        # until it is INSIDE the session, which is what lets a sign in be
        # protected, and a sign in happens before there is any token at all.
        if not self.plain_ways and not scope.get(INSIDE):
            # Told, not silently refused: a device that still speaks in the
            # clear must learn that this service only listens inside.
            return await _json(send, 426, {"reason": "this service speaks only inside a session"})
        if path == "/v1/token":
            if method != "POST":
                return await _json(send, 405, {"reason": f"{method} is not what this path is for"})
            return await self._token(scope, receive, send)

        caller, headers = self._caller(scope)
        if not caller.authorized:
            log.warning("unauthorized from=%s method=%s path=%s", _client(scope), method, path)
            return await _json(send, 401, {"reason": caller.reason},
                               [(b"www-authenticate", b"Bearer")])

        listed = parse_listing_path(path)
        if listed is not None:
            if method != "GET":
                return await _json(send, 405, {"reason": f"{method} is not what this path is for"})
            return await self._list(send, caller.owner, listed)

        kind = parse_document_path(path)
        if kind is None:
            return await _json(send, 404, {"reason": "no such path"})

        query = _query(scope.get("query_string", b""))
        query_scope = query.get("scope", "")
        if not valid_scope(query_scope):
            return await _json(send, 400, {"reason": "the scope is not a scope"})

        if method == "GET":
            return await self._get(send, caller.owner, kind, query_scope)
        if method == "PUT":
            return await self._put(scope, receive, send, caller.owner, headers, kind, query_scope)
        return await _json(send, 405, {"reason": f"{method} is not what this path is for"})

    async def _key(self, send):
        """The service's static PUBLIC key, so a device can pin it.

        No token, and it could not sensibly ask for one: this is the first
        thing a device fetches, before it can speak to anything privately.

        **This is the trust on first use channel, and it says so.** Over a
        plain connection whoever sits in the middle can hand out their own
        key here. That is the migration the operator chose: trust the first
        time, hold on afterwards. The holding is what turns a later swap into
        an event, and it is the device's half.
        """
        if self.identity is None:
            return await _json(send, 503, {"reason": "this service has no static key"})
        private, printed = self.identity
        return await _json(send, 200, {
            "key": base64.b64encode(public_of(private)).decode("ascii"),
            "fingerprint": printed})

    async def _session(self, scope, receive, send):
        """Without a session header: answer a handshake and hand back the
        name of the session. With one: carry a sealed request in that
        session.

        The body is the caller's first Noise message, raw bytes, and the
        answer is the service's reply, raw bytes, with the name in a header.
        Nothing here is JSON: these are two fixed shape messages of a
        protocol, and wrapping them in a document would only invite somebody
        to add a field to it.
        """
        if self.identity is None:
            return await _json(send, 503, {"reason": "this service has no static key"})
        try:
            body = await _read_body(receive, self.max_body_bytes)
        except BodyIncomplete:
            return await _json(send, 400, {"reason": "the handshake broke off mid request"})
        except BodyTooLarge:
            return await _json(send, 413, {"reason": "a handshake is not this large"})

        _, headers = self._caller(scope)
        name = headers.get("x-maer-session")
        if name is not None:
            return await self._sealed(scope, send, name, body)

        private, _ = self.identity
        try:
            name, reply, _ = self.sessions.open(body, private)
        except Exception:
            # Every way of not being a handshake is one answer. Saying which
            # would tell somebody probing exactly how far they got.
            log.warning("handshake refused from=%s", _client(scope))
            return await _json(send, 400, {"reason": "this is not a handshake"})
        return await _bytes(send, 200, reply, [(b"x-maer-session", name.encode("ascii"))])

    async def _sealed(self, scope, send, name, body):
        """One request inside a session: unseal, run it through the same
        doors every request goes through, seal the reply.

        **The inside is the ordinary wire.** The frame carries a method, a
        target, headers and a body, so the token rides in an `Authorization`
        header exactly as the contract says, only nobody between the device
        and this process can read it. Nothing about the five older ways is
        learned twice.

        A session that has ended is a 410 and nothing else: the device's
        answer is one new handshake and one retry, never a sentence to a
        person. A message that does not open is a 400, and the session
        survives it: the cipher's counter moves only on success, so noise
        sent under a stolen name costs its owner nothing.
        """
        try:
            plaintext = self.sessions.unseal(name, body)
        except UnknownSession:
            return await _json(send, 410, {"reason": "this session has ended"})
        except Exception:
            log.warning("sealed message refused from=%s", _client(scope))
            return await _json(send, 400, {"reason": "this is not a sealed message"})
        try:
            head, inner_body = unpack(plaintext)
            method, path, query, inner_headers = request_of(head)
        except NotAFrame:
            return await _json(send, 400, {"reason": "this is not a request"})
        if path in ("/v1/session", "/v1/key"):
            # A session does not nest, and the key is what the session was
            # built from; asking for either from inside says nothing.
            return await _json(send, 400, {"reason": "this path is not answered inside a session"})

        status, reply_headers, reply_body = await self._inner(scope, method, path, query,
                                                             inner_headers, inner_body)
        # Proven means: this session carried a request that proved WHO was
        # asking. A token that was accepted proves it, and so does a sign in
        # that the store said yes to. A 401 proves nothing.
        if status != 401 and ("authorization" in inner_headers or path == "/v1/token"):
            try:
                self.sessions.prove(name)
            except UnknownSession:
                return await _json(send, 410, {"reason": "this session has ended"})
        try:
            sealed = self.sessions.seal(name, pack({"status": status, "headers": reply_headers}, reply_body))
        except UnknownSession:
            return await _json(send, 410, {"reason": "this session has ended"})
        return await _bytes(send, 200, sealed)

    async def _inner(self, outer, method, path, query, headers, body):
        """Run one request through `_http` as if it had arrived on its own,
        and collect what it answered. The client address is the outer one,
        so log lines name the device and not the session."""
        wire_headers = [(k.encode("utf-8"), v.encode("utf-8")) for k, v in headers.items()]
        wire_headers.append((b"content-length", str(len(body)).encode("ascii")))
        scope = {"type": "http", "method": method, "path": path, "query_string": query,
                 "headers": wire_headers, "client": outer.get("client"), INSIDE: True}
        handed = [False]
        sent = []

        async def receive():
            if handed[0]:
                return {"type": "http.disconnect"}
            handed[0] = True
            return {"type": "http.request", "body": body, "more_body": False}

        async def send(message):
            sent.append(message)

        await self._http(scope, receive, send)
        start = next(m for m in sent if m["type"] == "http.response.start")
        reply_body = b"".join(m.get("body", b"") for m in sent if m["type"] == "http.response.body")
        reply_headers = {k.decode("utf-8"): v.decode("utf-8") for k, v in start["headers"]
                         if k.lower() != b"content-length"}
        return start["status"], reply_headers, reply_body

    async def _token(self, scope, receive, send):
        try:
            body = await _read_body(receive, self.max_body_bytes)
        except BodyIncomplete:
            # A sign-in that breaks off mid-request is not a sign-in.
            log.warning("incomplete sign-in from=%s", _client(scope))
            return await _json(send, 400, {"reason": "the request ended before it was whole"})
        except BodyTooLarge:
            log.warning("sign-in too large from=%s", _client(scope))
            return await _json(send, 413, {"reason": "the request is larger than this host takes"})
        try:
            asked = json.loads(body)
            owner, secret = asked["owner"], asked["secret"]
            device = asked.get("device", "")
        except (ValueError, KeyError, TypeError):
            return await _json(send, 400, {"reason": "this path wants an owner and a secret"})

        token = self.store.issue_token(owner, secret, device)
        if token is None:
            # The same answer a wrong secret gets, and the same one an
            # account that is not here gets. Anything else makes this a
            # list of who has an account. The log line carries the source
            # address so the operator can count tries per address.
            log.warning("sign-in refused from=%s owner=%s", _client(scope), owner)
            return await _json(send, 401, {"reason": DENIED})
        log.info("sign-in from=%s owner=%s device=%s", _client(scope), owner, device)
        return await _json(send, 200, {"token": token})

    async def _list(self, send, owner, kind):
        documents = [{"scope": scope, "version": etag, "format": fmt}
                     for scope, etag, fmt in self.store.scopes(owner, kind)]
        await _json(send, 200, {"kind": kind, "documents": documents})

    async def _get(self, send, owner, kind, query_scope):
        held = self.store.current(owner, kind, query_scope)
        if held is None:
            return await _json(send, 404, {"reason": "no such document"})
        body, fmt, etag = held
        await _bytes(send, 200, body, [
            (b"etag", etag.encode()),
            (b"x-maer-format", str(fmt).encode()),
        ])

    async def _put(self, scope, receive, send, owner, headers, kind, query_scope):
        raw_format = headers.get("x-maer-format")
        try:
            writer_format = int(raw_format)
        except (TypeError, ValueError):
            # Guessing a format would let every client that forgot the
            # header overwrite documents written by newer ones.
            return await _json(send, 400, {"reason": "the write named no format"})

        try:
            announced = int(headers["content-length"]) if "content-length" in headers else None
        except (TypeError, ValueError):
            announced = None

        try:
            body = await _read_body(receive, self.max_body_bytes, announced)
        except BodyIncomplete:
            # Nothing is stored: the held version stays whole. A write that
            # breaks off is not a smaller document, it is half of one, and
            # half a document takes the whole thing away from every reader.
            log.warning("incomplete write from=%s owner=%s kind=%s scope=%s announced=%s",
                        _client(scope), owner, kind, query_scope, announced)
            return await _json(send, 400, {"reason": "the write ended before the body did"})
        except BodyTooLarge:
            log.warning("write too large from=%s owner=%s kind=%s scope=%s limit=%d",
                        _client(scope), owner, kind, query_scope, self.max_body_bytes)
            return await _json(send, 413, {"reason": "the document is larger than this host takes"})
        # What the service cannot read never replaces what it holds. Every
        # format this protocol defines is JSON; a body that does not parse
        # is either truncated or not ours, and storing it takes the
        # document away from every reader, including the writer itself.
        # An EMPTY body is not a broken document: it is the "empty state"
        # the store answers with its own 409 and its own sentence. Checking
        # it here would take that better answer away.
        try:
            if body:
                json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            size = len(body)
            log.warning("unreadable write from=%s owner=%s kind=%s scope=%s bytes=%d",
                        _client(scope), owner, kind, query_scope, size)
            return await _json(send, 400, {"reason": "the write is not readable as a document"})

        decision, etag = self.store.write(
            owner, kind, body, writer_format, scope=query_scope,
            if_match=headers.get("if-match"),
            declared_shrink=headers.get("x-maer-declared-shrink") == "1")

        if not decision.accepted:
            held = self.store.current(owner, kind, query_scope)
            extra = [(b"etag", held[2].encode())] if held else []
            log.warning("refused from=%s owner=%s kind=%s scope=%s status=%d reason=%s",
                        _client(scope), owner, kind, query_scope, decision.status, decision.reason)
            return await _json(send, decision.status, {"reason": decision.reason}, extra)

        await self.listeners.announce(owner, {
            "kind": kind, "scope": query_scope, "version": etag,
        })
        await _json(send, 200, {"reason": decision.reason, "version": etag},
                    [(b"etag", etag.encode())])

    # MARK: - WebSocket

    async def _websocket(self, scope, receive, send):
        if scope["path"] != "/v1/changes":
            await send({"type": "websocket.close", "code": 4004})
            return

        caller, headers = self._caller(scope)
        name = headers.get("x-maer-session")
        if name is not None:
            return await self._sealed_socket(scope, receive, send, name)
        if not self.plain_ways:
            await send({"type": "websocket.close", "code": 4426})
            return

        # The handshake carries the token in a header like every other
        # request. An unauthorized socket is closed before it is accepted:
        # accepting first and closing after would put a stranger on the
        # announcement list for as long as that takes.
        if not caller.authorized:
            log.warning("unauthorized socket from=%s", _client(scope))
            await send({"type": "websocket.close", "code": 4401})
            return

        message = await receive()
        if message["type"] != "websocket.connect":
            return
        await send({"type": "websocket.accept"})
        self.listeners.add(caller.owner, send)
        try:
            while True:
                message = await receive()
                if message["type"] == "websocket.disconnect":
                    break
                # The client has nothing to say on this socket. Anything it
                # sends is ignored rather than parsed: a channel that only
                # ever announces cannot be talked into doing something else.
        finally:
            self.listeners.remove(caller.owner, send)

    async def _sealed_socket(self, scope, receive, send, name):
        """The changes socket inside a session.

        The token cannot ride in the upgrade request, because that request
        is the one thing on this path that is not sealed. So the socket is
        accepted first, and the FIRST frame the device sends is a sealed
        request head carrying its `Authorization`; only then is the device
        put on the list. A stranger with a made up name is turned away
        before the accept, like an unauthorized socket in the clear. Every
        announcement afterwards goes out sealed, as bytes.

        **A socket wants a session of its own.** A session's counters run in
        the order messages are sealed, and a stream of announcements
        interleaved with the replies of a request session would arrive out
        of that order. The device opens two handshakes, one per direction
        of use, and the store's capacity was chosen with that in mind.
        """
        if self.identity is None or not self.sessions.is_live(name):
            await send({"type": "websocket.close", "code": 4410})
            return
        message = await receive()
        if message["type"] != "websocket.connect":
            return
        await send({"type": "websocket.accept"})

        first = await receive()
        try:
            if first["type"] != "websocket.receive" or first.get("bytes") is None:
                raise NotAFrame("the first frame is not sealed bytes")
            head, _ = unpack(self.sessions.unseal(name, first["bytes"]))
            _, _, _, inner_headers = request_of(head)
        except UnknownSession:
            await send({"type": "websocket.close", "code": 4410})
            return
        except Exception:
            log.warning("sealed socket refused from=%s", _client(scope))
            await send({"type": "websocket.close", "code": 4400})
            return
        caller = caller_for(inner_headers.get("authorization"), self.store.owner_of_token)
        if not caller.authorized:
            log.warning("unauthorized sealed socket from=%s", _client(scope))
            await send({"type": "websocket.close", "code": 4401})
            return
        self.sessions.prove(name)

        async def sealed_send(announcement):
            # The listeners speak text; this socket speaks sealed bytes.
            text = announcement.get("text", "")
            await send({"type": "websocket.send",
                        "bytes": self.sessions.seal(name, text.encode("utf-8"))})

        self.listeners.add(caller.owner, sealed_send)
        try:
            while True:
                message = await receive()
                if message["type"] == "websocket.disconnect":
                    break
        finally:
            self.listeners.remove(caller.owner, sealed_send)

    # MARK: - Lifespan

    async def _lifespan(self, receive, send):
        while True:
            message = await receive()
            if message["type"] == "lifespan.startup":
                await send({"type": "lifespan.startup.complete"})
            elif message["type"] == "lifespan.shutdown":
                self.store.close()
                await send({"type": "lifespan.shutdown.complete"})
                return


# MARK: - The small amount of HTTP a byte store needs


async def _read_body(receive, limit, announced=None):
    """The whole body, or `BodyTooLarge` as soon as it passes the limit,
    so an oversized request never sits in memory in full.

    A request that ENDS early is not a body: a disconnect raises
    `BodyIncomplete`, and so does a body shorter than the length the
    client announced. Both used to read as "done" and were stored."""
    body = b""
    while True:
        message = await receive()
        if message.get("type") == "http.disconnect":
            raise BodyIncomplete()
        body += message.get("body", b"")
        if len(body) > limit:
            raise BodyTooLarge()
        if not message.get("more_body"):
            if announced is not None and len(body) != announced:
                raise BodyIncomplete()
            return body


def _query(raw):
    """The query as a dict, values URL-decoded: a client that encodes the
    colon in a device id must find its own document again."""
    out = {}
    for pair in raw.decode("utf-8", "replace").split("&"):
        if "=" in pair:
            key, value = pair.split("=", 1)
            out[key] = unquote(value)
    return out


def _client(scope):
    """The source address for the log line, or a dash."""
    client = scope.get("client")
    if not client:
        return "-"
    return f"{client[0]}:{client[1]}"


async def _bytes(send, status, body, headers=()):
    await send({"type": "http.response.start", "status": status,
                "headers": [(b"content-type", b"application/octet-stream"),
                            (b"content-length", str(len(body)).encode())] + list(headers)})
    await send({"type": "http.response.body", "body": body})


async def _json(send, status, payload, headers=()):
    body = json.dumps(payload).encode()
    await send({"type": "http.response.start", "status": status,
                "headers": [(b"content-type", b"application/json"),
                            (b"content-length", str(len(body)).encode())] + list(headers)})
    await send({"type": "http.response.body", "body": body})


def make(path=None):
    """The application, with the store it serves and the key it answers as.

    The identity is opened HERE and not inside `App`, for the same reason
    `serve()` opens it before binding a port: making one is a decision with a
    consequence for every device, so it belongs where an application is
    assembled on purpose, not where one is constructed by a witness that only
    wanted somewhere to put a document.

    A refusal is not swallowed. It reaches the caller, and `serve()` turns it
    into an exit with the reason; an application that cannot say who it is
    must not come up.
    """
    where = path or os.environ.get("MAER_SYNC_DB", "maer-sync.sqlite")
    store = Store(where)
    identity = open_identity(store, key_path(where))
    return App(store, identity=identity)
