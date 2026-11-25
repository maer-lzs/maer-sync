// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The lock file of the fetch program started as a copy of the service's;
//! every crate both lock files name must keep the same version in both, so
//! the catalog code is built and tested against what the service delivers.

use std::collections::BTreeMap;

fn versions(path: &str) -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut name: Option<String> = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("name = \"") {
            name = Some(v.trim_end_matches('"').to_string());
        } else if let (Some(v), Some(n)) = (line.strip_prefix("version = \""), name.take()) {
            out.entry(n).or_default().push(v.trim_end_matches('"').to_string());
        }
    }
    for v in out.values_mut() {
        v.sort();
    }
    out
}

#[test]
fn both_locks_name_the_same_versions() {
    let service = versions(concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock"));
    let fetch = versions(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
    assert!(service.len() > 50, "the service lock was not read");
    // Every version the service names stands unchanged in the fetch lock. A
    // fetch crate may bring a second generation of a shared crate (ring
    // brings getrandom 0.2 next to 0.4), but never moves the service's one.
    let mut differ = vec![];
    for (name, v) in &service {
        let w = fetch.get(name).cloned().unwrap_or_default();
        for version in v {
            if !w.contains(version) {
                differ.push(format!("{name} {version}: not in the fetch lock (it has {w:?})"));
            }
        }
    }
    assert!(differ.is_empty(), "{differ:#?}");
}
