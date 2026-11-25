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
    let mut differ = vec![];
    for (name, v) in &service {
        match fetch.get(name) {
            Some(w) if w == v => {}
            Some(w) => differ.push(format!("{name}: service {v:?}, fetch {w:?}")),
            None => differ.push(format!("{name}: missing in the fetch lock")),
        }
    }
    assert!(differ.is_empty(), "{differ:#?}");
}
