// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Scheibe 1: was INNEN reist, wer der Dienst IST, und wie die Sitzung entsteht.
//!
//! Die drei Module hier sind rein. Sie kennen keine Datei, keinen Socket und
//! keine Uhr, und genau deshalb sind sie an den veroeffentlichten Vektoren
//! pruefbar. Was sie tun, steht byte-fuer-byte in `core/frames.py`,
//! `core/identity.py` und `core/handshake.py` der Python-Fassung; diese hier
//! ist eine zweite Umsetzung derselben Zahlen, keine neue Absicht.

pub mod admission;
pub mod aufstieg;
pub mod befehle;
pub mod authorization;
pub mod contract;
pub mod formats;
pub mod frames;
pub mod handshake;
pub mod horcher;
pub mod identity;
pub mod kennung;
pub mod listeners;
pub mod paths;
pub mod protokoll;
pub mod restore;
pub mod sessions;
pub mod sitzungsladen;
pub mod store;
pub mod versions;
pub mod wege;
pub mod writes;
