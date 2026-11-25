// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The readers of Musopen (Wikimedia Commons) and Radio Browser against the
//! shared vectors, and their rules as tables. Pure: no request is made here.

use std::path::{Path, PathBuf};

use maer_sync::katalog::Row;
use maer_sync_abruf::musopen::{self, MusopenFehler};
use maer_sync_abruf::radio;
use serde_json::{json, Value};

fn root(q: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("vektoren").join("katalog").join(q)
}

fn cases(q: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root(q)).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

fn read(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn opt(v: &Value) -> Option<String> { v.as_str().map(str::to_string) }
fn strings(v: &Value) -> Vec<String> { v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() }

fn expected(v: &Value) -> Row {
    Row {
        quelle: v["quelle"].as_str().unwrap().into(), id: v["id"].as_str().unwrap().into(), art: v["art"].as_str().unwrap().into(),
        titel: opt(&v["titel"]), urheber: opt(&v["urheber"]), sprache: opt(&v["sprache"]), jahr: v["jahr"].as_i64(),
        rechte_kennung: v["rechte"]["kennung"].as_str().unwrap().into(), rechte_url: opt(&v["rechte"]["url"]),
        laden: v["laden"].as_bool().unwrap(), reihen: strings(&v["reihen"]), auch: strings(&v["auch"]),
        tief: if v["tief"].is_null() { None } else { Some(v["tief"].to_string()) },
    }
}

/// Compare rows with `tief` as JSON, not as text.
fn same(got: &[Row], want: &[Row]) -> bool {
    let norm = |r: &Row| { let mut r = r.clone(); r.tief = r.tief.as_deref().map(|t| serde_json::from_str::<Value>(t).unwrap().to_string()); r };
    got.iter().map(norm).collect::<Vec<_>>() == want.iter().map(norm).collect::<Vec<_>>()
}

// ---------------------------------------------------------------- Musopen

#[test]
fn musopen_rows_match_the_vectors() {
    let all = cases("musopen");
    assert_eq!(all.len(), 13);
    for dir in all {
        let a = std::fs::read(dir.join("antwort.json")).unwrap();
        let want: Vec<Row> = read(&dir.join("erwartet.json"))["zeilen"].as_array().unwrap().iter().map(expected).collect();
        let got = musopen::rows_from_answer(&a).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display())).zeilen;
        assert!(same(&got, &want), "{}:\n got  {got:?}\n want {want:?}", dir.display());
    }
}

fn info(mime: Option<&str>, url: &str, derivatives: Value, licence: &str) -> Value {
    json!({"mime": mime, "url": url, "derivatives": derivatives,
           "extmetadata": {"License": {"value": licence}, "LicenseShortName": {"value": licence}}})
}

