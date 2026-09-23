// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Befehl `health`, den der Container als Pruefung nimmt.
//!
//! Das Abbild traegt nur das Binary, also muss das Binary sich selbst fragen
//! koennen. Diese Zeugen halten drei Dinge: wohin es fragt, was als gesund
//! gilt, und dass die echte Frage ueber TCP in beide Richtungen stimmt, gegen
//! einen Dienst, der antwortet, und gegen einen Port, an dem niemand ist.

use std::io::{Read, Write};
use std::net::TcpListener;

use maer_sync::befehle::{gesund, gesundheit_fragen, ziel_aus_bindung};

#[test]
fn eine_bindung_an_alle_adressen_wird_zur_schleife() {
    assert_eq!(ziel_aus_bindung("0.0.0.0:8099").unwrap().to_string(), "127.0.0.1:8099");
    assert_eq!(ziel_aus_bindung("[::]:8099").unwrap().to_string(), "[::1]:8099");
    assert_eq!(ziel_aus_bindung("127.0.0.1:8081").unwrap().to_string(), "127.0.0.1:8081");
    assert_eq!(ziel_aus_bindung("10.0.0.5:9000").unwrap().to_string(), "10.0.0.5:9000");
    // Eine unlesbare Bindung ist kein geratener Port.
    assert!(ziel_aus_bindung("localhost").is_none());
    assert!(ziel_aus_bindung("").is_none());
}

#[test]
fn gesund_heisst_200_und_der_koerper_des_dienstes() {
    assert!(gesund(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\r\n{\"ok\":true}"));
    assert!(gesund(b"HTTP/1.1 200 OK\r\n\r\n{\"ok\": true}\n"));
    // Ein fremder Dienst auf demselben Port antwortet auch mit 200.
    assert!(!gesund(b"HTTP/1.1 200 OK\r\n\r\n<html>hallo</html>"));
    assert!(!gesund(b"HTTP/1.1 200 OK\r\n\r\n{\"ok\":false}"));
    assert!(!gesund(b"HTTP/1.1 503 Service Unavailable\r\n\r\n{\"ok\":true}"));
    assert!(!gesund(b""));
}

/// Ein Dienst, der genau eine Anfrage beantwortet, und zwar mit `antwort`.
fn ein_dienst(antwort: &'static [u8]) -> std::net::SocketAddr {
    let horcher = TcpListener::bind("127.0.0.1:0").expect("ein freier Port");
    let wo = horcher.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut l, _)) = horcher.accept() {
            let mut puffer = [0u8; 1024];
            let _ = l.read(&mut puffer);
            let _ = l.write_all(antwort);
        }
    });
    wo
}

#[test]
fn die_echte_frage_findet_einen_gesunden_dienst() {
    let wo = ein_dienst(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\n\r\n{\"ok\":true}");
    let satz = gesundheit_fragen(wo).expect("gesund");
    assert!(satz.starts_with("healthy"), "{satz}");
}

#[test]
fn die_echte_frage_erkennt_einen_fremden_dienst() {
    let wo = ein_dienst(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhallo");
    assert!(gesundheit_fragen(wo).is_err());
}

#[test]
fn ohne_dienst_ist_es_nicht_gesund() {
    // Einen Port belegen und wieder freigeben: dort lauscht danach niemand.
    let wo = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let grund = gesundheit_fragen(wo).expect_err("an einem leeren Port ist nichts gesund");
    assert!(grund.contains("nothing answers"), "{grund}");
}
