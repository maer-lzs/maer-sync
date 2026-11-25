// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! `maer-sync-abruf`: the fetch program of the catalog index. It runs one
//! source at a time as the plan says and sleeps in between; every run leaves
//! one line on stderr. `MAER_SYNC_KATALOG` names the catalog file.

use maer_sync_abruf::netz::{Netz, SystemUhr, Uhr};
use maer_sync_abruf::programm::Programm;
use maer_sync_abruf::ureq_holer::UreqHoler;

fn main() -> std::process::ExitCode {
    let path = std::env::var("MAER_SYNC_KATALOG")
        .ok().filter(|p| !p.is_empty())
        .unwrap_or_else(|| "/var/lib/maer-sync-katalog/katalog.db".into());
    let mut p = match Programm::new(Netz::new(UreqHoler::new(), SystemUhr), &path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("abruf: the catalog {path} cannot be opened: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    eprintln!("abruf: started, catalog {path}");
    loop {
        let (bericht, schlaf) = p.schritt();
        if let Some(b) = bericht {
            eprintln!("{}", b.zeile());
        }
        p.netz.uhr.schlafe_ms(schlaf * 1000);
    }
}