/// The index takes exactly the files the app takes (iOS
/// `MusopenCatalog.recording`), in its order: a sound at all, then the
/// licence, then a form the player takes; and it plays what the app plays:
/// the MP3 Commons derives, else an MP3 original without its query.
#[test]
fn playable_form_as_the_app() {
    use musopen::Dropped::*;
    let mp3 = json!([{"src": "https://upload.wikimedia.org/t/a.ogg.mp3", "type": "audio/mpeg", "transcodekey": "mp3"}]);
    let only_ogg = json!([{"src": "https://upload.wikimedia.org/t/a.flac.ogg", "type": "audio/ogg; codecs=\"vorbis\"", "transcodekey": "ogg"}]);
    let video = json!([{"src": "https://upload.wikimedia.org/t/a.ogv.webm", "type": "video/webm; codecs=\"vp9\"", "transcodekey": "240p.vp9.webm"},
                       {"src": "https://upload.wikimedia.org/t/a.ogv.mp3", "type": "audio/mpeg", "transcodekey": "mp3"}]);
    let by_type = json!([{"src": "https://upload.wikimedia.org/t/b.mp3", "type": "audio/mpeg"}]);
    let o = "https://upload.wikimedia.org/o/a";
    let table: Vec<(Value, Result<&str, musopen::Dropped>)> = vec![
        (info(Some("application/ogg"), &format!("{o}.ogg"), mp3.clone(), "pd"), Ok("https://upload.wikimedia.org/t/a.ogg.mp3")),
        (info(Some("audio/flac"), &format!("{o}.flac"), mp3.clone(), "cc0"), Ok("https://upload.wikimedia.org/t/a.ogg.mp3")),
        (info(Some("audio/x-flac"), &format!("{o}.flac"), by_type, "cc0"), Ok("https://upload.wikimedia.org/t/b.mp3")),
        (info(Some("audio/mpeg"), &format!("{o}.mp3?utm_source=commons#t=2"), json!([]), "pd"), Ok("https://upload.wikimedia.org/o/a.mp3#t=2")),
        (info(Some("audio/mpeg"), &format!("{o}.mp3"), json!([]), "pd"), Ok("https://upload.wikimedia.org/o/a.mp3")),
        (info(Some("audio/flac"), &format!("{o}.flac"), only_ogg, "pd"), Err(NoPlayableForm)),
        (info(Some("audio/flac"), &format!("{o}.flac"), json!([]), "pd"), Err(NoPlayableForm)),
        (info(Some("application/ogg"), &format!("{o}.ogv"), video, "pd"), Err(NotAudio)),
        (info(Some("video/webm"), &format!("{o}.webm"), mp3.clone(), "pd"), Err(NotAudio)),
        (info(Some("image/png"), &format!("{o}.png"), json!([]), "pd"), Err(NotAudio)),
        (info(None, &format!("{o}.ogg"), mp3.clone(), "pd"), Err(NotAudio)),
        (info(Some("application/ogg"), &format!("{o}.ogg"), mp3.clone(), "cc-by-sa-3.0"), Err(Licence)),
        // Not a rule of the app: the address the device plays is on Commons.
        (info(Some("application/ogg"), &format!("{o}.ogg"), json!([{"src": "https://example.org/a.mp3", "type": "audio/mpeg", "transcodekey": "mp3"}]), "pd"), Err(ForeignHost)),
    ];
    for (vi, want) in table {
        assert_eq!(musopen::playable(&vi).as_deref().map_err(|e| *e), want, "{vi}");
    }
    // Every row of the vectors plays the address its answer derives.
    for dir in cases("musopen") {
        for r in musopen::rows_from_answer(&std::fs::read(dir.join("antwort.json")).unwrap()).unwrap().zeilen {
            let t: Value = serde_json::from_str(r.tief.as_deref().unwrap()).unwrap();
            assert!(t["spuren"][0]["adresse"].as_str().unwrap().ends_with(".mp3"), "{}: {t}", dir.display());
        }
    }
}

#[test]
fn musopen_licence_as_the_app() {
    let table: [(Option<&str>, Option<&str>, Option<&str>); 9] = [
        (Some("pd"), Some("Public domain"), Some("pd")),
        (Some("PD"), Some("Public Domain"), Some("pd")),
        (Some("cc0"), Some("CC0"), Some("cc0")),
        (None, Some("PDM-owner"), Some("pdm")),
        (Some(""), Some("PDM-owner"), Some("pdm")),
        (Some("cc-by-3.0"), Some("PDM-owner"), None),
        (Some("cc-by-sa-3.0"), Some("CC BY-SA 3.0"), None),
        (Some("attribution"), Some("Attribution"), None),
        (None, Some("FAL"), None),
    ];
    for (stated, short, mark) in table {
        assert_eq!(musopen::licence(stated, short), mark, "{stated:?} {short:?}");
    }
}

/// The cases of iOS `MovementNumberTests` (1d254baa).
#[test]
fn musopen_movement_number_as_the_app() {
    let table: [(&str, Option<i64>); 13] = [
        ("Suite - IX. Air", Some(9)),
        ("Suite - XII. Gigue", Some(12)),
        ("Suite - IV) Bourrée", Some(4)),
        ("Suite - 10. Air", Some(10)),
        ("Wiener Blut Op. 354", None),
        ("Traviata - Act 1 - Libiamo", None),
        ("Nocturne Op. 9, I. Allegro", Some(1)),
        ("String Quartet op. 95 - ii. largo", Some(2)),
        ("String Quartet op. 95, iv. allegretto agitato", Some(4)),
        ("Prelude, C. major", None),
        ("Bach, J. S. - Air", None),
        ("Suite in D major BWV 1068 - XII. Satz 12", Some(12)),
        ("Suite in D major BWV 1068 - 12. Satz 12", Some(12)),
    ];
    for (t, n) in table {
        assert_eq!(musopen::movement_number(t), n, "{t}");
    }
    assert_eq!(musopen::work_stem("String Quartet op. 95 - ii. largo"), musopen::work_stem("String Quartet op. 95 - iii. allegretto"));
    assert_eq!(musopen::work_stem("Nocturne Op. 9, I. Allegro"), musopen::work_stem("Nocturne Op. 9, II. Andante"));
    assert!(musopen::work_stem("Nocturne Op. 9, I. Allegro").is_some());
}

