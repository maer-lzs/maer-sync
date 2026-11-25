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
    // A service block runs from its name to the first line indented by less
    // than four spaces (the next service, its comment, or a top-level key).
    let block = |name: &str| -> String {
        let head = format!("  {name}:");
        let mut lines = compose.lines().skip_while(|l| *l != head);
        let mut out = String::new();
        out.push_str(lines.next().unwrap_or_else(|| panic!("no service {name}")));
        out.push('\n');
        for l in lines {
            if !l.trim().is_empty() && !l.starts_with("    ") {
                break;
            }
            out.push_str(l);
            out.push('\n');
        }
        out
    };
    let sync = block("maer-sync");
    let abruf = block("maer-sync-abruf");
    assert!(sync.contains("maer-sync-katalog:/var/lib/maer-sync-katalog:ro"), "the service mounts the catalog writable");
    // Off by default: the service names the catalog only when the operator sets it.
    assert!(sync.contains("MAER_SYNC_KATALOG: ${MAER_SYNC_KATALOG:-}"), "the service names a catalog by default");
    assert!(abruf.contains("maer-sync-katalog:/var/lib/maer-sync-katalog\n") || abruf.contains("maer-sync-katalog:/var/lib/maer-sync-katalog:rw"));
    assert!(!abruf.contains("maer-sync-store"), "the fetch program sees the user store");
    assert!(abruf.contains("mem_limit: 1g"));
    assert!(abruf.contains("init: true"));
}

#[test]
fn the_default_compose_does_not_start_the_fetch() {
    // A plain `docker compose up -d` after an update must not begin to fetch
    // from archive.org: the fetch program sits behind its own profile.
    let compose = read("../compose.yaml");
    let start = compose.find("\n  maer-sync-abruf:\n").expect("the fetch service");
    let block: String = compose[start + 1..].lines().skip(1)
        .take_while(|l| l.trim().is_empty() || l.starts_with("    ")).collect::<Vec<_>>().join("\n");
    assert!(block.contains("profiles: [\"katalog\"]"), "the fetch program starts by default");
    let readme = read("../README.md");
    assert!(readme.contains("## Catalog index (optional)"), "the README does not say how to switch it on");
    assert!(readme.contains("docker compose --profile katalog up -d"));
    assert!(readme.contains("`MAER_SYNC_KATALOG`"));
}

/// The licence texts travel with the program: CDLA-Permissive-2.0 of
/// webpki-roots asks only that its text goes with the data, and the other
/// new licences ask the same of a binary. Every crate the list marks as built
/// has its licence files under `abruf/lizenzen/<crate>-<version>/`, the image
/// copies that folder, and the list points to it.
#[test]
fn every_built_fetch_crate_carries_its_licence_texts() {
    let list = read("FREMDTEILE.md");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lizenzen");
    let mut built = 0;
    for l in list.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = l.split('|').map(str::trim).collect();
        if cells[3] != "yes" { continue; }
        built += 1;
        let dir = root.join(cells[1].trim_matches('`').replace(' ', "-"));
        let files: Vec<String> = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.to_uppercase().starts_with("LICENSE")).collect();
        assert!(!files.is_empty(), "no licence file for {}", cells[1]);
    }
    assert_eq!(built, 17);
    let cdla = std::fs::read_to_string(root.join("webpki-roots-1.0.12").join("LICENSE")).unwrap();
    assert!(cdla.contains("Community Data License Agreement"), "the CDLA text of webpki-roots is missing");
    assert!(read("Dockerfile").contains("COPY abruf/lizenzen /lizenzen"), "the image does not carry the licence texts");
    assert!(list.contains("abruf/lizenzen/") && list.contains("/lizenzen"), "the list does not point to the texts");
}
