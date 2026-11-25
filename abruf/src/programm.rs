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

use std::collections::BTreeMap;

use maer_sync::katalog::Katalog;

use crate::lauf::{datum, lauf, Einstellungen, LaufFehler, Laufbericht};
use crate::netz::{HolFehler, Holer, Netz, Uhr};
use crate::plan::{naechster, schlaf_s, Art, Zustand};

/// The sources that have a reader today, in the order of their names.
pub const QUELLEN: &[&str] = &["archive-buch", "librivox", "musopen", "netlabel", "otr", "radio", "schellack"];

pub struct Programm<H: Holer, U: Uhr> {
    pub netz: Netz<H, U>,
    path: String,
    zustaende: BTreeMap<String, Zustand>,
}

impl<H: Holer, U: Uhr> Programm<H, U> {
    /// Open the catalog once to read the run times, and close it again.
    pub fn new(netz: Netz<H, U>, path: &str) -> Result<Self, String> {
        let k = Katalog::open(path).map_err(|e| e.to_string())?;
        let mut zustaende = BTreeMap::new();
        for q in QUELLEN {
            let z = Zustand {
                letzter_voll: k.last_run(q, "voll").map_err(|e| e.to_string())?,
                letzter_delta: k.last_run(q, "delta").map_err(|e| e.to_string())?,
                ..Default::default()
            };
            zustaende.insert(q.to_string(), z);
        }
        k.close_clean().map_err(|e| e.to_string())?;
        Ok(Programm { netz, path: path.to_string(), zustaende })
    }

    fn jetzt_s(&self) -> i64 {
        self.netz.uhr.jetzt_ms().div_euclid(1000)
    }

    /// The calendar year of the clock (UTC), for the shellac year bound.
    pub fn kalenderjahr(&self) -> i32 {
        datum(self.jetzt_s())[..4].parse().expect("datum starts with the year")
    }

    /// Run the source that is due, if any, and answer the seconds to sleep.
    pub fn schritt(&mut self) -> (Option<Laufbericht>, i64) {
        let jetzt = self.jetzt_s();
        let Some((quelle, art)) = naechster(jetzt, &self.zustaende) else {
            return (None, schlaf_s(jetzt, &self.zustaende));
        };
        let e = Einstellungen { scrape_count: 10_000, adv_rows: 10_000, kalenderjahr: self.kalenderjahr() };
        let bericht = match Katalog::open(&self.path) {
            Ok(mut k) => {
                let b = lauf(&mut self.netz, &mut k, &quelle, art, &e);
                if let Err(err) = k.close_clean() {
                    eprintln!("abruf: catalog not closed cleanly: {err}");
                }
                b
            }
            Err(err) => Laufbericht {
                quelle: quelle.clone(), art, gelesen: 0, version_vorher: -1, version_nachher: -1, dauer_ms: 0,
                ausgang: Err(LaufFehler::Katalog(err.to_string())),
            },
        };
        let nach = self.jetzt_s();
        let z = self.zustaende.entry(quelle).or_default();
        match &bericht.ausgang {
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
