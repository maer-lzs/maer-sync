// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Text rules the readers share, as the app spells them in Swift.

/// Swift's `.whitespaces`: spaces and tabs, not line breaks.
pub fn trim_spaces(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() && !is_newline(c))
}

/// A line break as Swift counts it.
pub fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}
