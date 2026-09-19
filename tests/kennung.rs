// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! Der Schlüssel auf der Platte, mit eigenen Zeugen.
//!
//! **Warum diese Datei entstanden ist.** Die Zählung der 126 Fälle auf
//! Fall-Ebene hat gefragt, wer `src/kennung.rs` bezeugt, und die Antwort war:
//! niemand. Das Modul trägt `read_key`, `write_key`, `open_identity`, `adopt`
//! und `make_new`, also jede Stelle, an der ein privater Dienst-Schlüssel
//! entsteht, gelesen oder ersetzt wird. Bezeugt war davon nur, was vier
//! CLI-Fälle in `vektoren/befehle.json` am Draht mitnehmen.
//!
//! Das ist genau die Überdehnung, gegen die die Zählung gebaut ist: `identity`
//! hatte einen Gegenpart (`tests/vektoren.rs` rechnet die Abdrücke nach), und
//! daraus las sich „die Kennung ist gedeckt". Gedeckt war das RECHNEN, nicht
//! das SCHREIBEN. Die Fälle, um die es wirklich geht, sind die, in denen etwas
//! auf der Platte nicht stimmt, und keiner von ihnen wurde gefahren.
//!
//! Jeder Zeuge hier steht für genau einen Fall aus `tests/test_identity.py`.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use maer_sync::authorization::zufall;
use maer_sync::identity::{decide, fingerprint, public_of};
use maer_sync::kennung::{adopt, key_path, make_new, open_identity, read_key, write_key,
                         KEY_MODE};
use maer_sync::store::Store;

// ----------------------------------------------------------------- Werkzeug

/// Ein eigener Ordner je Fall, und er verschwindet wieder.
///
/// Kein geratener Name in `/tmp`: ein fester Pfad dort gehört mir nicht, und
/// eine fremde Datei desselben Namens hat in diesem Haus schon dreimal an
/// einem Tag wie ein Messergebnis ausgesehen.
struct Wegwerf {
    ordner: PathBuf,
}

impl Wegwerf {
    fn neu(name: &str) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static ZAEHLER: AtomicU32 = AtomicU32::new(0);
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let ordner = std::env::temp_dir().join(format!(
            "maer-sync-kennung-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir_all(&ordner).expect("Ordner anlegen");
        Self { ordner }
    }

    fn db(&self) -> String {
        self.ordner.join("store.sqlite").to_string_lossy().into_owned()
    }

    fn schluessel(&self) -> PathBuf {
        self.ordner.join("identity.key")
    }

    fn laden(&self) -> Store {
        Store::open(&self.db(), 10, None).expect("Speicher")
    }
}

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.ordner);
    }
}

fn ein_schluessel() -> [u8; 32] {
    let mut k = [0u8; 32];
    k.copy_from_slice(&zufall(32).expect("Zufall"));
    k
}

/// Nur für den einen Fall, der die Umgebung anfasst.
///
/// Eine Umgebungsvariable ist prozessweit, und die Zeugen laufen nebenläufig:
/// am 18.09. haben zwei Fälle ihren Nachbarn vergiftet, weil einer eine
/// Variable setzte, während der andere sie las.
fn allein() -> MutexGuard<'static, ()> {
    static SPERRE: OnceLock<Mutex<()>> = OnceLock::new();
    SPERRE.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
}

// -------------------------------------------------- der Schlüssel entsteht

#[test]
fn der_erste_start_legt_einen_schluessel_an_und_merkt_ihn() {
    let w = Wegwerf::neu("erster-start");
    let mut laden = w.laden();
    let (privat, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("Kennung");
    assert!(w.schluessel().exists(), "der erste Start legt die Datei an");
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(abdruck.as_str()),
               "und der Speicher merkt sich denselben Abdruck");
    assert_eq!(fingerprint(&public_of(&privat)).expect("Abdruck"), abdruck,
               "der gemeldete Abdruck gehört zum gelieferten Schlüssel");
}

