// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The fetch program as a program.
//!
//! One `schritt` runs the source that is due (if any) and answers how long
//! to sleep. The catalog is opened for that run only and closed cleanly
//! after it (`close_clean`, `journal_mode=DELETE`), so a stop while the
//! program sleeps, which is nearly always, leaves no side files and the
//! service, which mounts the catalog read only, always opens it. A stop in
//! the middle of a run leaves the side files; measured 25.11.2026,
//! the service then still reads the last committed state, and the next run
//! cleans up. So the program needs no signal handling of its own.
//!
//! Failures stay in memory (backoff, `Retry-After`); a restart forgets them
//! and tries again, which is the safe side. Run times come from the catalog.
//!
//! One run at a time on a catalog file: every run, planned or confirmed by
//! the operator, holds the run lock (`Katalog::laufsperre`) from its first
//! request to its clean close. A run that finds it taken fetches and writes
//! nothing; the plan tries again after a minute and counts no failure.

use std::collections::BTreeMap;

use maer_sync::katalog::{Katalog, KatalogError};

use crate::lauf::{datum, lauf, lauf_bestaetigt, Einstellungen, LaufFehler, Laufbericht};
use crate::netz::{HolFehler, Holer, Netz, Uhr};
use crate::plan::{naechster, schlaf_s, Art, Zustand, MIN_SCHLAF_S};

/// What the program was started to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Befehl {
    /// No argument: run the sources as the plan says, without end.
    Dauerlauf,
    /// `--bestaetigt <quelle>`: one confirmed full pass of that source, then end.
    Bestaetigt(String),
}

/// Read the arguments after the program name.
pub fn befehl(args: &[String]) -> Result<Befehl, String> {
    match args {
        [] => Ok(Befehl::Dauerlauf),
        [flag, quelle] if flag == "--bestaetigt" && QUELLEN.contains(&quelle.as_str()) =>
            Ok(Befehl::Bestaetigt(quelle.clone())),
        _ => Err(format!("usage: maer-sync-abruf [--bestaetigt <quelle>], quelle one of {}", QUELLEN.join(", "))),
    }
}

/// The sources that have a reader today, in the order of their names.
pub const QUELLEN: &[&str] = &["archive-buch", "gutenberg", "librivox", "musopen", "netlabel", "otr", "radio", "schellack"];

pub struct Programm<H: Holer, U: Uhr> {
    pub netz: Netz<H, U>,
    path: String,
    zustaende: BTreeMap<String, Zustand>,
}

impl<H: Holer, U: Uhr> Programm<H, U> {
    /// Read the run times, reading only: a run of another process may hold
    /// the catalog (measured 26.11.2026: opened for writing beside a writer
    /// it failed at once, and the program ended at its start). A file that
    /// is not there yet has no run times; the first run lays it down. A
    /// file that holds no catalog yet (no bytes, or no table `meta`, as a
    /// first run leaves it when it ends before the schema) is the same.
    pub fn new(netz: Netz<H, U>, path: &str) -> Result<Self, String> {
        let k = if std::path::Path::new(path).exists() {
            match Katalog::open_read(path) {
                Ok(k) => Some(k),
                Err(KatalogError::NoCatalog) => None,
                Err(e) => return Err(e.to_string()),
            }
        } else {
            None
        };
        let mut zustaende = BTreeMap::new();
        for q in QUELLEN {
            let z = match &k {
                Some(k) => Zustand {
                    letzter_voll: k.last_run(q, "voll").map_err(|e| e.to_string())?,
                    letzter_delta: k.last_run(q, "delta").map_err(|e| e.to_string())?,
                    ..Default::default()
                },
                None => Zustand::default(),
            };
            zustaende.insert(q.to_string(), z);
        }
        Ok(Programm { netz, path: path.to_string(), zustaende })
    }

    fn jetzt_s(&self) -> i64 {
        self.netz.uhr.jetzt_ms().div_euclid(1000)
    }

    /// The calendar year of the clock (UTC), for the shellac year bound.
    pub fn kalenderjahr(&self) -> i32 {
        datum(self.jetzt_s())[..4].parse().expect("datum starts with the year")
    }

    fn einstellungen(&self) -> Einstellungen {
        Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: self.kalenderjahr() }
    }

    /// One full pass of `quelle` that the operator confirmed
    /// (`--bestaetigt`), outside the plan. The catalog is opened for that run
    /// and closed cleanly, as for a planned one.
    pub fn bestaetigt(&mut self, quelle: &str) -> Laufbericht {
        self.ein_lauf(quelle, Art::Voll, true)
    }

    /// One run under the run lock: take it, open the catalog, run, close
    /// cleanly, and only then let the lock go.
    pub fn ein_lauf(&mut self, quelle: &str, art: Art, bestaetigt: bool) -> Laufbericht {
        let ohne = |ausgang| Laufbericht {
            quelle: quelle.into(), art, gelesen: 0, version_vorher: -1, version_nachher: -1, dauer_ms: 0,
            ausgang: Err(ausgang), bestaetigt, stufen_gewechselt: 0, wege: None,
        };
        let _sperre = match Katalog::laufsperre(&self.path) {
            Ok(s) => s,
            Err(KatalogError::Busy) => return ohne(LaufFehler::Belegt),
            Err(err) => return ohne(LaufFehler::Katalog(err.to_string())),
        };
        let e = self.einstellungen();
        match Katalog::open(&self.path) {
            Ok(mut k) => {
                let b = if bestaetigt {
                    lauf_bestaetigt(&mut self.netz, &mut k, quelle, &e)
                } else {
                    lauf(&mut self.netz, &mut k, quelle, art, &e)
                };
                if let Err(err) = k.close_clean() {
                    eprintln!("abruf: catalog not closed cleanly: {err}");
                }
                b
            }
            Err(err) => ohne(LaufFehler::Katalog(err.to_string())),
        }
    }

    /// Run the source that is due, if any, and answer the seconds to sleep.
    pub fn schritt(&mut self) -> (Option<Laufbericht>, i64) {
        let jetzt = self.jetzt_s();
        let Some((quelle, art)) = naechster(jetzt, &self.zustaende) else {
            return (None, schlaf_s(jetzt, &self.zustaende));
        };
        let bericht = self.ein_lauf(&quelle, art, false);
        let nach = self.jetzt_s();
        let z = self.zustaende.entry(quelle).or_default();
        match &bericht.ausgang {
            // Another run holds the catalog: later again, and no failure.
            Err(LaufFehler::Belegt) => z.nicht_vor = Some(nach + MIN_SCHLAF_S),
            Ok(()) => {
                z.fehler = 0;
                z.letzter_fehler = None;
                z.nicht_vor = None;
                match art {
                    Art::Voll => z.letzter_voll = Some(nach),
                    Art::Delta => z.letzter_delta = Some(nach),
                }
            }
            Err(err) => {
                z.fehler += 1;
                z.letzter_fehler = Some(nach);
                if let LaufFehler::Holen(HolFehler::SpaeterWieder(s)) = err {
                    z.nicht_vor = Some(nach + *s as i64);
                }
            }
        }
        (Some(bericht), schlaf_s(nach, &self.zustaende))
    }
}
