// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Fences of the fetch program: the witnesses' holer stays in the
//! witnesses, every fetch crate is listed with its licence, and the
//! container is built like the service's.

use std::collections::BTreeSet;

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn the_plain_http_holer_stays_in_the_witnesses() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for e in std::fs::read_dir(&src).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&p).unwrap();
        let uses = text.matches("plain_http_for_witnesses").count();
        let allowed = if name == "ureq_holer.rs" { 1 } else { 0 };
        assert_eq!(uses, allowed, "{name} names plain_http_for_witnesses {uses} times");
    }
}

fn packages(lock: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut name = None;
    for l in lock.lines() {
        if let Some(v) = l.strip_prefix("name = \"") { name = Some(v.trim_end_matches('"').to_string()); }
        else if let (Some(v), Some(n)) = (l.strip_prefix("version = \""), name.take()) { out.insert(format!("{n} {}", v.trim_end_matches('"'))); }
    }
    out
}

#[test]
fn every_fetch_crate_is_listed_with_its_licence() {
    let service = packages(&read("../Cargo.lock"));
    let fetch = packages(&read("Cargo.lock"));
    let list = read("FREMDTEILE.md");
    let own = "maer-sync-abruf 0.0.0".to_string();
    let missing: Vec<&String> = fetch.difference(&service).filter(|p| **p != own)
        .filter(|p| !list.lines().any(|l| l.starts_with(&format!("| `{p}` |")))).collect();
    assert!(missing.is_empty(), "not in FREMDTEILE.md: {missing:?}");
    for l in list.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = l.split('|').map(str::trim).collect();
        assert!(cells.len() >= 4 && !cells[2].is_empty(), "a line without licence: {l}");
        let p = cells[1].trim_matches('`');
        assert!(fetch.contains(p), "FREMDTEILE.md names {p}, the lock file does not");
    }
}

#[test]
fn the_container_is_built_like_the_service() {
    let service = read("../Dockerfile");
    let fetch = read("Dockerfile");
    let base = |t: &str| t.lines().find(|l| l.starts_with("FROM rust@sha256:")).map(str::to_string);
    assert!(base(&service).is_some());
    assert_eq!(base(&fetch), base(&service), "another base image");
    let musl = |t: &str| t.lines().find(|l| l.contains("musl-dev~")).map(|l| l.trim().to_string());
    assert_eq!(musl(&fetch), musl(&service));
    assert!(fetch.contains("FROM scratch"));
    assert!(fetch.contains("USER 10002:10002"), "the fetch program runs as its own uid");
}

#[test]
fn the_service_mounts_the_catalog_read_only() {
    let compose = read("../compose.yaml");
    let block = |name: &str| -> String {
        let start = compose.find(&format!("\n  {name}:\n")).unwrap_or_else(|| panic!("no service {name}"));
        let rest = &compose[start + 1..];
        let end = rest[3..].find("\n  ").map(|i| i + 3).unwrap_or(rest.len());
        let mut out = String::new();
        for l in rest.lines() {
            if out.len() > end { break; }
            out.push_str(l); out.push('\n');
            if l.trim().is_empty() && out.len() > 10 { }
        }
        let next = rest[1..].find("\n  ").map(|i| i + 1);
        let mut s = rest.to_string();
        // the block ends where the next service of the same depth begins
        let mut cut = s.len();
        for (i, _) in s.match_indices("\n  ") {
            let after = &s[i + 3..];
            if i > 0 && !after.starts_with(' ') && !after.starts_with('#') { cut = i; break; }
        }
        let _ = next;
        s.truncate(cut);
        s
    };
    let sync = block("maer-sync");
    let abruf = block("maer-sync-abruf");
    assert!(sync.contains("maer-sync-katalog:/var/lib/maer-sync-katalog:ro"), "the service mounts the catalog writable");
    assert!(sync.contains("MAER_SYNC_KATALOG: /var/lib/maer-sync-katalog/katalog.db"));
    assert!(abruf.contains("maer-sync-katalog:/var/lib/maer-sync-katalog\n") || abruf.contains("maer-sync-katalog:/var/lib/maer-sync-katalog:rw"));
    assert!(!abruf.contains("maer-sync-store"), "the fetch program sees the user store");
    assert!(abruf.contains("mem_limit: 1g"));
    assert!(abruf.contains("init: true"));
}
