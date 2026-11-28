// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The echo of archive.org is compared after one pass of unmasking: the five
//! entities a server writes for the characters of a query. Today's queries
//! carry only the quote; the others have no way in through a run, so they are
//! witnessed here, on the comparison itself.

use maer_sync_abruf::lauf::echo_gleich;

#[test]
fn e3_the_five_entities_are_unmasked() {
    assert!(echo_gleich("a:\"b\"", "a:&quot;b&quot;"));
    assert!(echo_gleich("a & b", "a &amp; b"));
    assert!(echo_gleich("a < b > c", "a &lt; b &gt; c"));
    assert!(echo_gleich("it's", "it&#39;s"));
    assert!(echo_gleich("plain", "plain"));
}

#[test]
fn e3_once_only_and_nothing_else() {
    assert!(!echo_gleich("a:\"b\"", "a:&amp;quot;b&amp;quot;"), "unmasked twice");
    assert!(!echo_gleich("a:\"b\"", "a:&#34;b&#34;"), "an entity outside the five");
    assert!(!echo_gleich("a:b", "a:c"));
    assert!(!echo_gleich("a & b", "a &amp b"), "an entity without its semicolon");
}
