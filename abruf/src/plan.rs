// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! When which source is due (K3c). Signatures only.

use std::collections::BTreeMap;

pub const DELTA_S: i64 = 86_400;
pub const VOLL_S: i64 = 7 * 86_400;
pub const BACKOFF_S: [i64; 3] = [900, 3_600, 21_600];
pub const MIN_SCHLAF_S: i64 = 60;
pub const MAX_SCHLAF_S: i64 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Art { Voll, Delta }

impl Art {
    pub fn name(self) -> &'static str { match self { Art::Voll => "voll", Art::Delta => "delta" } }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Zustand {
    pub letzter_voll: Option<i64>,
    pub letzter_delta: Option<i64>,
    pub fehler: u32,
    pub letzter_fehler: Option<i64>,
    pub nicht_vor: Option<i64>,
}

pub fn faellig(_jetzt: i64, _z: &Zustand) -> Option<Art> { todo!("K3c") }
pub fn wann(_z: &Zustand) -> i64 { todo!("K3c") }
pub fn naechster(_jetzt: i64, _z: &BTreeMap<String, Zustand>) -> Option<(String, Art)> { todo!("K3c") }
pub fn schlaf_s(_jetzt: i64, _z: &BTreeMap<String, Zustand>) -> i64 { todo!("K3c") }
