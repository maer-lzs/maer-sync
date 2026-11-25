// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The tree of the service stays as it was. The fetch program (`abruf/`) is
//! a package of its own, and the crates it needs to fetch (`ureq` and those
//! that follow it) live only there. This
//! witness asks cargo for the tree of the delivered package itself, for the
//! target the delivery is built for, not only the manifest.

use std::collections::BTreeSet;
use std::process::Command;

/// The crates of the service before the fetch program existed (measured
/// 25.11.2026 on `dev` b5825a2: `cargo tree -e normal`, musl target).
const SERVICE_CRATES: usize = 76;
const FETCH_CRATES: [&str; 6] = ["ureq", "roxmltree", "csv", "rustls", "ring", "webpki-roots"];

#[test]
fn the_service_names_no_fetch_crate() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.args(["tree", "-e", "normal", "--prefix", "none", "--format", "{p}", "--offline", "-p", "maer-sync", "--manifest-path", manifest]);
    if let Ok(target) = std::env::var("CARGO_BUILD_TARGET") {
        cmd.args(["--target", &target]);
    }
    let out = cmd.output().expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    let names: BTreeSet<String> = String::from_utf8_lossy(&out.stdout).lines()
        .map(|l| l.split(" (").next().unwrap().trim_end_matches(" (*)").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    for c in FETCH_CRATES {
        assert!(!names.iter().any(|n| n.split(' ').next() == Some(c)), "{c} is in the tree of the service");
    }
    assert_eq!(names.len(), SERVICE_CRATES, "the tree of the service changed: {names:?}");
}
