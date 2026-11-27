// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! `maer-sync-abruf`: the fetch program of the catalog index. It runs one
//! source at a time as the plan says and sleeps in between; every run leaves
//! one line on stderr. `MAER_SYNC_KATALOG` names the catalog file.
//!
//! `maer-sync-abruf --bestaetigt <quelle>` runs one full pass of that
//! source that the operator confirmed, for a source the catalog refused
//! because it would lose too many rows, and ends: 0 when it was written.

use maer_sync_abruf::netz::{Netz, SystemUhr, Uhr};
use maer_sync_abruf::programm::{befehl, Befehl, Programm};
use maer_sync_abruf::ureq_holer::UreqHoler;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let befehl = match befehl(&args) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("abruf: {e}");
            return std::process::ExitCode::from(2);
        }
    };
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
    if let Befehl::Bestaetigt(quelle) = befehl {
        eprintln!("abruf: one confirmed full pass of {quelle}, catalog {path}");
        let b = p.bestaetigt(&quelle);
        eprintln!("{}", b.zeile());
        return std::process::ExitCode::from(if b.ausgang.is_ok() { 0 } else { 1 });
    }
    eprintln!("abruf: started, catalog {path}");
    loop {
        let (bericht, schlaf) = p.schritt();
        if let Some(b) = bericht {
            eprintln!("{}", b.zeile());
        }
        p.netz.uhr.schlafe_ms(schlaf * 1000);
    }
}