#[test]
fn die_schluesseldatei_gehoert_nur_ihrem_eigentuemer() {
    use std::os::unix::fs::PermissionsExt;
    let w = Wegwerf::neu("rechte");
    let mut laden = w.laden();
    open_identity(&mut laden, &w.schluessel()).expect("Kennung");
    let rechte = std::fs::metadata(w.schluessel()).expect("Datei")
        .permissions().mode() & 0o777;
    assert_eq!(rechte, KEY_MODE,
               "ein Schlüssel, den die Welt eine Millisekunde lesen konnte, war lesbar");
}

#[test]
fn ein_zweiter_start_ersetzt_den_schluessel_nicht() {
    let w = Wegwerf::neu("zweiter-start");
    let mut laden = w.laden();
    let (erst, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("erster Start");
    let (wieder, abdruck_wieder) = open_identity(&mut laden, &w.schluessel())
        .expect("zweiter Start");
    assert_eq!(erst, wieder, "derselbe private Schlüssel");
    assert_eq!(abdruck, abdruck_wieder, "und derselbe Abdruck");
}

// ------------------------------------------- was auf der Platte nicht stimmt

#[test]
fn eine_geaenderte_schluesseldatei_weist_ab_und_laesst_das_gedaechtnis_in_ruhe() {
    let w = Wegwerf::neu("geaendert");
    let mut laden = w.laden();
    let (_, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("erster Start");

    write_key(&w.schluessel(), &ein_schluessel()).expect("fremden Schlüssel hinlegen");
    assert!(open_identity(&mut laden, &w.schluessel()).is_err(),
            "ein fremder Schlüssel darf nicht einfach bedient werden");
    // Der Teil, auf den es ankommt: überlebt das Gedächtnis die Weigerung
    // NICHT, sähe der zweite Versuch aus wie der freundliche Fall `remember`
    // und liesse den Fremden herein.
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(abdruck.as_str()),
               "die Weigerung darf das Gedächtnis nicht anfassen");
}

#[test]
fn eine_fehlende_schluesseldatei_weist_ab_statt_eine_neue_wachsen_zu_lassen() {
    let w = Wegwerf::neu("fehlend");
    let mut laden = w.laden();
    let (_, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("erster Start");
    std::fs::remove_file(w.schluessel()).expect("wegnehmen");

    assert!(open_identity(&mut laden, &w.schluessel()).is_err(),
            "ein Speicher, der einen Schlüssel erwartet, bekommt keinen neuen");
    assert!(!w.schluessel().exists(), "eine Weigerung darf keinen Schlüssel hinlegen");
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(abdruck.as_str()));
}

#[test]
fn eine_beschaedigte_datei_gilt_nicht_als_abwesenheit() {
    // Abwesend heisst „dieser Dienst ist neu". Eine beschädigte Datei heisst,
    // dass jemand einen Schlüssel hat und es nicht dieser ist, und neu
    // anzufangen würfe den echten weg.
    let w = Wegwerf::neu("beschaedigt");
    std::fs::write(w.schluessel(), "this is not a key\n").expect("schreiben");
    assert!(read_key(&w.schluessel()).is_err(), "kein stilles Nichts");
    assert_eq!(std::fs::read_to_string(w.schluessel()).expect("lesen"),
               "this is not a key\n",
               "und die beschädigte Datei bleibt liegen, wie sie war");
}

#[test]
fn ein_schluessel_falscher_laenge_wird_ebenfalls_abgewiesen() {
    // Lesbares base64 mit fünf Byte darin. Die Form stimmt, die Länge nicht,
    // und genau dieser Fall geht an einer reinen Formprüfung vorbei.
    let w = Wegwerf::neu("laenge");
    std::fs::write(w.schluessel(), "c2hvcnQ=\n").expect("schreiben");
    assert!(read_key(&w.schluessel()).is_err(), "32 Byte, oder gar keine Antwort");
}

#[test]
fn ein_speicher_neben_seinem_eigenen_schluessel_nimmt_ihn_ohne_frage_an() {
    // Die gewöhnliche Rückkehr: die Datenbank kommt aus einer Sicherung
    // zurück, die älter ist als das Gedächtnis, und die Maschine hält ihren
    // Schlüssel noch. Hier zu weigern hiesse den Fall zu brechen, für den
    // diese Konstruktion gebaut ist.
    let w = Wegwerf::neu("rueckkehr");
    let meiner = ein_schluessel();
    write_key(&w.schluessel(), &meiner).expect("hinlegen");
    let mut laden = w.laden();
    let (getragen, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("Kennung");
    assert_eq!(getragen, meiner, "der Schlüssel, der schon dalag");
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(abdruck.as_str()));
}

// ------------------------------------------- der Betreiber sagt, dass er will

#[test]
fn adopt_nimmt_den_schluessel_der_daliegt_und_sagt_was_vorher_war() {
    let w = Wegwerf::neu("adopt");
    let mut laden = w.laden();
    let (_, war) = open_identity(&mut laden, &w.schluessel()).expect("erster Start");
    write_key(&w.schluessel(), &ein_schluessel()).expect("anderen hinlegen");

    let (gefunden, vorher) = adopt(&mut laden, &w.schluessel()).expect("übernehmen");
    assert_eq!(vorher.as_deref(), Some(war.as_str()), "es sagt, was vorher galt");
    assert_ne!(gefunden, war, "und es ist ein anderer");
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(gefunden.as_str()));
    // Und der Dienst läuft wieder an, was der ganze Zweck des Befehls ist.
    let urteil = decide(Some(&gefunden), Some(&gefunden));
    assert!(urteil.may_serve, "nach dem Übernehmen darf bedient werden");
}

