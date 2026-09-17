# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""The frame inside a session, at its edges, with bytes alone."""

import unittest

from core.frames import LENGTH_BYTES, NotAFrame, pack, reply_of, request_of, unpack


class TheFrame(unittest.TestCase):
    def test_a_head_and_a_body_come_back_as_they_went_in(self):
        frame = pack({"method": "PUT", "target": "/v1/doc/stands?scope=", "headers": {"X-Maer-Format": "1"}},
                     b'{"k":"v"}')
        head, body = unpack(frame)
        self.assertEqual(body, b'{"k":"v"}')
        self.assertEqual(head["target"], "/v1/doc/stands?scope=")

    def test_the_body_is_raw_bytes_and_not_text(self):
        # A document is the bulk of the traffic; it travels as it is, not
        # as a string inside the head.
        raw = bytes(range(256))
        head, body = unpack(pack({"status": 200}, raw))
        self.assertEqual(body, raw)
        self.assertEqual(unpack(pack({"status": 200}))[1], b"")

    def test_the_head_is_deterministic(self):
        one = pack({"b": "2", "a": "1"})
        two = pack({"a": "1", "b": "2"})
        self.assertEqual(one, two, "the same head gives the same bytes, whatever order it was built in")

    def test_what_is_not_a_frame_is_one_error(self):
        for wrong in (b"", b"\x00", b"\x00\x05ab", b"\x00\x02[]", b"\x00\x03abc"):
            with self.assertRaises(NotAFrame, msg=repr(wrong)):
                unpack(wrong)

    def test_a_head_longer_than_the_length_field_is_refused_when_packing(self):
        with self.assertRaises(NotAFrame):
            pack({"x": "y" * (1 << (8 * LENGTH_BYTES))})


class TheRequestHead(unittest.TestCase):
    def test_a_request_head_becomes_the_parts_a_server_reads(self):
        method, path, query, headers = request_of(
            {"method": "GET", "target": "/v1/doc/claim?scope=dev%3Aa", "headers": {"Authorization": "Bearer t"}})
        self.assertEqual((method, path, query), ("GET", "/v1/doc/claim", b"scope=dev%3Aa"))
        self.assertEqual(headers, {"authorization": "Bearer t"}, "header names are lowercased, as off the wire")

    def test_a_target_without_a_query_has_an_empty_one(self):
        self.assertEqual(request_of({"method": "GET", "target": "/v1/docs/claim"})[2], b"")

    def test_what_is_not_a_request_is_refused(self):
        for wrong in ({"method": "DELETE", "target": "/v1/doc/x"},
                      {"method": "GET", "target": "v1/doc/x"},
                      {"method": "GET"},
                      {"method": "GET", "target": "/v1/doc/x", "headers": ["not", "a", "dict"]},
                      {"method": "GET", "target": "/v1/doc/x", "headers": {"a": 1}}):
            with self.assertRaises(NotAFrame, msg=repr(wrong)):
                request_of(wrong)


class TheReplyHead(unittest.TestCase):
    def test_a_reply_head_becomes_status_and_headers(self):
        self.assertEqual(reply_of({"status": 412, "headers": {"ETag": "7"}}), (412, {"etag": "7"}))

    def test_what_is_not_a_status_is_refused(self):
        for wrong in ({"status": "200"}, {"status": True}, {"status": 99}, {}):
            with self.assertRaises(NotAFrame, msg=repr(wrong)):
                reply_of(wrong)


if __name__ == "__main__":
    unittest.main()
