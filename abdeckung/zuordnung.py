# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Die 108 Faelle auf FALL-Ebene: wer beantwortet sie auf der Rust-Seite.

    python3 abdeckung/zuordnung.py > abdeckung/108-faelle.json

**Warum das eine Handliste ist und trotzdem messbar.** Welcher Zeuge einen
Python-Fall beantwortet, ist ein Urteil und keine Messung; es gibt kein Muster,
das es ausrechnet. Die Liste traegt es deshalb je Fall, mit Grund. Messbar wird
sie durch den Zaun darueber (`tests/test_abdeckung.py`): er prueft, dass jeder
der 108 genau einmal vorkommt, dass kein Fall erfunden ist, dass **jeder
genannte Zeuge wirklich existiert**, und dass jedes »kein Gegenpart« einen
Grund traegt. Eine Handliste, die auf nichts zeigt, faellt damit auf.

Die vier Arten:

- `tafel`   der Fall steht in einer Vektortafel, die ein Rust-Zeuge abfaehrt
- `zeuge`   ein eigener Rust-Zeuge beantwortet ihn
- `draht`   er wird gegen das laufende Erzeugnis gefahren
- `keiner`  es gibt keinen Gegenpart, und der Grund steht dabei
"""

import json, sys

# fall -> (art, gegenpart, grund)
Z = {}

def z(modul, klasse, name, art, gegenpart, grund=""):
    Z[f"{modul}.{klasse}.{name}"] = {"art": art, "gegenpart": gegenpart, "grund": grund}

# ----------------------------------------------------------------- test_app
A = "test_app"
WEGE = "tests/wege.rs::jeder_gemessene_weg_antwortet_genauso"
DRAHT = "werkzeug/gegenprobe.py"
HORCH = "tests/horcher.rs::jeder_gemessene_fall_des_horchers_antwortet_genauso"

z(A, "NothingAnswersWithoutAToken", "test_every_document_path_is_a_401_without_a_token",
  "tafel", WEGE, "wege.json: 'Schriftstueck ohne Zeichen' und 'mit unbekanntem Zeichen'")
z(A, "NothingAnswersWithoutAToken", "test_a_write_without_a_token_never_reaches_the_store",
  "keiner", "",
  "die Tafel prueft die ANTWORT (401), nicht dass der Laden danach unberuehrt ist. "
  "Ein Zeuge dafuer fehlt und ist eine eigene Zeile wert")
z(A, "NothingAnswersWithoutAToken", "test_the_refusal_says_how_to_ask",
  "tafel", WEGE, "wege.json haelt die Kopfzeile www-authenticate: Bearer fest")
z(A, "NothingAnswersWithoutAToken", "test_health_answers_without_one_and_says_nothing_about_the_store",
  "tafel", WEGE, "wege.json: 'gesundheit GET' mit Koerper {ok:true}")
z(A, "NothingTheCallerWroteIsAnIdentity", "test_every_handler_is_handed_the_caller_that_the_store_named",
  "keiner", "",
  "der Python-Fall liest den eigenen QUELLTEXT (zaehlt Handler-Signaturen). Das ist "
  "eine Frage an die Gestalt der Python-Fassung, nicht an das Verhalten; die "
  "Rust-Fassung reicht den Besitzer als Wert durch, eine Signatur zum Zaehlen gibt es nicht")
z(A, "NothingTheCallerWroteIsAnIdentity", "test_the_listener_list_is_keyed_by_the_caller_too",
  "zeuge", "tests/hoerer.rs", "die Ansageliste ist nach Besitzer gefuehrt, eigene Vektoren")
z(A, "HandingOutAToken", "test_the_right_secret_gets_a_token_that_works",
  "tafel", WEGE, "wege.json: 'Zeichen, richtig' plus die Faelle, die es danach benutzen")
z(A, "HandingOutAToken", "test_a_wrong_secret_and_an_unknown_account_sound_identical",
  "tafel", WEGE, "wege.json: 'falsches Geheimnis' und 'Konto gibt es nicht', beide 401 mit demselben Satz")
z(A, "HandingOutAToken", "test_a_request_that_is_not_one_is_a_400_not_a_500",
  "tafel", WEGE, "wege.json: 'Zeichen ohne JSON' und 'ohne Geheimnis'")
z(A, "HandingOutAToken", "test_the_token_path_is_only_for_posting",
  "tafel", WEGE, "wege.json: 'Zeichen mit falscher Methode' 405")
z(A, "OneOwnerNeverSeesAnother", "test_a_document_is_invisible_to_the_other_account",
  "keiner", "",
  "die Tafel faehrt EINEN Besitzer. Die Trennung zweier Besitzer ist in Rust nur "
  "mittelbar gedeckt (der Laden schluesselt nach Besitzer, bezeugt in speicher.rs); "
  "ein Fall ueber die WEGE fehlt")
z(A, "OneOwnerNeverSeesAnother", "test_a_write_by_one_does_not_touch_the_other",
  "zeuge", "tests/speicher.rs", "der Laden trennt nach Besitzer, aber am Laden und nicht am Weg")
z(A, "OneOwnerNeverSeesAnother", "test_an_announcement_reaches_only_its_own_owner",
  "tafel", HORCH, "horcher.json: 'eine Ansage erreicht nur ihren eigenen Besitzer'")
z(A, "OneOwnerNeverSeesAnother", "test_a_revoked_token_stops_working",
  "keiner", "", "revoke_token ist portiert, aber kein Fall faehrt es ueber einen Weg")
z(A, "TheSocketNeedsATokenToo", "test_an_unauthorized_socket_is_closed_before_it_is_accepted",
  "tafel", HORCH, "horcher.json: 'ohne Zeichen wird vor dem Annehmen geschlossen', 4401")
z(A, "TheSocketNeedsATokenToo", "test_a_bad_token_is_closed_too",
  "tafel", HORCH, "horcher.json: 'ein unbekanntes Zeichen ebenso'")
for n, g in [
  ("test_a_document_that_is_not_there_is_a_404", "wege.json: 'Schriftstueck gibt es nicht'"),
  ("test_a_put_then_a_get_comes_back_byte_for_byte", "wege.json: Schrieb und 'Schriftstueck lesen' mit Koerper"),
  ("test_the_version_travels_on_both_ways", "wege.json haelt etag in beiden Richtungen fest"),
  ("test_a_scope_addresses_its_own_document", "wege.json: geraet-1 gegen geraet-2"),
  ("test_health_answers_without_a_document", "wege.json: 'gesundheit GET' auf leerem Laden"),
]:
    z(A, "FetchingAndPutting", n, "tafel", WEGE, g)
for n, g in [
  ("test_the_listing_names_every_scope_with_its_version", "wege.json: 'Liste' mit scope, version, format"),
  ("test_the_listing_carries_no_bodies", "wege.json: die Liste traegt nur Namen und Fassungen"),
  ("test_an_empty_kind_lists_nothing_and_is_not_a_404", "wege.json: 'Liste einer leeren Art' 200"),
  ("test_the_listing_needs_a_token_like_everything_else", "wege.json: die Liste liegt hinter der Zeichenpruefung"),
  ("test_the_listing_is_only_for_getting", "wege.json: 'Liste mit falscher Methode' 405"),
  ("test_a_listing_of_a_name_that_is_not_one_is_a_404", "wege.json: 'Art mit Grossbuchstaben ist kein Weg'"),
]:
    z(A, "ListingWhatOtherDevicesWrote", n, "tafel", WEGE, g)
PROT = "tests/protokoll.rs::jede_abweisung_hinterlaesst_die_zeile_der_python_fassung"
for n, g in [
  ("test_a_refused_write_leaves_a_line_with_who_what_and_why",
   "protokoll.json: 'ein Schrieb, den das Tor abweist', mit Quelle, Besitzer, Art, "
   "Bereich, Status und Grund"),
  ("test_a_failed_sign_in_leaves_a_line_with_the_source_address",
   "protokoll.json: 'eine Anmeldung mit falschem Geheimnis'; das Geheimnis steht in "
   "keiner Zeile, der Zeuge prueft es eigens"),
  ("test_an_unauthorized_request_leaves_a_line_too",
   "protokoll.json: 'mit einem Zeichen, das niemandem gehoert'; das Zeichen steht in "
   "keiner Zeile"),
  ("test_an_accepted_write_leaves_no_warning",
   "protokoll.json: 'ein angenommener Schrieb sagt NICHTS', null Zeilen"),
]:
    z(A, "TheOperatorSeesEveryRefusal", n, "tafel", PROT, g)
z(A, "TheOperatorSeesEveryRefusal", "test_no_log_statement_names_a_token_a_secret_or_a_body",
  "zeuge", "tests/protokoll.rs::keine_protokoll_anweisung_nennt_ein_zeichen_ein_geheimnis_oder_einen_koerper",
  "ein Zaun ueber dem QUELLTEXT, an ganzen Woertern und nach der QUELLE des Werts "
  "gefragt: Kopfzeilen und Koerper duerfen nie in eine Zeile, der Koerper darf "
  "gezaehlt werden")
z(A, "AHalfWriteNeverReplacesAWholeDocument", "test_a_write_that_breaks_off_mid_body_is_refused",
  "draht", "werkzeug/befehlsprobe.py und der Lauf am Erzeugnis",
  "nur am Draht sichtbar: angekuendigt 50, geschickt 7, dann Abbruch, 400")
z(A, "AHalfWriteNeverReplacesAWholeDocument", "test_a_body_shorter_than_announced_is_refused",
  "draht", DRAHT, "dieselbe Messung am laufenden Erzeugnis")
z(A, "AHalfWriteNeverReplacesAWholeDocument", "test_an_unreadable_body_never_replaces_what_is_held",
  "tafel", WEGE, "wege.json: 'Schrieb mit unlesbarem Koerper' 400")
z(A, "AHalfWriteNeverReplacesAWholeDocument", "test_an_empty_body_still_meets_its_own_gate",
  "tafel", WEGE, "wege.json: 'Schrieb mit leerem Koerper geht an den Laden'")
z(A, "TheHostIsProtectedFromAboveToo", "test_a_body_over_the_limit_is_a_413_and_never_reaches_the_store",
  "keiner", "", "die Grenze ist gebaut und im Quelltext bezeugt, aber kein Fall faehrt sie")
z(A, "TheHostIsProtectedFromAboveToo", "test_the_limit_is_read_from_the_environment",
  "keiner", "", "MAER_SYNC_MAX_BODY_BYTES wird gelesen, aber von keinem Fall gefahren")
z(A, "TheHostIsProtectedFromAboveToo", "test_a_scope_with_an_encoded_colon_finds_its_own_document",
  "keiner", "", "die Prozent-Entschluesselung ist gebaut (frage_lesen), aber kein Fall faehrt sie")
z(A, "TheHostIsProtectedFromAboveToo", "test_a_header_that_is_not_utf8_is_a_401_not_a_crash",
  "keiner", "",
  "in Rust kann eine Kopfzeile den Typ String gar nicht verletzen; der Transport "
  "entscheidet, was mit nicht-UTF-8 geschieht, und das ist nicht gemessen")
for n, g in [
  ("test_an_unknown_path_is_a_404", "wege.json: 'unbekannter Weg'"),
  ("test_a_kind_that_is_not_a_name_is_refused_as_a_path", "wege.json: 'Art mit Grossbuchstaben'"),
  ("test_a_missing_document_says_something_else_than_a_missing_path",
   "wege.json haelt 'no such path' gegen 'no such document'"),
  ("test_a_scope_that_is_not_a_scope_is_a_400", "wege.json: 'kaputter Bereich'"),
  ("test_a_write_without_a_format_is_a_400", "wege.json: 'Schrieb ohne Formatangabe'"),
  ("test_a_method_the_path_is_not_for_is_a_405", "wege.json: die drei 405-Faelle"),
]:
    z(A, "WhatTheShellRefuses", n, "tafel", WEGE, g)
for n, g in [
  ("test_a_blind_overwrite_is_a_412_and_hands_back_the_version", "schreiben.json: admit, 412"),
  ("test_an_empty_state_over_a_full_one_is_a_409", "schreiben.json: der leere Stand"),
  ("test_a_declared_shrink_goes_through", "schreiben.json: declared_shrink"),
  ("test_an_older_format_is_a_409", "schreiben.json: das Format-Tor"),
  ("test_every_refusal_says_why", "schreiben.json traegt je Fall den Satz"),
]:
    z(A, "TheGatesAnswerThroughHttp", n, "tafel", "tests/schreiben.rs",
      g + " (am TOR, nicht ueber HTTP: der Weg dorthin ist in wege.json)")
z(A, "AnnouncingAChange", "test_a_listener_hears_an_accepted_write",
  "tafel", HORCH, "horcher.json: 'ein Horcher mit Zeichen hoert seine Ansage'")
z(A, "AnnouncingAChange", "test_a_listener_that_left_is_dropped_not_retried",
  "zeuge", "tests/horcher.rs::ein_getrennter_horcher_steht_nicht_mehr_auf_der_liste", "")
z(A, "AnnouncingAChange", "test_a_refused_write_announces_nothing",
  "tafel", WEGE, "wege.json: die abgewiesenen Schriebe tragen keine Ansage, der Zeuge prueft das eigens")
z(A, "AnnouncingAChange", "test_a_socket_on_another_path_is_closed",
  "tafel", HORCH, "horcher.json: 'ein anderer Weg ist kein Horcher', 4004")

# ----------------------------------------------------------------- test_cli
C = "test_cli"
BEF = "werkzeug/befehlsprobe.py gegen vektoren/befehle.json"
z(C, "TheParserNeverServesByAccident", "test_help_asks_to_print_and_exits_rather_than_serving",
  "keiner", "", "die Rust-Fassung hat kein --help; ein unbekannter Befehl nennt die Form und gibt 2")
z(C, "TheParserNeverServesByAccident", "test_no_command_is_serve", "zeuge",
  "src/main.rs", "ohne Befehl faellt die Verteilung in den Dienst; kein eigener Fall, nur gebaut")
z(C, "TheParserNeverServesByAccident", "test_the_commands_route_to_their_names", "tafel", BEF,
  "befehle.json faehrt alle vier Befehle unter ihren Namen")
z(C, "TheParserNeverServesByAccident", "test_showing_the_key_is_the_form_without_a_flag",
  "tafel", BEF, "befehle.json: 'key' ohne Schalter zeigt und aendert nichts")
z(C, "TheParserNeverServesByAccident", "test_backup_without_a_path_is_refused_not_served",
  "tafel", BEF, "befehle.json: 'backup' ohne Ziel, Ausgang 2")
z(C, "RemovingAnAccountThroughTheCommand", "test_account_remove_takes_the_account_away",
  "tafel", BEF, "befehle.json: 'account-remove nimmt es weg'")
z(C, "RemovingAnAccountThroughTheCommand", "test_removing_an_absent_account_says_so_and_still_succeeds",
  "tafel", BEF, "befehle.json: 'account-remove auf eines, das es nicht gibt', Ausgang 0")

# -------------------------------------------------------- test_session_store
S = "test_session_store"
SL = "tests/sitzungsladen.rs"
for n, g, f in [
  ("test_a_message_sealed_by_the_device_opens_here",
   "sitzungsladen.rs: zwei_sitzungen_teilen_keine_schluessel siegelt und entsiegelt "
   "in der eigenen Sitzung", "zwei_sitzungen_teilen_keine_schluessel"),
  ("test_an_answer_sealed_here_opens_at_the_device",
   "der Tunnel entsiegelt die Antwort am Geraet", "der_tunnel_traegt_dieselben_antworten"),
  ("test_two_sessions_do_not_share_keys",
   "ein Brief der zweiten Sitzung geht in der ersten NICHT auf",
   "zwei_sitzungen_teilen_keine_schluessel"),
  ("test_a_different_prologue_never_becomes_a_session",
   "ein Handschlag mit fremdem Prologue wird keine Sitzung und hinterlaesst nichts",
   "ein_abweichender_prologue_wird_nie_eine_sitzung"),
  ("test_a_failed_handshake_costs_the_store_nothing",
   "vier Arten, kein Handschlag zu sein, und der Laden bleibt leer",
   "ein_gescheiterter_handschlag_kostet_den_laden_nichts"),
]:
    ziel = ("tests/wege.rs::der_tunnel_traegt_dieselben_antworten"
            if f == "der_tunnel_traegt_dieselben_antworten" else f"{SL}::{f}")
    z(S, "AHandshakeBecomesASession", n, "zeuge", ziel, g)
for n, g, f in [
  ("test_a_name_nobody_made_is_unknown", "unseal, seal und is_live auf einem erfundenen Namen",
   "ein_name_den_niemand_gemacht_hat_ist_unbekannt"),
  ("test_a_session_that_went_quiet_is_unknown", "mit gestellter Uhr ueber die Frist hinaus",
   "eine_beendete_sitzung_wird_auf_der_stelle_fallen_gelassen"),
  ("test_an_ended_session_is_dropped_on_the_spot",
   "sie ist weg, ohne dass jemand gefegt hat",
   "eine_beendete_sitzung_wird_auf_der_stelle_fallen_gelassen"),
  ("test_noise_under_a_known_name_does_not_keep_it_alive",
   "der Zeuge steht da, WEIL eine Rot-Probe gruen blieb: der Zeitpunkt der Benutzung "
   "rueckt nur bei Erfolg", "rauschen_haelt_die_sitzung_nicht_am_leben"),
  ("test_a_real_message_does_keep_it_alive", "die echte Nachricht stellt die Frist neu",
   "eine_echte_nachricht_haelt_die_sitzung_am_leben"),
  ("test_a_handshake_in_use_stays_alive_inside_its_minute",
   "dieselbe Messung mit gestellter Uhr innerhalb der Frist",
   "eine_echte_nachricht_haelt_die_sitzung_am_leben"),
]:
    z(S, "TheWaysOfNotBeingASession", n, "zeuge", f"{SL}::{f}", g)
for n, g, f in [
  ("test_a_flood_cannot_grow_it", "vierzig Handschlaege gegen eine Decke von vier",
   "eine_flut_laesst_den_laden_nicht_wachsen"),
  ("test_a_flood_cannot_push_out_a_proven_session",
   "die bewiesene Sitzung ueberlebt zwanzig Handschlaege",
   "eine_flut_drueckt_keine_bewiesene_sitzung_hinaus"),
  ("test_proving_needs_a_live_session", "auf eine tote und auf eine erfundene: Fehler",
   "ein_beweis_braucht_eine_lebende_sitzung"),
]:
    z(S, "TheStoreStaysBounded", n, "zeuge", f"{SL}::{f}", g)
z(S, "TheStoreStaysBounded", "test_sweeping_says_how_many_it_dropped", "tafel",
  "tests/sitzungen.rs", "sitzungen.json: sweep gibt die Liste der Gefallenen")
z(S, "TheNameIsUnguessable", "test_two_sessions_never_share_a_name", "zeuge",
  f"{SL}::zwei_sitzungen_teilen_nie_einen_namen",
  "vierundzwanzig Namen, alle verschieden, alle 32 Hex-Zeichen")

# -------------------------------------------------------- test_session_paths
P = "test_session_paths"
z(P, "TheTwoPathsNeedNoToken", "test_the_key_is_handed_out_without_one", "tafel", WEGE,
  "wege.json: 'Schluessel mit Kennung' ohne Zeichen, 200")
z(P, "TheTwoPathsNeedNoToken", "test_every_other_path_still_wants_one", "tafel", WEGE,
  "wege.json: die 401-Faelle")
z(P, "TheTwoPathsNeedNoToken", "test_a_handshake_becomes_a_session_without_one", "tafel",
  "tests/wege.rs::der_tunnel_traegt_dieselben_antworten", "der Handschlag im Tunnel laeuft ohne Zeichen")
z(P, "TheWaysOfNotBeingAHandshake", "test_rubbish_is_one_answer_and_says_nothing_else", "tafel", WEGE,
  "wege.json: 'Sitzung, kein Handschlag', 400 mit EINEM Satz")
z(P, "TheWaysOfNotBeingAHandshake", "test_an_empty_body_is_refused_too", "keiner", "",
  "kein Fall mit leerem Koerper auf /v1/session")
z(P, "TheWaysOfNotBeingAHandshake", "test_a_handshake_for_a_different_key_is_refused", "keiner", "",
  "kein Fall; der Handschlag gegen einen fremden statischen Schluessel fehlt")
z(P, "TheWaysOfNotBeingAHandshake", "test_the_wrong_method_is_not_a_handshake", "tafel", WEGE,
  "wege.json: 'Sitzung mit falscher Methode', 405")
z(P, "AServiceWithoutAKeyAnswersHonestly", "test_the_key_path_says_there_is_no_key", "tafel", WEGE,
  "wege.json: 'Schluessel ohne Kennung', 503")
z(P, "AServiceWithoutAKeyAnswersHonestly", "test_the_session_path_says_the_same", "tafel", WEGE,
  "wege.json: 'Sitzung ohne Kennung', 503")

# -------------------------------------------------------------- test_tunnel
T = "test_tunnel"
TUN = "tests/wege.rs::der_tunnel_traegt_dieselben_antworten"
z(T, "TheInsideIsTheOrdinaryWire", "test_a_sign_in_a_write_a_read_and_a_listing_all_work_inside",
  "tafel", TUN, "wege.json, Tunnel: Anmeldung, Schrieb, Lesen; eine LISTE innen fehlt")
z(T, "TheInsideIsTheOrdinaryWire", "test_the_gates_inside_are_the_gates_outside", "tafel", TUN,
  "wege.json, Tunnel: 'Schriftstueck ohne Zeichen bleibt 401'")
z(T, "TheInsideIsTheOrdinaryWire", "test_a_token_from_inside_works_outside_and_the_other_way_round",
  "keiner", "", "kein Fall reicht ein innen geholtes Zeichen nach aussen")
z(T, "TheInsideIsTheOrdinaryWire", "test_the_session_is_proven_by_a_sign_in_and_outlives_the_short_idle",
  "keiner", "", "prove wird gerufen, aber kein Fall misst die laengere Frist danach")
z(T, "WhatIsNotARequestInside", "test_an_ended_session_is_410_and_a_new_handshake_mends_it",
  "tafel", TUN, "wege.json, Tunnel: 410; das Heilen durch einen neuen Handschlag fehlt")
z(T, "WhatIsNotARequestInside", "test_a_made_up_name_is_410_too", "tafel", TUN,
  "wege.json, Tunnel: 'eine Sitzung, die es nicht gibt'")
z(T, "WhatIsNotARequestInside", "test_noise_under_a_real_name_is_400_and_the_session_survives_it",
  "zeuge", "tests/wege.rs::rauschen_unter_einem_namen_kostet_die_sitzung_nichts", "")
z(T, "WhatIsNotARequestInside", "test_a_sealed_thing_that_is_not_a_request_is_400", "tafel", TUN,
  "wege.json, Tunnel: 'innen: kein Rahmen'")
z(T, "WhatIsNotARequestInside", "test_the_session_does_not_nest_and_the_key_is_not_asked_inside",
  "zeuge", "tests/wege.rs::eine_weigerung_der_aeusseren_schicht_reist_unversiegelt", "")
z(T, "ThePlainWaysCanBeTurnedOff", "test_a_plain_request_is_told_to_come_inside", "tafel", WEGE,
  "wege.json: die vier 426-Faelle")
z(T, "ThePlainWaysCanBeTurnedOff", "test_the_session_paths_and_the_health_route_still_answer",
  "tafel", WEGE, "wege.json: 'gesundheit auch bei geschlossenen Wegen' und 'Schluessel auch bei'")
z(T, "ThePlainWaysCanBeTurnedOff", "test_the_switch_reads_the_environment", "keiner", "",
  "MAER_SYNC_PLAIN_WAYS wird gelesen, aber kein Fall faehrt die Umgebung")
z(T, "TheSocketInsideASession", "test_the_first_frame_carries_the_token_and_announcements_come_back_sealed",
  "tafel", HORCH, "horcher.json: 'versiegelt: erst der Rahmen mit Zeichen, dann die Ansage'")
z(T, "TheSocketInsideASession", "test_a_first_frame_without_a_token_closes_after_the_accept",
  "tafel", HORCH, "horcher.json: 'der erste Rahmen ohne Zeichen', 4401 NACH dem Annehmen")
z(T, "TheSocketInsideASession", "test_a_made_up_name_is_turned_away_before_the_accept",
  "tafel", HORCH, "horcher.json: 'eine Sitzung, die es nicht gibt', 4410 vor dem Annehmen")
z(T, "TheSocketInsideASession", "test_a_first_frame_that_is_not_sealed_closes", "tafel", HORCH,
  "horcher.json: 'das erste Wort ist nicht versiegelt', 4400")

# ----------------------------------------------------------- test_rehearsal
R = "test_rehearsal"
z(R, "ABackupThatComesBack", "test_every_version_of_every_document_is_in_the_copy", "tafel",
  "werkzeug/befehlsprobe.py gegen vektoren/befehle.json",
  "befehle.json: 'backup mit einem Dokument', fits mit Zahl und Groesse")
z(R, "ABackupThatComesBack", "test_the_accounts_travel_with_the_copy", "keiner", "",
  "die Probe vergleicht DOKUMENTE, nicht Konten; kein Fall prueft, dass die Konten mitreisen")
z(R, "ABackupThatCannotBeJudged", "test_an_empty_store_is_not_measured_not_green", "tafel",
  "werkzeug/befehlsprobe.py gegen vektoren/befehle.json",
  "befehle.json: 'backup mit Speicher', not_measured und Ausgang 1")
z(R, "ABackupThatCannotBeJudged", "test_a_copy_that_cannot_be_written_is_not_measured_and_says_why",
  "tafel", "werkzeug/befehlsprobe.py gegen vektoren/befehle.json",
  "befehle.json: 'backup in ein Verzeichnis, das es nicht gibt'")
z(R, "ABackupThatDiffers", "test_a_copy_that_lost_a_document_on_the_way_differs", "tafel",
  "tests/rueckkehr.rs", "rueckkehr.json haelt das Urteil differs; die DATEI-Haelfte dazu fehlt")

print(json.dumps({
    "was": "Die 108 Faelle aus der Gruppe, die auf Entscheide wartete, auf FALL-Ebene.",
    "wie": "Je Fall ein Urteil mit Grund. Ein Urteil ist keine Messung; messbar macht es "
           "der Zaun in tests/test_abdeckung.py, der jeden genannten Zeugen gegen die "
           "Wirklichkeit haelt.",
    "arten": {"tafel": "steht in einer Vektortafel, die ein Rust-Zeuge abfaehrt",
              "zeuge": "ein eigener Rust-Zeuge",
              "draht": "gegen das laufende Erzeugnis gefahren",
              "keiner": "kein Gegenpart, Grund dabei"},
    "faelle": [{"fall": k, **v} for k, v in sorted(Z.items())]},
    indent=2, ensure_ascii=False))