#[test]
fn adopt_macht_keinen_schluessel_still() {
    // Ein `adopt`, das erzeugt, wenn es nichts zu übernehmen gibt, wäre genau
    // die stille Ersetzung, gegen die das ganze Modul geschrieben ist.
    let w = Wegwerf::neu("adopt-leer");
    let mut laden = w.laden();
    assert!(adopt(&mut laden, &w.schluessel()).is_err(), "nichts da, nichts zu übernehmen");
    assert!(!w.schluessel().exists(), "und es hat auch keinen angelegt");
}

#[test]
fn new_macht_einen_und_behaelt_die_alte_datei_daneben() {
    let w = Wegwerf::neu("new");
    let mut laden = w.laden();
    let (_, war) = open_identity(&mut laden, &w.schluessel()).expect("erster Start");
    let vorher_text = std::fs::read_to_string(w.schluessel()).expect("lesen");

    let (gefunden, vorher, behalten) = make_new(&mut laden, &w.schluessel()).expect("drehen");
    assert_eq!(vorher.as_deref(), Some(war.as_str()));
    assert_ne!(gefunden, war);
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(gefunden.as_str()));
    // Eine versehentliche Drehung darf nicht unwiederbringlich sein.
    let behalten = behalten.expect("die alte Datei wurde behalten");
    assert_eq!(std::fs::read_to_string(&behalten).expect("lesen"), vorher_text,
               "und sie steht unverändert daneben");
}

