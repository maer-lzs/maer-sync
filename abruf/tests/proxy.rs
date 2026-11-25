// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! ureq reads proxy variables of the environment by default
//! (`Proxy::try_from_env`: ALL_PROXY, HTTPS_PROXY, HTTP_PROXY). The fetch
//! program must not follow a proxy that a container's environment happens to
//! carry. A file of its own, because the environment is one per process.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use maer_sync_abruf::netz::{Anfrage, Holer, USER_AGENT};
use maer_sync_abruf::ureq_holer::UreqHoler;

#[test]
fn the_environment_proxy_is_ignored() {
    // A port nothing listens on: a holer that follows the proxy fails.
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_addr = format!("http://{}", dead.local_addr().unwrap());
    drop(dead);
    for v in ["ALL_PROXY", "HTTPS_PROXY", "HTTP_PROXY", "all_proxy", "https_proxy", "http_proxy"] {
        std::env::set_var(v, &dead_addr);
    }
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/x", l.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((s, _)) = l.accept() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" { line.clear(); }
            let mut s = s;
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        }
    });
    let mut h = UreqHoler::plain_http_for_witnesses();
    assert!(!h.follows_a_proxy(), "the holer took a proxy from the environment");
    let a = h.hole(&Anfrage { url, user_agent: USER_AGENT.into(), max_bytes: 1024, timeout_s: 5 }).unwrap();
    assert_eq!(a.koerper, b"ok");
    assert!(!UreqHoler::new().follows_a_proxy());
}