/// The cases of iOS `MusopenComposerTests.fieldForms` (1d254baa).
#[test]
fn musopen_composer_as_the_app() {
    let table: [(Option<&str>, &str, Option<&str>); 14] = [
        (Some("(See Musopen for author information)"), "Isaac Albeniz - suite espanola op. 47 - leyenda", Some("Isaac Albeniz")),
        (Some("(See Musopen for author information)"), "Jacques Ibert- cinq pieces en trio", None),
        (Some("Unknown authorUnknown author"), "Nocturne in E flat", None),
        (Some("Musopen Symphony Orchestra"), "Symphony No. 5 in C minor", None),
        (Some("The Modena Chamber Orchestra"), "Concerto", None),
        (Some("European Archives"), "A Midsummer Night's Dream", None),
        (Some("Composer: Frédéric Chopin; Performer: Paul Pitman"), "Nocturne Op. 9 No. 2", Some("Frédéric Chopin")),
        (Some("Performers: Ensemble Morales\n\nComposer: William Byrd (c.1539–1623)"), "Mass for Four Voices", Some("William Byrd")),
        (Some("J.S. Bach. Performer: Michel Rondeau"), "Prelude in C", None),
        (Some("Frank Lévy (performer)"), "Sonata", None),
        (Some("Grieg, Edvard; Musopen Symphony Orchestra"), "Peer Gynt", Some("Edvard Grieg")),
        (Some("Robert Schumann (1810-1856) (see Musopen for performance author information)"), "Kinderszenen", Some("Robert Schumann")),
        (Some("Johann Sebastian Bach"), "Bach, J S - Goldberg Variations, BWV988 - 01. Aria", Some("Johann Sebastian Bach")),
        (None, "Bach, J S - Goldberg Variations", Some("J S Bach")),
    ];
    for (field, title, want) in table {
        assert_eq!(musopen::composer(field, title).as_deref(), want, "{field:?} | {title}");
    }
    assert_eq!(musopen::composer(None, "La Traviata - Act 1 - Libiamo ne' lieti calici").as_deref(), Some("Giuseppe Verdi"));
}

/// The cases of iOS `MusopenMetadataTests.artistText` (1d254baa).
#[test]
fn musopen_artist_text_as_the_app() {
    assert_eq!(musopen::artist_text(Some("Unknown author<span style=\"display: none;\">Unknown author</span>")), None);
    assert_eq!(musopen::artist_text(Some("<a href=\"//commons.wikimedia.org/wiki/User:Morn\" title=\"User:Morn\">Morn</a>")), None);
    assert_eq!(musopen::artist_text(Some("<bdi><a href=\"https://en.wikipedia.org/wiki/en:Fr%C3%A9d%C3%A9ric_Chopin\">Frédéric Chopin</a></bdi> / <bdi>Frank Levy</bdi>")).as_deref(),
               Some("Frédéric Chopin; Frank Levy"));
    assert_eq!(musopen::artist_text(Some("European &amp; Archive")).as_deref(), Some("European & Archive"));
    assert_eq!(musopen::artist_text(None), None);
}