#[test]
fn jede_drehung_behaelt_ihren_schluessel_nicht_nur_die_letzte() {
    // Der Fall, den die Python-Seite sich teuer erkauft hat: EINE Drehung
    // findet die alte Datei daneben und ist grün über einer Konstruktion mit
    // einem einzigen festen Namen. Die zweite überschreibt, was die erste
    // beiseitegelegt hat. Eine Drehung kann das nicht sehen, also zählt
    // dieser Zeuge sie.
    let w = Wegwerf::neu("drehungen");
    let mut laden = w.laden();
    open_identity(&mut laden, &w.schluessel()).expect("erster Start");

    let mut gesehen = std::collections::BTreeSet::new();
    gesehen.insert(std::fs::read_to_string(w.schluessel()).expect("lesen").trim().to_string());
    for _ in 0..3 {
        make_new(&mut laden, &w.schluessel()).expect("drehen");
        gesehen.insert(std::fs::read_to_string(w.schluessel()).expect("lesen").trim().to_string());
    }
    assert_eq!(gesehen.len(), 4, "vier Kennungen sind entstanden");

    let mut beiseite: Vec<PathBuf> = std::fs::read_dir(&w.ordner).expect("Ordner")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str())
                     .map(|n| n.contains(".was")).unwrap_or(false))
        .collect();
    beiseite.sort();
    assert_eq!(beiseite.len(), 3, "eine Drehung hat einen Schlüssel verloren: {beiseite:?}");

    let mut auf_der_platte: std::collections::BTreeSet<String> = beiseite.iter()
        .map(|p| std::fs::read_to_string(p).expect("lesen").trim().to_string())
        .collect();
    auf_der_platte.insert(std::fs::read_to_string(w.schluessel()).expect("lesen").trim().to_string());
    assert_eq!(auf_der_platte, gesehen, "jeder je bediente Schlüssel ist noch da");
}

