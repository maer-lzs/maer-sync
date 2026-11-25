// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Code is no place for decisions: a comment says what the code does and,
//! where it is not obvious, why it has to. Work-order codes (a letter and a
//! number, as `K3a-4`) and references to who decided what stay out of
//! comments; the date of a measurement may stay.

fn code_tokens(line: &str) -> Vec<String> {
    // A capital K, one or more digits, an optional small letter, an optional
    // dash and digits, standing as a word of its own.
    let b = line.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let start_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if start_ok && b[i] == b'K' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() { j += 1; }
            if j < b.len() && b[j].is_ascii_lowercase() { j += 1; }
            if j + 1 < b.len() && b[j] == b'-' && b[j + 1].is_ascii_digit() {
                j += 1;
                while j < b.len() && b[j].is_ascii_digit() { j += 1; }
            }
            let end_ok = j == b.len() || !(b[j].is_ascii_alphanumeric() || b[j] == b'_');
            if end_ok { out.push(line[i..j].to_string()); }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// A date as "25.11.2026" or "2026-11-25".
fn has_date(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |i: usize, n: usize| i + n <= b.len() && b[i..i + n].iter().all(u8::is_ascii_digit);
    (0..b.len()).any(|i| {
        (digits(i, 4) && i + 10 <= b.len() && b[i + 4] == b'-' && digits(i + 5, 2) && b[i + 7] == b'-' && digits(i + 8, 2))
            || ((digits(i, 1) || digits(i, 2)) && {
                let d = if digits(i, 2) { 2 } else { 1 };
                i + d + 1 < b.len() && b[i + d] == b'.' && {
                    let j = i + d + 1;
                    let m = if digits(j, 2) { 2 } else if digits(j, 1) { 1 } else { 0 };
                    m > 0 && j + m < b.len() && b[j + m] == b'.' && digits(j + m + 1, 4)
                }
            })
    })
}

fn comment_of(line: &str, hash: bool) -> Option<&str> {
    let t = line.trim_start();
    if let Some(i) = line.find("//") { return Some(&line[i..]); }
    if hash && t.starts_with('#') { return Some(t); }
    None
}

#[test]
fn comments_carry_no_order_codes_and_no_decisions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = vec![];
    let mut files = vec![];
    for dir in ["src", "tests", "abruf/src", "abruf/tests"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|x| x.to_str()) == Some("rs") && !p.ends_with("tests/kommentare.rs") {
                files.push((p, false));
            }
        }
    }
    for f in ["Cargo.toml", "abruf/Cargo.toml", "abruf/Dockerfile", "Dockerfile", "compose.yaml"] {
        files.push((root.join(f), true));
    }
    for (p, hash) in files {
        let text = std::fs::read_to_string(&p).unwrap();
        for (n, line) in text.lines().enumerate() {
            let Some(c) = comment_of(line, hash) else { continue };
            let low = c.to_lowercase();
            let codes = code_tokens(c);
            // A reference to a decision: the word with a date, or who decided.
            let word = low.contains("decision") || low.contains("decided") || low.contains("entscheid");
            let who = low.contains("operator's decision") || low.contains("decision of the operator") || low.contains("operator decided");
            // ... or a decision named by its code ("decision D4").
            let coded = low.split("decision d").skip(1).any(|r| r.starts_with(|c: char| c.is_ascii_digit()));
            let decision = who || coded || (word && has_date(&low));
            if !codes.is_empty() || decision {
                found.push(format!("{}:{} {:?}{}", p.strip_prefix(root).unwrap().display(), n + 1, codes,
                                   if decision { " decision" } else { "" }));
            }
        }
    }
    assert!(found.is_empty(), "{} comment lines:\n{}", found.len(), found.join("\n"));
}

#[test]
fn the_scan_finds_what_it_should() {
    assert_eq!(code_tokens("// K3a-4 and K3c, not K3add or XK3"), vec!["K3a-4", "K3c"]);
    assert_eq!(code_tokens("// see K10"), vec!["K10"]);
    assert!(code_tokens("// Kotlin 3 and K-pop").is_empty());
    assert!(has_date("decision of 25.11.2026") && has_date("2026-11-25") && !has_date("version 6.2"));
}
