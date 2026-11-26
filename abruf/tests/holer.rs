// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The real `Holer` over `ureq`, against a local server on 127.0.0.1 in a
//! thread of the witness: no request leaves the host. The witnesses use a
//! plain-http holer because the local server speaks no TLS; the program
//! uses `UreqHoler::new()`, which allows https only and always verifies
//! certificates.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use maer_sync_abruf::netz::{Anfrage, HolFehler, Holer, USER_AGENT};
use maer_sync_abruf::ureq_holer::{mapped, UreqHoler};

/// Serve one connection with `answer` after reading the request head; the
/// head comes back over the channel.
fn serve_once(answer: Vec<u8>, delay: Duration) -> (String, mpsc::Receiver<String>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", l.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Ok((s, _)) = l.accept() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let _ = tx.send(head);
            std::thread::sleep(delay);
            let mut s = s;
            let _ = s.write_all(&answer);
            let _ = s.flush();
        }
    });
    (addr, rx)
}

fn answer(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len()).into_bytes();
    for h in headers {
        out.extend_from_slice(h.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

fn ask(url: &str, max_bytes: usize, timeout_s: u64) -> Anfrage {
    Anfrage { url: url.into(), user_agent: USER_AGENT.into(), max_bytes, timeout_s }
}

#[test]
fn the_program_holer_verifies_and_allows_https_only() {
    let h = UreqHoler::new();
    assert!(h.verifies_certificates());
    assert!(h.https_only());
    // The witnesses' holer differs only in the scheme, never in verification.
    assert!(UreqHoler::plain_http_for_witnesses().verifies_certificates());
}

#[test]
fn no_switch_turns_verification_off() {
    let src = include_str!("../src/ureq_holer.rs");
    // Reading the setting is allowed (`disable_verification()`); setting it is not.
    for (i, _) in src.match_indices("disable_verification(") {
        let after = &src[i + "disable_verification(".len()..];
        assert!(after.starts_with(')'), "the holer sets disable_verification: {}", &after[..after.len().min(20)]);
    }
    assert!(!src.contains("danger"), "the holer names a dangerous option");
}

#[test]
fn a_request_names_the_client_and_reads_the_body() {
    let (url, head) = serve_once(answer("200 OK", &[], b"{\"ok\":true}"), Duration::ZERO);
    let a = UreqHoler::plain_http_for_witnesses().hole(&ask(&format!("{url}/x?q=a%20b"), 1024, 5)).unwrap();
    assert_eq!(a.status, 200);
    assert_eq!(a.koerper, b"{\"ok\":true}");
    let head = head.recv_timeout(Duration::from_secs(5)).unwrap().to_lowercase();
    assert!(head.starts_with("get /x?q=a%20b http/1.1"), "{head}");
    assert!(head.contains(&format!("user-agent: {}", USER_AGENT.to_lowercase())), "{head}");
}

#[test]
fn a_redirect_is_returned_not_followed() {
    let (url, _) = serve_once(answer("301 Moved Permanently", &["Location: http://example.org/elsewhere"], b""), Duration::ZERO);
    let a = UreqHoler::plain_http_for_witnesses().hole(&ask(&url, 1024, 5)).unwrap();
    assert_eq!(a.status, 301);
    assert_eq!(a.location.as_deref(), Some("http://example.org/elsewhere"));
}

#[test]
fn an_error_status_is_an_answer_with_its_wait() {
    let (url, _) = serve_once(answer("429 Too Many Requests", &["Retry-After: 120"], b"slow down"), Duration::ZERO);
    let a = UreqHoler::plain_http_for_witnesses().hole(&ask(&url, 1024, 5)).unwrap();
    assert_eq!((a.status, a.retry_after_s), (429, Some(120)));
    let (url, _) = serve_once(answer("500 Internal Server Error", &[], b""), Duration::ZERO);
    assert_eq!(UreqHoler::plain_http_for_witnesses().hole(&ask(&url, 1024, 5)).unwrap().status, 500);
}

#[test]
fn reading_stops_at_the_bound() {
    let big = vec![b'x'; 2 * 1024 * 1024];
    let (url, _) = serve_once(answer("200 OK", &[], &big), Duration::ZERO);
    assert_eq!(UreqHoler::plain_http_for_witnesses().hole(&ask(&url, 1024, 5)), Err(HolFehler::ZuGross(1025)));
}

#[test]
fn a_slow_server_runs_into_the_time_limit() {
    let (url, _) = serve_once(answer("200 OK", &[], b"late"), Duration::from_secs(4));
    let started = std::time::Instant::now();
    assert_eq!(UreqHoler::plain_http_for_witnesses().hole(&ask(&url, 1024, 1)), Err(HolFehler::Zeit));
    assert!(started.elapsed() < Duration::from_secs(3), "the limit was not kept: {:?}", started.elapsed());
}

#[test]
fn the_program_holer_refuses_plain_http() {
    let (url, _) = serve_once(answer("200 OK", &[], b"x"), Duration::ZERO);
    assert!(UreqHoler::new().hole(&ask(&url, 1024, 5)).is_err());
    let _ = std::io::empty().read(&mut [0u8; 1]);
}

#[test]
fn a_name_that_does_not_resolve_is_host_not_found() {
    // Measured 26.11.2026 with this holer in the build container: a name
    // that does not resolve comes from the system as an io error with this
    // text, not as ureq's own HostNotFound (that one stands for an empty
    // answer). A refused connection stays a connection failure.
    let io = |kind, text: &str| ureq::Error::Io(std::io::Error::new(kind, text.to_string()));
    let unknown = io(std::io::ErrorKind::Other, "failed to lookup address information: Name does not resolve");
    assert_eq!(mapped(unknown, 10), HolFehler::WirtUnbekannt);
    assert_eq!(mapped(ureq::Error::HostNotFound, 10), HolFehler::WirtUnbekannt);
    let refused = io(std::io::ErrorKind::ConnectionRefused, "Connection refused (os error 111)");
    assert!(matches!(mapped(refused, 10), HolFehler::Verbindung(_)));
}
