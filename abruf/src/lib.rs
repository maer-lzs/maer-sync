// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The fetch program of the catalog index: readers that turn an answer of a
//! source into catalog rows. They read bytes and never the network, so the
//! shared vectors can prove them.

pub mod archive;
pub mod lauf;
pub mod musopen;
pub mod netz;
pub mod plan;
pub mod programm;
pub mod radio;
pub mod stufe;
pub mod text;
pub mod ureq_holer;