/// iOS `MusopenCatalog.shownTitle`: the object name the page states, else the
/// file name without its namespace and its extension; an object name with
/// markup is the rendering of a template, not a name.
#[test]
fn musopen_titles_as_the_app() {
    for (object, file, title) in [
        (Some("Oboe Concerto - I. Allegro"), "File:Albinoni, Concerto.ogg", "Oboe Concerto - I. Allegro"),
        (Some("  "), "File:A - I. B.ogg", "A - I. B"),
        (Some("<span>x</span>"), "File:y.flac", "y"),
        (None, "File:z.MP3", "z"),
        (None, "File:u.mid", "u"),
        (None, "File:Op. 9 no.2.ogg", "Op. 9 no.2"),
        (None, "File:noext", "noext"),
    ] {
        assert_eq!(musopen::shown_title(object, file), title);
    }
}

fn page(pageid: i64, title: &str, url: &str, licence: &str) -> Value {
    json!({"pageid": pageid, "ns": 6, "title": title, "videoinfo": [{"url": url, "mime": "application/ogg",
        "derivatives": [{"src": format!("{url}.mp3"), "type": "audio/mpeg", "transcodekey": "mp3"}],
        "extmetadata": {"License": {"value": licence}, "LicenseShortName": {"value": licence}}}]})
}

#[test]
fn musopen_a_malformed_answer_is_an_error() {
    for bad in [&b"<html>"[..], br#"{"batchcomplete": true}"#, br#"{"query": {"pages": [{"title": "File:x.ogg"}]}}"#] {
        assert!(matches!(musopen::rows_from_answer(bad), Err(MusopenFehler::Unreadable(_))), "{}", String::from_utf8_lossy(bad));
    }
    // A page without videoinfo is no media file: skipped, but counted as seen.
    let a = json!({"query": {"pages": [{"pageid": 1, "ns": 6, "title": "File:x.pdf"}, page(2, "File:y.ogg", "https://upload.wikimedia.org/y.ogg", "pd")]}});
    let s = musopen::rows_from_answer(a.to_string().as_bytes()).unwrap();
    assert_eq!((s.zeilen.len(), s.gesehen), (1, 2));
}

/// The app keeps the page of each recording on Commons
/// (`MusopenCatalog.Recording.pageURL`, from `descriptionurl`).
#[test]
fn musopen_page_of_the_recording() {
    let mut a: Value = serde_json::from_slice(&std::fs::read(root("musopen").join("normal-cc0").join("antwort.json")).unwrap()).unwrap();
    let want = a["query"]["pages"][0]["videoinfo"][0]["descriptionurl"].as_str().unwrap().to_string();
    let t = |a: &Value| -> Value {
        let r = musopen::rows_from_answer(a.to_string().as_bytes()).unwrap().zeilen;
        serde_json::from_str(r[0].tief.as_deref().unwrap()).unwrap()
    };
    assert_eq!(t(&a)["seite"], json!(want));
    a["query"]["pages"][0]["videoinfo"][0].as_object_mut().unwrap().remove("descriptionurl");
    assert_eq!(t(&a)["seite"], Value::Null, "no page stated, none in tief");
}

#[test]
fn musopen_maxlag_postpones() {
    let a = json!({"error": {"code": "maxlag", "info": "Waiting for a database server", "lag": 7}});
    assert_eq!(musopen::rows_from_answer(a.to_string().as_bytes()), Err(MusopenFehler::Maxlag(7)));
}

#[test]
fn musopen_continue_token() {
    let a = json!({"continue": {"gcmcontinue": "file|ABC|123", "continue": "gcmcontinue||"}, "query": {"pages": []}});
    assert_eq!(musopen::rows_from_answer(a.to_string().as_bytes()).unwrap().weiter.as_deref(), Some("file|ABC|123"));
}

// ---------------------------------------------------------------- Radio Browser

#[test]
fn radio_rows_match_the_vectors() {
    let all = cases("radio");
    assert_eq!(all.len(), 10);
    for dir in all {
        let a = std::fs::read(dir.join("antwort.json")).unwrap();
        let want: Vec<Row> = read(&dir.join("erwartet.json"))["zeilen"].as_array().unwrap().iter().map(expected).collect();
        let got = radio::rows_from_page(&a).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display())).zeilen;
        assert!(same(&got, &want), "{}:\n got  {got:?}\n want {want:?}", dir.display());
    }
}

