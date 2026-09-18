// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Wann eine verschluesselte Sitzung endet, und welche weicht, wenn der Raum
//! voll ist.
//!
//! **Warum das eine Regel ist und keine Zahl im Transport**, und beides ist
//! gemessen und nicht angenommen:
//!
//! 1. Der Handschlag ist ANONYM, so will es das Muster. `NK` heisst, dass der
//!    Rufer nichts beweist, bevor er drin ist, und genau das schuetzt die
//!    ANMELDUNG. Der Preis: jeder, der den Dienst erreicht, darf eine Sitzung
//!    verlangen. **Eine Decke allein beantwortet das NICHT**, sie verwandelt
//!    Erschoepfung in VERDRAENGUNG: ein Fremder, der weiter fragt, schiebt
//!    echte Geraete einzeln hinaus, ohne Absturz, ohne Wachstum, ohne
//!    Protokollzeile. Deshalb fragt die Regel zuerst: hat diese Sitzung je
//!    eine Anfrage getragen, die bewies, wer fragt? Eine, die das nicht hat,
//!    ist ein Handschlag und sonst nichts, und Handschlaege weichen zuerst.
//! 2. Eine Sitzung, die endet, darf nicht wie ein Fehler aussehen. Die Antwort
//!    des Klienten ist ein neuer Handschlag und ein zweiter Versuch, nie ein
//!    Satz an einen Menschen.

/// Eine Stunde ohne ein Wort. Die Takte des Klienten sind 10, 30 und 60
/// Sekunden, eine arbeitende App kann das also nie ausloesen.
pub const IDLE_SECONDS: f64 = 3600.0;
/// Eine Sitzung, die nie etwas bewiesen hat, bekommt eine MINUTE.
pub const UNPROVEN_IDLE_SECONDS: f64 = 60.0;
/// Ein Tag, auch fuer eine Sitzung, die nie schweigt.
pub const MAX_AGE_SECONDS: f64 = 86400.0;
/// Wie viele gleichzeitig gehalten werden. Eine Decke fuer den anonymen
/// Rufer, kein Kapazitaetsplan.
pub const CAPACITY: usize = 4096;

pub const LIVE: &str = "live";
pub const IDLE: &str = "idle";
pub const OLD: &str = "old";

/// Die Zahlen zusammen, damit ein Zeuge sie senken kann, ohne das Modul
/// anzufassen.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub idle_seconds: f64,
    pub unproven_idle_seconds: f64,
    pub max_age_seconds: f64,
    pub capacity: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            idle_seconds: IDLE_SECONDS,
            unproven_idle_seconds: UNPROVEN_IDLE_SECONDS,
            max_age_seconds: MAX_AGE_SECONDS,
            capacity: CAPACITY,
        }
    }
}

/// Eine Sitzung, wie die Buchfuehrung sie sieht.
#[derive(Debug, Clone, PartialEq)]
pub struct Sitzung {
    pub id: String,
    pub created_at: f64,
    pub last_used_at: f64,
    pub proven: bool,
}

/// Ob eine Sitzung noch benutzt werden darf, und wenn nicht, warum nicht.
///
/// `proven` hat mit Absicht KEINE Vorgabe: eine Vorgabe waere eine Vermutung
/// ueber die sicherheitsrelevanteste Eingabe hier.
///
/// Das Alter wird VOR der Stille geprueft. Eine Sitzung kann beides sein, und
/// die aeltere der beiden Bedingungen haette sie zuerst beendet.
pub fn standing(created_at: f64, last_used_at: f64, now: f64, proven: bool, limits: &Limits)
    -> &'static str
{
    if now - created_at >= limits.max_age_seconds {
        return OLD;
    }
    let stille = if proven { limits.idle_seconds } else { limits.unproven_idle_seconds };
    if now - last_used_at >= stille {
        return IDLE;
    }
    LIVE
}

/// Welche Sitzungen gehen, damit EINE mehr passt.
///
/// **Unbewiesene zuerst, und das ist der ganze Zweck.** Nur "zuletzt benutzt"
/// ist genau falsch unter dem einen Druck, den dieser Speicher wirklich
/// trifft: der Handschlag ist anonym, ein Fremder kann Sitzungen so schnell
/// machen, wie das Netz erlaubt, und jede neue ist die zuletzt benutzte. Unter
/// reinem LRU ueberleben also immer die Fremden.
///
/// **Die Grenze, und sie ist echt:** eine Flut konkurriert weiter mit dem
/// ersten Handschlag eines echten Geraets, denn der ist auch unbewiesen. Ein
/// Fremder kann also eine Anmeldung verzoegern; hinausschieben kann er
/// niemanden, der schon angemeldet ist.
pub fn give_way(sessions: &[(String, f64, bool)], limits: &Limits) -> Vec<String> {
    if sessions.len() < limits.capacity {
        return Vec::new();
    }
    let surplus = sessions.len() - limits.capacity + 1;
    let mut order: Vec<&(String, f64, bool)> = sessions.iter().collect();
    // Unbewiesene vor bewiesenen, dann zuletzt benutzt innerhalb jeder Gruppe.
    // Stabil, wie die Python-Seite: bei gleichem Schluessel bleibt die Ordnung.
    order.sort_by(|a, b| {
        a.2.cmp(&b.2).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    order.into_iter().take(surplus).map(|r| r.0.clone()).collect()
}

/// Die Kennungen, die JETZT fallen: alles, was nicht `live` ist.
///
/// Getrennt von `give_way` mit Absicht. Dies beantwortet "was ist tot", und
/// das ist wahr, ob jemand Platz will oder nicht.
pub fn sweep(sessions: &[Sitzung], now: f64, limits: &Limits) -> Vec<String> {
    sessions.iter()
        .filter(|s| standing(s.created_at, s.last_used_at, now, s.proven, limits) != LIVE)
        .map(|s| s.id.clone())
        .collect()
}

/// Alles, was gehen muss, damit eine mehr passt. EINE Tuer.
///
/// **Das gibt es, weil die ORDNUNG die Entscheidung ist**, und ein Rufer, der
/// zwei Aufrufe machte, bekaeme sie falsch. Erst fegen und dann weichen lassen
/// ist nicht dasselbe wie umgekehrt:
///
/// zwei bewiesene Sitzungen, seit Stunden still, und ein echtes Geraet mitten
/// im Handschlag, in einem Speicher fuer drei.
///
/// Erst weichen lassen, und die Regel wirft das echte Geraet hinaus, weil ein
/// Handschlag vor einer bewiesenen Sitzung weicht und beide toten Zeilen ihren
/// Platz noch besetzen. Erst fegen, und die zwei toten gehen, das Geraet
/// bleibt, und es weicht ueberhaupt niemand. **Eine tote Sitzung darf eine
/// lebende nie ihren Platz kosten.**
pub fn room_for_one(sessions: &[Sitzung], now: f64, limits: &Limits) -> Vec<String> {
    let mut tot: Vec<String> = sweep(sessions, now, limits);
    tot.sort();
    let lebend: Vec<(String, f64, bool)> = sessions.iter()
        .filter(|s| !tot.contains(&s.id))
        .map(|s| (s.id.clone(), s.last_used_at, s.proven))
        .collect();
    let mut aus = tot;
    aus.extend(give_way(&lebend, limits));
    aus
}
