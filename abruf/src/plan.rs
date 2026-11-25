// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! When which source is due (K3c). Pure: the time is given in, so the
//! witnesses need no clock.
//!
//! A source without a full pass gets one first. After that a delta is due a
//! day after its last run of either kind, a full pass a week after the last
//! full one; when both are due the full pass wins. The delta only sees
//! records added since (`addeddate`): changed rights and deletions come with
//! the weekly full pass, so the delta alone is never the whole truth.
//!
//! After a failed run the same source waits 15 min, then 1 h, then 6 h; a
//! source that answered "later" (`Retry-After`) is not asked before. One
//! source runs at a time: the one due longest first, then by name.

use std::collections::BTreeMap;

pub const DELTA_S: i64 = 86_400;
pub const VOLL_S: i64 = 7 * 86_400;
pub const BACKOFF_S: [i64; 3] = [900, 3_600, 21_600];
pub const MIN_SCHLAF_S: i64 = 60;
pub const MAX_SCHLAF_S: i64 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Art {
    Voll,
    Delta,
}

impl Art {
    pub fn name(self) -> &'static str {
        match self {
            Art::Voll => "voll",
            Art::Delta => "delta",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Zustand {
    pub letzter_voll: Option<i64>,
    pub letzter_delta: Option<i64>,
    pub fehler: u32,
    pub letzter_fehler: Option<i64>,
    pub nicht_vor: Option<i64>,
}

/// The earliest time a failed or postponed source may run again.
fn gate(z: &Zustand) -> i64 {
    let backoff = match (z.fehler, z.letzter_fehler) {
        (0, _) | (_, None) => i64::MIN,
        (n, Some(t)) => t + BACKOFF_S[(n as usize - 1).min(BACKOFF_S.len() - 1)],
    };
    backoff.max(z.nicht_vor.unwrap_or(i64::MIN))
}

/// What is due for a source at `jetzt`, if anything.
pub fn faellig(jetzt: i64, z: &Zustand) -> Option<Art> {
    if jetzt < gate(z) {
        return None;
    }
    let Some(voll) = z.letzter_voll else { return Some(Art::Voll) };
    if jetzt >= voll + VOLL_S {
        return Some(Art::Voll);
    }
    let last = voll.max(z.letzter_delta.unwrap_or(i64::MIN));
    if jetzt >= last + DELTA_S {
        return Some(Art::Delta);
    }
    None
}

/// The earliest time anything is due for a source.
pub fn wann(z: &Zustand) -> i64 {
    let base = match z.letzter_voll {
        None => i64::MIN,
        Some(voll) => {
            let last = voll.max(z.letzter_delta.unwrap_or(i64::MIN));
            (voll + VOLL_S).min(last + DELTA_S)
        }
    };
    base.max(gate(z))
}

/// The source to run now: due, the one due longest first, then by name.
pub fn naechster(jetzt: i64, z: &BTreeMap<String, Zustand>) -> Option<(String, Art)> {
    z.iter()
        .filter_map(|(q, s)| faellig(jetzt, s).map(|a| (wann(s), q.clone(), a)))
        .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(_, q, a)| (q, a))
}

/// How long to sleep until the next source is due, within one minute and
/// one hour.
pub fn schlaf_s(jetzt: i64, z: &BTreeMap<String, Zustand>) -> i64 {
    let next = z.values().map(wann).min().unwrap_or(i64::MAX);
    next.saturating_sub(jetzt).clamp(MIN_SCHLAF_S, MAX_SCHLAF_S)
}