#[test]
fn radio_a_malformed_page_is_an_error() {
    for bad in [&b"<html>"[..], br#"{"stations": []}"#, br#"[{"name": "no uuid", "url": "https://x"}]"#, br#"[{"stationuuid": "", "name": "x", "url": "https://x"}]"#] {
        assert!(radio::rows_from_page(bad).is_err(), "{}", String::from_utf8_lossy(bad));
    }
    let empty = radio::rows_from_page(b"[]").unwrap();
    assert_eq!((empty.zeilen.len(), empty.roh), (0, 0));
}

#[test]
fn radio_no_station_is_ever_loaded() {
    for dir in cases("radio") {
        for r in radio::rows_from_page(&std::fs::read(dir.join("antwort.json")).unwrap()).unwrap().zeilen {
            assert!(!r.laden && r.rechte_kennung == "unbekannt" && r.id.starts_with("radio:rb:") && r.art == "sender");
        }
    }
}

#[test]
fn radio_counts_the_raw_page_and_its_uuids() {
    // Dropped entries (empty name or address) still count for the paging and
    // the count against the statistics; a uuid twice in one page counts once.
    let a = std::fs::read(root("radio").join("doppelte-uuid").join("antwort.json")).unwrap();
    let s = radio::rows_from_page(&a).unwrap();
    assert_eq!((s.roh, s.uuids.len(), s.zeilen.len()), (2, 1, 1));
    let b = std::fs::read(root("radio").join("leerer-name").join("antwort.json")).unwrap();
    let s = radio::rows_from_page(&b).unwrap();
    assert_eq!((s.roh, s.uuids.len(), s.zeilen.len()), (1, 1, 0));
}

fn station(uuid: &str, name: &str, url: &str, resolved: &str) -> Value {
    json!({"stationuuid": uuid, "name": name, "url": url, "url_resolved": resolved, "languagecodes": ""})
}

/// iOS `RadioMapping.stations`: name and address lose spaces and tabs at
/// their ends, the address played is `url_resolved`, else `url`; an entry
/// without a name or an address is dropped before the uuid counts as seen.
#[test]
fn radio_name_and_address_as_the_app() {
    let page = |v: Value| radio::rows_from_page(v.to_string().as_bytes()).unwrap();
    let s = page(json!([station("a", "\tBlasmusikradio mit Bernd ", " https://x/a\t", "")]));
    assert_eq!(s.zeilen[0].titel.as_deref(), Some("Blasmusikradio mit Bernd"));
    let t: Value = serde_json::from_str(s.zeilen[0].tief.as_deref().unwrap()).unwrap();
    assert_eq!((t["url"].clone(), t["url_resolved"].clone()), (json!("https://x/a"), Value::Null));
    assert_eq!(page(json!([station("b", "B", "", "https://x/b")])).zeilen.len(), 1, "url_resolved alone plays");
    assert_eq!(page(json!([station("c", "C", " \t", "")])).zeilen.len(), 0, "an address of blanks is none");
    assert_eq!(page(json!([station("d", " \t", "https://x/d", "")])).zeilen.len(), 0, "a name of blanks is none");
    // The first entry of a uuid is unusable, the second is not: the app keeps the second.
    let s = page(json!([station("e", "", "https://x/e", ""), station("e", "E", "https://x/e", "")]));
    assert_eq!((s.zeilen.len(), s.uuids.len(), s.roh), (1, 1, 2));
    assert_eq!(s.zeilen[0].titel.as_deref(), Some("E"));
}

/// `URL(string:)` gives nil only for an empty string and for a host that
/// opens a bracket and never closes it (measured on macOS 27.2.1, the
/// behaviour since iOS 17); everything else becomes an address, encoded.
#[test]
fn radio_drops_only_what_url_string_refuses() {
    let rows = |url: &str| radio::rows_from_page(json!([station("u", "U", url, "")]).to_string().as_bytes()).unwrap().zeilen.len();
    for bad in ["http://[::1/stream", "https://user@[fe80::1/x", "http://[::1"] {
        assert_eq!(rows(bad), 0, "{bad}");
    }
    for good in ["http://[::1]/stream", "http://a b/c", "kein url", "http://", "http://x/[y]", "http://x/\u{1}y", "http://h\u{e4}user/"] {
        assert_eq!(rows(good), 1, "{good}");
    }
}