#[test]
fn den_stand_zeigen_aendert_nichts() {
    // Eine Auskunft darf den Zustand nicht ändern, sonst kann niemand
    // nachsehen, ohne einzugreifen. Gemessen wird der Weg, den `key` ohne
    // Schalter geht: lesen, rechnen, urteilen.
    let w = Wegwerf::neu("zeigen");
    let mut laden = w.laden();
    let (_, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("erster Start");
    let roh = std::fs::read(w.schluessel()).expect("lesen");

    let privat = read_key(&w.schluessel()).expect("lesbar").expect("da");
    let gefunden = fingerprint(&public_of(&privat)).expect("Abdruck");
    let gemerkt = laden.remembered_key().expect("lesen");
    let urteil = decide(Some(&gefunden), gemerkt.as_deref());

    assert!(urteil.may_serve);
    assert_eq!(gefunden, abdruck);
    assert_eq!(std::fs::read(w.schluessel()).expect("lesen"), roh, "die Datei ist unberührt");
    assert_eq!(laden.remembered_key().expect("lesen").as_deref(), Some(abdruck.as_str()));
}

// --------------------------------- der Schlüssel liegt NICHT in der Datenbank

#[test]
fn der_private_schluessel_steht_nicht_im_speicher_und_nicht_in_seiner_kopie() {
    // Der Entscheid dieser Scheibe, gemessen statt behauptet. Jeder andere
    // Fall dieser Datei bestünde auch, wenn der private Schlüssel als Zeile
    // in der Datenbank läge. Dieser hier ist der Grund, warum er es nicht
    // tut: `backup` reicht die Datenbank dorthin, wo Sicherungen hingehen,
    // und ein privater Schlüssel darin lässt jeden, der eine alte Kopie hat,
    // als dieser Dienst antworten.
    let w = Wegwerf::neu("nicht-in-der-db");
    let kopie = w.ordner.join("kopie.sqlite").to_string_lossy().into_owned();
    let (privat, abdruck) = {
        let mut laden = w.laden();
        let (privat, abdruck) = open_identity(&mut laden, &w.schluessel()).expect("Kennung");
        laden.write("the-owner", "stands", b"{}", 1, "", None, false).expect("Schrieb");
        laden.backup(&kopie).expect("Kopie");
        (privat, abdruck)
    };

    let geschrieben = std::fs::read(w.schluessel()).expect("Schlüsseldatei");
    let geschrieben = geschrieben.strip_suffix(b"\n").unwrap_or(&geschrieben).to_vec();
    assert!(!geschrieben.is_empty(), "die geschriebene Form ist nicht leer");

    for pfad in [w.db(), kopie.clone()] {
        let inhalt = std::fs::read(&pfad).expect("Datei");
        assert!(!enthaelt(&inhalt, &privat), "der rohe private Schlüssel liegt in {pfad}");
        assert!(!enthaelt(&inhalt, &geschrieben),
                "die geschriebene Form des privaten Schlüssels liegt in {pfad}");
        // Der ABDRUCK steht drin, und das ist der Punkt: er benennt, ohne zu
        // beweisen, also darf das Gedächtnis reisen, wo der Schlüssel nicht darf.
        assert!(enthaelt(&inhalt, abdruck.as_bytes()), "der Abdruck fehlt in {pfad}");
    }
}

fn enthaelt(heuhaufen: &[u8], nadel: &[u8]) -> bool {
    if nadel.is_empty() || nadel.len() > heuhaufen.len() { return false; }
    heuhaufen.windows(nadel.len()).any(|f| f == nadel)
}

#[test]
fn die_schluesseldatei_wird_von_einem_sicherungs_muster_nicht_mitgenommen() {
    // `store.sqlite*` ist, wie die Sicherungszeile eines Betreibers aussieht.
    // Der Schlüssel darf ihr nicht ins Netz gehen, und deshalb heisst er
    // nicht wie die Datenbank.
    let _allein = allein();
    let w = Wegwerf::neu("glob");
    let db = w.db();

    // Gemessen wird die VORGABE. Eine gesetzte Umgebung liesse den Fall eine
    // andere Frage beantworten, also wird sie beiseitegelegt und danach
    // zurückgestellt.
    let genannt = std::env::var("MAER_SYNC_KEY").ok();
    std::env::remove_var("MAER_SYNC_KEY");
    let pfad = key_path(Some(&db));
    if let Some(wert) = genannt {
        std::env::set_var("MAER_SYNC_KEY", wert);
    }

    assert!(!pfad.to_string_lossy().starts_with(&db),
            "{} fiele einem Muster auf {db} zum Opfer", pfad.display());
    assert_eq!(pfad.parent(), Some(w.ordner.as_path()),
               "er liegt neben der Datenbank, nicht woanders");
}

// ------------------------------------------------ die geschriebene Form

#[test]
fn der_abdruck_ist_sechs_gruppen_zu_vier() {
    let gedruckt = fingerprint(&public_of(&ein_schluessel())).expect("Abdruck");
    let gruppen: Vec<&str> = gedruckt.split('-').collect();
    assert_eq!(gruppen.len(), 6, "sechs Gruppen: {gedruckt}");
    for g in &gruppen {
        assert_eq!(g.len(), 4, "vier Zeichen je Gruppe: {gedruckt}");
    }
}

#[test]
fn die_verwechselbaren_buchstaben_stehen_nicht_im_alphabet() {
    // Crockford lässt `I`, `L`, `O` und `U` weg, damit niemand eine Eins von
    // einem I und eine Null von einem O unterscheiden muss. Das Alphabet
    // selbst ist nicht öffentlich, also wird es am ERGEBNIS gemessen: viele
    // Abdrücke, und keiner trägt eines der vier Zeichen.
    const ERLAUBT: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut gesehen = std::collections::BTreeSet::new();
    for i in 0u8..64 {
        let gedruckt = fingerprint(&public_of(&[i; 32])).expect("Abdruck");
        for c in gedruckt.chars().filter(|c| *c != '-') {
            assert!(ERLAUBT.contains(c), "{c} steht nicht im Alphabet: {gedruckt}");
            gesehen.insert(c);
        }
    }
    for verwechselbar in ['I', 'L', 'O', 'U'] {
        assert!(!gesehen.contains(&verwechselbar),
                "{verwechselbar} ist in 64 Abdrücken aufgetaucht");
    }
    // Gegenprobe, sonst misst der Fall nur, dass die Schleife lief: die 64
    // Abdrücke müssen zusammen mehr als eine Handvoll Zeichen zeigen.
    assert!(gesehen.len() > 20, "nur {} verschiedene Zeichen gesehen", gesehen.len());
}
