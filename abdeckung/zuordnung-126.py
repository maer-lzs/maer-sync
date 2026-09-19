# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS

"""Die 126 Faelle auf FALL-Ebene: wer beantwortet sie auf der Rust-Seite.

    python3 abdeckung/zuordnung-126.py > abdeckung/126-faelle.json

Die 126 sind die Gruppe, die in der Zaehlung der 317 als „reines Modul mit
Gegenpart und eigenen Vektoren" dastand. Diese Zeile war wahr und irrefuehrend
zugleich, und das stand schon damals dabei: gedeckt war das MODUL, nicht der
einzelne Fall. Die Zaehlung hier loest das ein und hat dabei drei Sorten
Luecke gefunden.

**Die grosse:** `src/kennung.rs` hatte keinen einzigen Rust-Zeugen. Das Modul
traegt jede Stelle, an der ein privater Dienst-Schluessel entsteht, gelesen
oder ersetzt wird. Bezeugt war davon nur, was vier CLI-Faelle am Draht
mitnehmen. Aus „`identity` hat einen Gegenpart" las sich „die Kennung ist
gedeckt"; gedeckt war das RECHNEN, nicht das SCHREIBEN.

**Die zweite:** jede Tafel faehrt ihre Faelle mit der VORGABE. Ein Boden, den
ein Betreiber wirklich verstellt, war nirgends gefahren, und ob `decide` ihn
ueberhaupt weiterreicht, stand nirgends.

**Die dritte:** ein Vektor bringt feste Schluessel mit. Was daran haengt, dass
sie FRISCH sind, kann er nicht messen: zwei Handschlaege nebeneinander, und
der Zaehler, der verhindert, dass ein Nonce zweimal gilt.

Form und Zaun sind dieselben wie bei den 108 (`abdeckung/zuordnung.py`). Die
vier Arten:

- `tafel`   der Fall steht in einer Vektortafel, die ein Rust-Zeuge abfaehrt
- `zeuge`   ein eigener Rust-Zeuge beantwortet ihn
- `draht`   er wird gegen das laufende Erzeugnis gefahren
- `keiner`  es gibt keinen Gegenpart, und der Grund steht dabei
"""

import json

# fall -> (art, gegenpart, grund)
Z = {}


def z(modul, klasse, name, art, gegenpart, grund=""):
    Z[f"{modul}.{klasse}.{name}"] = {"art": art, "gegenpart": gegenpart, "grund": grund}


TOR = "tests/schreiben.rs::das_groessen_tor_urteilt_wie_die_python_seite"
FASS = "tests/schreiben.rs::die_beiden_fassungs_tore_urteilen_wie_die_python_seite"
SCHRIEB = "tests/schreiben.rs::ein_schrieb_wird_beurteilt_wie_von_der_python_seite"
ROT = "tests/schreiben.rs::fassungen_rotieren_wie_die_python_seite"
GRUPPEN = "tests/schreiben.rs::die_vektordatei_traegt_alle_gruppen"
RAHMEN = "tests/vektoren.rs::rahmen_entstehen_byte_gleich"
KOPF = "tests/kopf_und_schluessel.rs"
STAND = "tests/sitzungen.rs::der_stand_einer_sitzung_ist_der_gemessene"
WEICHEN = "tests/sitzungen.rs::unbewiesene_weichen_zuerst"
FEGEN = "tests/sitzungen.rs::gefegt_wird_was_nicht_lebt"
PLATZ = "tests/sitzungen.rs::eine_tote_sitzung_kostet_keine_lebende_ihren_platz"
ZAHLEN = "tests/sitzungen.rs::die_zahlen_sind_auf_beiden_seiten_dieselben"
RUECK = "tests/rueckkehr.rs::jeder_fall_urteilt_wie_die_python_seite_und_sagt_denselben_satz"

# ------------------------------------------------------------ test_admission
A = "test_admission"
z(A, "EveryVerdictCarriesItsSentence", "test_no_reason_is_empty", "tafel", TOR,
  "schreiben.json haelt je Fall den SATZ und nicht nur das Urteil; ein stummes "
  "Urteil faellt dort auf, weil verglichen und nicht auf Laenge geprueft wird")
z(A, "EveryVerdictCarriesItsSentence", "test_the_default_floor_is_the_one_the_tests_assume",
  "zeuge", GRUPPEN,
  "die Konstante wird gegen den Wert der Tafel gehalten, also gegen die Zahl der "
  "Python-Seite und nicht gegen sich selbst")
z(A, "TheCollapseShortOfEmpty", "test_a_declared_shrink_passes_and_still_says_what_happened",
  "tafel", TOR, "schreiben.json, admit: 100 auf 40 mit declared_shrink, Satz nennt die 40%")
z(A, "TheCollapseShortOfEmpty", "test_a_state_at_the_floor_is_taken", "tafel", TOR,
  "schreiben.json, admit: 100 auf 50, genau am Boden, und 100 auf 51 daneben")
z(A, "TheCollapseShortOfEmpty", "test_a_state_below_the_floor_is_refused", "tafel", TOR,
  "schreiben.json, admit: 100 auf 40, dazu 3 auf 1 und 7 auf 3 fuer die Rundung "
  "und 1000 auf 499 fuer den Rand")
z(A, "TheCollapseShortOfEmpty", "test_growth_and_standing_still_pass", "tafel", TOR,
  "schreiben.json, admit: 100 auf 100 und 100 auf 250")
z(A, "TheCollapseShortOfEmpty", "test_the_floor_is_a_parameter_not_a_law_of_nature",
  "zeuge", "tests/schreiben.rs::der_boden_ist_ein_wert_und_kein_naturgesetz",
  "LUECKE AUS DIESER ZAEHLUNG: die Tafel faehrt ausschliesslich die Vorgabe, ein "
  "verstellter Boden war unbezeugt. Der Zeuge faehrt 0.1 und 0.95 und haelt die "
  "Vorgabe als Gegenprobe daneben")
z(A, "TheEmptyWriteOverAFullState", "test_an_empty_state_never_replaces_a_full_one",
  "tafel", TOR,
  "schreiben.json, admit: 100 auf 0; der Satz nennt die Zahl UND das Wort empty, "
  "sonst antwortete der Einbruchs-Zweig fuer den bekannten Verlustfall mit")
z(A, "TheEmptyWriteOverAFullState", "test_an_empty_state_over_an_empty_one_loses_nothing",
  "zeuge", "tests/schreiben.rs::ein_leerer_stand_ueber_einem_leeren_verliert_nichts",
  "LUECKE AUS DIESER ZAEHLUNG: die Tafel kennt 0 auf 50, aber nicht 0 auf 0. Wer "
  "das leere EINGEHENDE zuerst prueft, verteidigt hier einen leeren Speicher gegen "
  "seinen einzigen Benutzer")
z(A, "TheEmptyWriteOverAFullState", "test_the_client_may_say_it_meant_it", "tafel", TOR,
  "schreiben.json, admit: 100 auf 0 mit declared_shrink, Satz nennt declared und empty")
z(A, "TheEmptyWriteOverAFullState", "test_the_first_state_is_always_taken", "tafel", TOR,
  "schreiben.json, admit: null auf 0 und null auf 100")
z(A, "TheEmptyWriteOverAFullState", "test_the_shrink_refusal_does_not_borrow_the_empty_words",
  "tafel", TOR,
  "die andere Haelfte derselben Frage: der Einbruchs-Fall traegt in der Tafel den "
  "Satz shrank und nicht empty, und verglichen wird der ganze Satz")
z(A, "TheGateRefusesNonsenseLoudly", "test_a_floor_outside_the_range_raises", "zeuge", TOR,
  "derselbe Zeuge faehrt 0.0, -0.5 und 1.5 ausdruecklich: ein Boden ausserhalb "
  "(0,1] ist ein Rufer-Fehler und keine Vorliebe")
z(A, "TheGateRefusesNonsenseLoudly", "test_a_missing_or_negative_size_raises", "tafel", TOR,
  "schreiben.json, admit_fehler: eine negative eingehende und eine negative "
  "gespeicherte Groesse")

# -------------------------------------------------------------- test_formats
F = "test_formats"
z(F, "AnOlderClientDoesNotOverwriteANewerDocument", "test_an_older_writer_is_refused",
  "tafel", FASS, "schreiben.json, write_verdict: 1 ueber 2 und 5 ueber 9, beide mit Satz")
z(F, "AnOlderClientDoesNotOverwriteANewerDocument", "test_the_same_format_writes",
  "tafel", FASS, "schreiben.json, write_verdict: 1 ueber 1")
z(F, "AnOlderClientDoesNotOverwriteANewerDocument", "test_a_newer_writer_writes",
  "tafel", FASS, "schreiben.json, write_verdict: 2 ueber 1 und 9 ueber 5")
z(F, "AnOlderClientDoesNotOverwriteANewerDocument",
  "test_the_first_document_has_nothing_to_be_older_than", "tafel", FASS,
  "schreiben.json, write_verdict: null als gespeicherte Fassung")
z(F, "AnOlderClientDoesNotOverwriteANewerDocument", "test_a_writer_without_a_format_raises",
  "zeuge", "tests/schreiben.rs::die_zwei_zahlen_sind_nicht_dieselbe_zahl",
  "eine fehlende Schreiberfassung NEBEN einer vorhandenen gespeicherten; sie still "
  "als 1 zu lesen liesse jeden Klienten, der das Feld vergessen hat, alles "
  "ueberschreiben")
z(F, "ARolledBackServiceDoesNotDamageItsStore", "test_a_store_ahead_of_the_service_is_refused",
  "tafel", FASS, "schreiben.json, serve_verdict: Speicher 2, Dienst 1, Satz nennt backwards")
z(F, "ARolledBackServiceDoesNotDamageItsStore", "test_a_store_at_or_below_the_service_opens",
  "tafel", FASS, "schreiben.json, serve_verdict: 1 zu 1, 1 zu 2 und 3 zu 3")
z(F, "ARolledBackServiceDoesNotDamageItsStore",
  "test_an_empty_store_is_laid_out_at_the_running_schema", "tafel", FASS,
  "schreiben.json, serve_verdict: leerer Speicher, Satz nennt die Gestalt des Dienstes")
z(F, "ARolledBackServiceDoesNotDamageItsStore", "test_a_nonsense_schema_raises", "zeuge", FASS,
  "derselbe Zeuge haelt die Gegenrichtung ausdruecklich fest: eine fehlende oder "
  "unmoegliche Nummer ist ein Fehler und kein stilles Ja")
z(F, "TheTwoNumbersAreNotTheSameNumber",
  "test_a_document_format_far_ahead_says_nothing_about_serving", "zeuge",
  "tests/schreiben.rs::die_zwei_zahlen_sind_nicht_dieselbe_zahl",
  "LUECKE AUS DIESER ZAEHLUNG: die Tafel prueft jede Zahl einzeln, nie beide "
  "gegeneinander. Wuerden sie verwechselt, saehe ein Fassungssprung des Klienten "
  "aus wie eine Wanderung des Speichers")
z(F, "TheTwoNumbersAreNotTheSameNumber",
  "test_a_store_schema_far_ahead_says_nothing_about_writing", "zeuge",
  "tests/schreiben.rs::die_zwei_zahlen_sind_nicht_dieselbe_zahl",
  "die andere Richtung derselben Frage, im selben Zeugen")

# --------------------------------------------------------------- test_frames
R = "test_frames"
z(R, "TheFrame", "test_a_head_and_a_body_come_back_as_they_went_in", "tafel", RAHMEN,
  "rahmen.json, gueltig: vier Rahmen, je gepackt UND wieder gelesen, dazu die "
  "Kopflaenge aus den ersten zwei Byte")
z(R, "TheFrame", "test_the_body_is_raw_bytes_and_not_text", "tafel", RAHMEN,
  "rahmen.json: leerer_koerper und koerper_mit_nullbyte; der Koerper geht "
  "ausdruecklich nicht durch JSON")
z(R, "TheFrame", "test_the_head_is_deterministic", "zeuge",
  "tests/kopf_und_schluessel.rs::derselbe_kopf_gibt_dieselben_bytes_egal_wie_er_geschrieben_wurde",
  "LUECKE AUS DIESER ZAEHLUNG: auf der Python-Seite haengt es an sort_keys, hier "
  "sortiert schon das Einlesen. Gemessen wird die Zusicherung deshalb am Uebergang "
  "von TEXT zu Rahmen, mit einer Gegenprobe gegen einen anderen Kopf")
z(R, "TheFrame", "test_what_is_not_a_frame_is_one_error", "tafel",
  "tests/vektoren.rs::was_kein_rahmen_ist_ergibt_einen_fehler",
  "rahmen.json, ungueltig: vier Formen von kein Rahmen, alle mit demselben Fehler")
z(R, "TheFrame", "test_a_head_longer_than_the_length_field_is_refused_when_packing",
  "zeuge", "tests/kopf_und_schluessel.rs::ein_kopf_laenger_als_das_laengenfeld_wird_nicht_gepackt",
  "LUECKE AUS DIESER ZAEHLUNG: die Tafel kennt nur Fehler beim LESEN. Ein Packer, "
  "der einen zu langen Kopf ankuendigt, schreibt eine gelogene Laenge, und der "
  "Leser nimmt den Rest des Kopfs fuer den Koerper")
z(R, "TheRequestHead", "test_a_request_head_becomes_the_parts_a_server_reads", "tafel",
  f"{KOPF}::ein_antrags_kopf_wird_gelesen_wie_von_der_python_seite",
  "kopf-und-schluessel.json, request_of: fuenf Faelle mit Methode, Pfad, Frage und "
  "kleingeschriebenen Kopfzeilen")
z(R, "TheRequestHead", "test_a_target_without_a_query_has_an_empty_one", "tafel",
  f"{KOPF}::ein_antrags_kopf_wird_gelesen_wie_von_der_python_seite",
  "derselbe Satz Faelle traegt ein Ziel ohne Fragezeichen")
z(R, "TheRequestHead", "test_what_is_not_a_request_is_refused", "tafel",
  f"{KOPF}::was_kein_antrags_kopf_ist_wird_abgewiesen",
  "kopf-und-schluessel.json, request_of_fehler: sechs Formen")
z(R, "TheReplyHead", "test_a_reply_head_becomes_status_and_headers", "tafel",
  f"{KOPF}::ein_antwort_kopf_wird_gelesen_und_ein_falscher_abgewiesen",
  "kopf-und-schluessel.json, reply_of: vier Faelle")
z(R, "TheReplyHead", "test_what_is_not_a_status_is_refused", "tafel",
  f"{KOPF}::ein_antwort_kopf_wird_gelesen_und_ein_falscher_abgewiesen",
  "reply_of_fehler mit fuenf Formen, und der Wahrheitswert hat wegen der Sprachen "
  "einen eigenen Zeugen daneben")

# ------------------------------------------------------------ test_handshake
H = "test_handshake"
z(H, "VectorTests", "test_both_published_vectors_are_reproduced_byte_for_byte", "tafel",
  "tests/vektoren.rs::beide_veroeffentlichten_vektoren_werden_byte_fuer_byte_nachgefahren",
  "noise-nk.json: die zwei veroeffentlichten Vektoren, beide Nachrichten, der "
  "Handschlag-Hash und die Sitzungsnachrichten nach dem split")
z(H, "BehaviourTests", "test_a_changed_byte_is_refused", "zeuge",
  "tests/vektoren.rs::ein_veraendertes_siegel_oeffnet_nicht",
  "ein gekipptes Byte in der ersten Nachricht muss auffallen, sonst ist die "
  "Pruefsumme nicht geprueft worden")
z(H, "BehaviourTests", "test_a_different_prologue_gets_no_session", "zeuge",
  "tests/sitzungsladen.rs::ein_abweichender_prologue_wird_nie_eine_sitzung",
  "eine Ebene hoeher gemessen als drueben, am Laden statt am Handschlag: der Laden "
  "legt keine Sitzung an, und das ist die Wirkung, auf die es ankommt")
z(H, "BehaviourTests", "test_a_fresh_pair_agrees_on_both_directions", "zeuge",
  "tests/handschlag.rs::ein_frisches_paar_versteht_sich_in_beide_richtungen",
  "LUECKE AUS DIESER ZAEHLUNG: die Vektoren bringen feste Schluessel mit, ein "
  "frisch erzeugtes Paar war nirgends gefahren")
z(H, "BehaviourTests", "test_the_counter_moves_so_a_nonce_is_never_reused", "zeuge",
  "tests/handschlag.rs::der_zaehler_rueckt_vor_damit_kein_nonce_zweimal_gilt",
  "LUECKE AUS DIESER ZAEHLUNG, und die schwerste der drei: ein wiederholter Nonce "
  "gibt bei ChaCha20-Poly1305 den Schluesselstrom preis, und ein Rundlauf mit EINER "
  "Nachricht sieht das nie")
z(H, "BehaviourTests", "test_the_wrong_static_key_gets_no_session", "zeuge",
  "tests/vektoren.rs::ein_falscher_dienst_schluessel_haelt_den_handschlag_an",
  "der ganze Gewinn von NK: wer den privaten Teil nicht hat, kann die zweite "
  "Nachricht nicht erzeugen")
z(H, "BehaviourTests", "test_two_handshakes_never_look_alike", "zeuge",
  "tests/handschlag.rs::zwei_handschlaege_sehen_nie_gleich_aus",
  "LUECKE AUS DIESER ZAEHLUNG: ohne frisches Kurzzeit-Paar faellt die Konstruktion "
  "zusammen, und kein Rundlauf wuerde es bemerken, weil beide Seiten weiter sauber "
  "zueinander kommen")

# ------------------------------------------------------------- test_identity
I = "test_identity"
z(I, "OnDisk", "test_the_first_start_makes_a_key_and_remembers_it", "zeuge",
  "tests/kennung.rs::der_erste_start_legt_einen_schluessel_an_und_merkt_ihn",
  "LUECKE AUS DIESER ZAEHLUNG: src/kennung.rs hatte keinen einzigen Rust-Zeugen")
z(I, "OnDisk", "test_the_key_file_is_owner_only", "zeuge",
  "tests/kennung.rs::die_schluesseldatei_gehoert_nur_ihrem_eigentuemer",
  "die Rechte stehen am frischen Deskriptor und nicht in einem chmod hinterher")
z(I, "OnDisk", "test_a_second_start_does_not_replace_the_key", "zeuge",
  "tests/kennung.rs::ein_zweiter_start_ersetzt_den_schluessel_nicht", "")
z(I, "OnDisk", "test_a_changed_key_file_refuses_and_leaves_the_memory_alone", "zeuge",
  "tests/kennung.rs::eine_geaenderte_schluesseldatei_weist_ab_und_laesst_das_gedaechtnis_in_ruhe",
  "ueberlebt das Gedaechtnis die Weigerung nicht, saehe der zweite Versuch aus wie "
  "der freundliche Fall remember und liesse den Fremden herein")
z(I, "OnDisk", "test_a_missing_key_file_refuses_instead_of_growing_a_new_one", "zeuge",
  "tests/kennung.rs::eine_fehlende_schluesseldatei_weist_ab_statt_eine_neue_wachsen_zu_lassen",
  "und eine Weigerung darf keinen Schluessel hinlegen")
z(I, "OnDisk", "test_a_damaged_key_file_is_not_read_as_absence", "zeuge",
  "tests/kennung.rs::eine_beschaedigte_datei_gilt_nicht_als_abwesenheit",
  "abwesend heisst dieser Dienst ist neu; beschaedigt heisst, jemand hat einen "
  "Schluessel und es ist nicht dieser")
z(I, "OnDisk", "test_a_key_of_the_wrong_length_is_refused_too", "zeuge",
  "tests/kennung.rs::ein_schluessel_falscher_laenge_wird_ebenfalls_abgewiesen",
  "lesbares base64 mit fuenf Byte darin: die Form stimmt, die Laenge nicht")
z(I, "OnDisk", "test_a_store_restored_beside_its_own_key_adopts_it_without_being_asked",
  "zeuge", "tests/kennung.rs::ein_speicher_neben_seinem_eigenen_schluessel_nimmt_ihn_ohne_frage_an",
  "die gewoehnliche Rueckkehr; hier zu weigern hiesse den Fall zu brechen, fuer den "
  "die Konstruktion gebaut ist")
z(I, "TheDecision", "test_nothing_here_and_nothing_remembered_makes_the_first_key", "tafel",
  f"{KOPF}::der_stand_eines_dienst_schluessels_ist_der_gemessene",
  "kopf-und-schluessel.json, decide: fuenf Faelle mit Name, may_serve und SATZ")
z(I, "TheDecision", "test_a_key_here_that_the_store_never_saw_is_recorded", "tafel",
  f"{KOPF}::der_stand_eines_dienst_schluessels_ist_der_gemessene",
  "derselbe Satz Faelle, der bewusst nachgiebige Fall remember")
z(I, "TheDecision", "test_the_expected_key_carries_on", "tafel",
  f"{KOPF}::der_stand_eines_dienst_schluessels_ist_der_gemessene", "der Fall carry_on")
z(I, "TheDecision", "test_a_different_key_refuses_and_names_the_way_out", "zeuge",
  f"{KOPF}::die_beiden_abweisenden_faelle_sagen_was_zu_tun_ist",
  "der Zeuge prueft, dass der Grund BEIDE Abdruecke und den Befehl nennt: eine "
  "Abweisung darf kein Terminal erreichen, ohne zu sagen, was zu tun ist")
z(I, "TheDecision", "test_a_missing_key_refuses_and_names_the_way_out", "zeuge",
  f"{KOPF}::die_beiden_abweisenden_faelle_sagen_was_zu_tun_ist",
  "die zweite Haelfte desselben Zeugen, mit dem Befehl fuer eine neue Kennung")
z(I, "TheKeyIsNotInTheDatabase", "test_the_private_key_does_not_appear_in_the_store_or_its_backup",
  "zeuge", "tests/kennung.rs::der_private_schluessel_steht_nicht_im_speicher_und_nicht_in_seiner_kopie",
  "der Entscheid dieser Scheibe, gemessen statt behauptet: jeder andere Fall "
  "bestuende auch, wenn der Schluessel als Zeile in der Datenbank laege. Der "
  "ABDRUCK muss drin sein, und das ist der Gutfall der Suche")
z(I, "TheKeyIsNotInTheDatabase", "test_the_key_file_is_not_swept_up_by_a_backup_glob", "zeuge",
  "tests/kennung.rs::die_schluesseldatei_wird_von_einem_sicherungs_muster_nicht_mitgenommen",
  "store.sqlite* ist, wie die Sicherungszeile eines Betreibers aussieht; gemessen "
  "wird die Vorgabe, also wird eine gesetzte Umgebung unter einer Sperre "
  "beiseitegelegt und zurueckgestellt")
z(I, "TheOperatorSaysItIsWanted", "test_adopt_takes_the_key_that_is_there_and_says_what_was_there_before",
  "zeuge", "tests/kennung.rs::adopt_nimmt_den_schluessel_der_daliegt_und_sagt_was_vorher_war", "")
z(I, "TheOperatorSaysItIsWanted", "test_adopt_refuses_to_quietly_make_a_key", "zeuge",
  "tests/kennung.rs::adopt_macht_keinen_schluessel_still",
  "ein adopt, das erzeugt, wenn es nichts zu uebernehmen gibt, waere genau die "
  "stille Ersetzung, gegen die das Modul geschrieben ist")
z(I, "TheOperatorSaysItIsWanted", "test_new_makes_one_and_keeps_the_old_file_beside_it", "zeuge",
  "tests/kennung.rs::new_macht_einen_und_behaelt_die_alte_datei_daneben",
  "eine versehentliche Drehung darf nicht unwiederbringlich sein")
z(I, "TheOperatorSaysItIsWanted", "test_every_rotation_keeps_its_key_not_just_the_last_one",
  "zeuge", "tests/kennung.rs::jede_drehung_behaelt_ihren_schluessel_nicht_nur_die_letzte",
  "drei Drehungen, nicht eine: eine Drehung findet die alte Datei daneben und ist "
  "gruen ueber einer Konstruktion mit einem einzigen festen Namen")
z(I, "TheOperatorSaysItIsWanted", "test_showing_the_standing_changes_nothing", "zeuge",
  "tests/kennung.rs::den_stand_zeigen_aendert_nichts",
  "eine Auskunft darf den Zustand nicht aendern, sonst kann niemand nachsehen, "
  "ohne einzugreifen")
z(I, "TheWrittenForm", "test_it_is_six_groups_of_four", "zeuge",
  "tests/kennung.rs::der_abdruck_ist_sechs_gruppen_zu_vier",
  "LUECKE AUS DIESER ZAEHLUNG: die Abdruck-Tafel haelt vier feste Werte fest, die "
  "GESTALT stand nirgends")
z(I, "TheWrittenForm", "test_the_confusable_letters_are_not_in_the_alphabet", "zeuge",
  "tests/kennung.rs::die_verwechselbaren_buchstaben_stehen_nicht_im_alphabet",
  "das Alphabet ist in Rust nicht oeffentlich, also am ERGEBNIS gemessen: "
  "vierundsechzig Abdruecke, kein I, L, O oder U, und mehr als zwanzig "
  "verschiedene Zeichen, damit der Fall nicht nur die Schleife misst")
z(I, "TheWrittenForm", "test_the_same_key_always_prints_the_same_and_two_keys_do_not",
  "zeuge", "tests/vektoren.rs::zwei_verschiedene_schluessel_geben_zwei_abdruecke",
  "die Wiederholbarkeit steckt in der Tafel, die Verschiedenheit in diesem Zeugen")
z(I, "TheWrittenForm", "test_four_fixed_keys_print_what_the_application_prints_for_them",
  "tafel", "tests/vektoren.rs::abdruecke_stimmen_byte_gleich",
  "abdruecke.json: dieselben vier festen Schluessel, und der Erzeuger hat sie ein "
  "zweites Mal aus SHA-256 und der Tabelle gerechnet, ohne identity.py zu rufen")
z(I, "TheWrittenForm", "test_a_truncated_key_is_refused_rather_than_fingerprinted", "zeuge",
  "tests/vektoren.rs::ein_schluessel_falscher_laenge_wird_abgewiesen",
  "31, 33 und 0 Byte: ein abgeschnittener Schluessel darf nicht einfach einen "
  "anderen Abdruck ergeben, er darf ueberhaupt keinen ergeben")

# -------------------------------------------------------------- test_restore
K = "test_restore"
z(K, "ARestoreThatFits", "test_the_same_documents_at_the_same_sizes_pass", "tafel", RUECK,
  "rueckkehr.json: der Fall passt, mit Ausgang, SATZ und passed")
z(K, "ARestoreThatDiffers", "test_a_document_that_did_not_come_back", "tafel", RUECK,
  "rueckkehr.json: eins fehlt und zwei fehlen")
z(K, "ARestoreThatDiffers", "test_a_document_that_came_back_uninvited", "tafel", RUECK,
  "rueckkehr.json: eins zuviel; ein Wegwerf-Speicher, der nicht weggeworfen wurde, "
  "misst die Reste der letzten Probe")
z(K, "ARestoreThatDiffers", "test_a_document_that_came_back_the_wrong_size", "zeuge",
  "tests/rueckkehr.rs::ein_unterschied_wird_benannt_und_nicht_nur_gezaehlt",
  "der Satz nennt das Dokument und BEIDE Zahlen; wer nur 1 von 2 liest, weiss "
  "nicht, wo er suchen soll")
z(K, "ARestoreThatWasNotMeasured", "test_a_rehearsal_on_an_empty_backup_proves_nothing",
  "zeuge", "tests/rueckkehr.rs::eine_probe_die_nicht_laufen_konnte_besteht_nicht",
  "nichts mit nichts verglichen ist die Messung, die wie ein Erfolg aussieht")
z(K, "ARestoreThatWasNotMeasured", "test_a_rehearsal_that_stopped_is_not_a_pass", "zeuge",
  "tests/rueckkehr.rs::eine_probe_die_nicht_laufen_konnte_besteht_nicht",
  "derselbe Zeuge, drei Wege, auf denen eine Probe nicht stattfindet, und ein "
  "Gutfall daneben, damit er nicht einfach alles ablehnt")
z(K, "ARestoreThatWasNotMeasured", "test_an_error_wins_over_a_comparison_that_would_have_passed",
  "zeuge", "tests/rueckkehr.rs::eine_probe_die_nicht_laufen_konnte_besteht_nicht",
  "gestoppt MIT passenden Zahlen: laege der Fehler-Zweig hinten, meldete eine "
  "gescheiterte Probe passt")
z(K, "ARestoreThatWasNotMeasured", "test_nothing_to_compare_is_not_a_pass", "tafel", RUECK,
  "rueckkehr.json: nichts uebergeben und read_back fehlt")
z(K, "OnlyFitsPasses", "test_every_outcome_maps_the_way_its_name_says", "tafel", RUECK,
  "die Tafel prueft je Fall auch passed, und die zehn Faelle decken alle drei "
  "Ausgaenge; dazu haelt ein eigener Zeuge die drei Namen gegen die Python-Seite")

# ------------------------------------------------------------- test_sessions
S = "test_sessions"
z(S, "WhenASessionEnds", "test_a_fresh_session_is_live", "tafel", STAND,
  "sitzungen.json, standing: sechzehn Faelle, darunter vier, in denen BEIDES zutrifft")
z(S, "WhenASessionEnds", "test_a_session_used_a_minute_ago_is_live", "tafel", STAND,
  "der gewoehnliche Fall, und er darf nie enden: die langsamste regelmaessige "
  "Frequenz des Klienten ist eine Minute")
z(S, "WhenASessionEnds", "test_silence_past_the_idle_limit_ends_it", "tafel", STAND,
  "sitzungen.json, standing: eine Sekunde ueber der Frist")
z(S, "WhenASessionEnds", "test_one_second_short_of_the_idle_limit_is_still_live", "tafel", STAND,
  "der Rand daneben, ohne den ein Vergleich um einen Schritt verschoben werden "
  "koennte, ohne dass etwas reisst")
z(S, "WhenASessionEnds", "test_a_busy_session_still_ends_at_its_age", "tafel", STAND,
  "vor einer Sekunde benutzt, also greift nichts an der Stille; ohne die "
  "Altersgrenze hielte ein gespraechiger Klient einen Satz Schluessel ewig am Leben")
z(S, "WhenASessionEnds", "test_age_is_answered_before_idleness", "tafel", STAND,
  "die vier Faelle, in denen beides zutrifft: der Grund reist in eine Protokollzeile, "
  "und die Bedingung, die zuerst geendet haette, ist die ehrliche Antwort")
z(S, "WhenASessionEnds", "test_the_two_endings_are_told_apart", "zeuge", ZAHLEN,
  "IDLE und OLD werden einzeln gegen die Tafel gehalten; ein einziges expired "
  "verbaerge, welche der zwei Zahlen an dem Tag falsch ist, an dem eine es ist")
z(S, "AHandshakeThatProvesNothingGetsAMinute", "test_an_unused_handshake_dies_after_a_minute",
  "tafel", STAND, "sitzungen.json, standing: unbewiesen an der kurzen Frist")
z(S, "AHandshakeThatProvesNothingGetsAMinute", "test_a_proven_session_of_the_same_age_is_still_live",
  "tafel", STAND,
  "dieselben Zahlen, ein Feld anders; ohne diesen Fall wuerde die kurze Frist fuer "
  "alle gelten und jede echte Sitzung jede Minute enden")
z(S, "AHandshakeThatProvesNothingGetsAMinute", "test_a_handshake_used_a_moment_ago_is_live",
  "tafel", STAND, "sitzungen.json, standing: unbewiesen, gerade eben benutzt")
z(S, "TheAnonymousCallerIsBounded", "test_below_capacity_nobody_gives_way", "tafel", WEICHEN,
  "sitzungen.json, give_way: leer und unter der Decke")
z(S, "TheAnonymousCallerIsBounded", "test_at_capacity_exactly_one_gives_way", "tafel", WEICHEN,
  "sitzungen.json, give_way: genau voll, alle bewiesen; genau einer weicht, weil "
  "einer dazukommt")
z(S, "TheAnonymousCallerIsBounded", "test_over_capacity_enough_give_way_to_leave_room_for_one",
  "tafel", WEICHEN, "sitzungen.json, give_way: darueber")
z(S, "TheAnonymousCallerIsBounded", "test_the_busy_one_survives_even_when_it_is_the_oldest",
  "tafel", WEICHEN,
  "zuletzt BENUTZT und nicht aeltest nach Entstehung; die Tafel fuehrt je Fall die "
  "ganze Liste mit ihren Zeitpunkten, die Ordnung ist damit festgehalten")
z(S, "TheAnonymousCallerIsBounded", "test_a_handshake_gives_way_before_a_proven_session_does",
  "tafel", WEICHEN,
  "sitzungen.json, give_way: genau voll, unbewiesene zuerst, und alle unbewiesen "
  "als Gegenprobe")
z(S, "TheAnonymousCallerIsBounded", "test_the_shipped_capacity_is_a_ceiling_and_not_a_plan",
  "zeuge", ZAHLEN,
  "CAPACITY wird gegen die Tafel gehalten; die Zahl zu heben ist damit eine "
  "Entscheidung und keine stille Aenderung")
z(S, "AFloodCannotPushOutARealDevice", "test_the_store_stays_at_its_ceiling", "zeuge",
  "tests/sitzungsladen.rs::eine_flut_laesst_den_laden_nicht_wachsen",
  "vierzig Runden gegen eine Decke von vier, und er bleibt voll statt leer")
z(S, "AFloodCannotPushOutARealDevice", "test_the_real_session_is_still_there_afterwards",
  "zeuge", "tests/sitzungsladen.rs::eine_flut_drueckt_keine_bewiesene_sitzung_hinaus",
  "der Fall, fuer den give_way gebaut ist")
z(S, "AFloodCannotPushOutARealDevice", "test_it_survives_a_flood_far_larger_than_the_store",
  "zeuge", "tests/sitzungsladen.rs::eine_flut_von_hundertfacher_groesse_aendert_daran_nichts",
  "LUECKE AUS DIESER ZAEHLUNG: der Zeuge daneben faehrt zwanzig Runden, und eine "
  "Anordnung weniger Runden kann zufaellig guenstig sein. Dieser faehrt "
  "fuenfhundert gegen eine Decke von fuenf")
z(S, "AFloodCannotPushOutARealDevice", "test_the_strangers_drain_by_themselves_once_they_stop",
  "tafel", FEGEN,
  "sitzungen.json, sweep: unbewiesen verfallen und gemischt; dazu haelt "
  "tests/sitzungsladen.rs::eine_beendete_sitzung_wird_auf_der_stelle_fallen_gelassen "
  "fest, dass der Laden dafuer niemanden braucht, der fegt")
z(S, "TheOrderIsTheDecision", "test_the_dead_ones_go_and_the_device_stays", "tafel", PLATZ,
  "sitzungen.json, room_for_one: tote duerfen keinen lebenden kosten")
z(S, "TheOrderIsTheDecision", "test_the_other_order_would_evict_the_device", "zeuge",
  "tests/sitzungen.rs::die_ordnung_von_fegen_und_weichen_ist_die_entscheidung",
  "derselbe Bestand zweimal gerechnet, einmal durch die eine Tuer und einmal in "
  "der falschen Reihenfolge; waeren die Antworten gleich, waere die Ordnung egal")
z(S, "TheOrderIsTheDecision", "test_a_store_of_live_sessions_with_room_loses_nobody", "tafel",
  PLATZ, "sitzungen.json, room_for_one: platz da")
z(S, "TheOrderIsTheDecision", "test_a_full_store_of_live_sessions_still_gives_way", "tafel",
  PLATZ,
  "sitzungen.json, room_for_one: voll und alles lebt; das Fegen findet nichts, also "
  "muss es durchfallen statt nichts zu tun")
z(S, "SweepingIsNotTheSameQuestion", "test_dead_sessions_go_even_when_there_is_plenty_of_room",
  "tafel", FEGEN,
  "sitzungen.json, sweep: gemischt und zu alt; ein Speicher, der nur unter Druck "
  "aufraeumt, haelt auf einem stillen Dienst tausend tote Sitzungen")
z(S, "SweepingIsNotTheSameQuestion", "test_a_store_of_live_sessions_sweeps_to_nothing",
  "tafel", FEGEN, "sitzungen.json, sweep: nichts tot")
z(S, "SweepingIsNotTheSameQuestion", "test_sweeping_an_empty_store_is_not_an_error", "tafel",
  FEGEN, "sitzungen.json, sweep faehrt auch die leere Liste")

# ------------------------------------------------------------- test_versions
V = "test_versions"
z(V, "RotationKeepsTheNewest", "test_it_drops_the_oldest_first", "tafel", ROT,
  "schreiben.json, rotate: acht auf drei, und der Zeuge prueft zusaetzlich, dass "
  "dropped und kept zusammen wieder die Eingabe ergeben")
z(V, "RotationKeepsTheNewest", "test_a_store_under_the_limit_loses_nothing", "tafel", ROT,
  "schreiben.json, rotate: drei mit keep 9")
z(V, "RotationKeepsTheNewest", "test_exactly_at_the_limit_loses_nothing", "tafel", ROT,
  "schreiben.json, rotate: drei mit keep 3")
z(V, "RotationKeepsTheNewest", "test_an_empty_store_stays_empty", "tafel", ROT,
  "schreiben.json, rotate: die leere Liste")
z(V, "RotationKeepsTheNewest", "test_keeping_one_keeps_the_newest_and_only_the_newest",
  "tafel", ROT, "schreiben.json, rotate: drei mit keep 1")
z(V, "RotationNeverEmptiesTheStore", "test_keeping_none_raises", "tafel", ROT,
  "schreiben.json, rotate_fehler: keep 0 und keep -1")
z(V, "RotationNeverEmptiesTheStore", "test_it_raises_even_when_there_is_nothing_to_lose",
  "tafel", ROT,
  "der Zeuge faehrt die Fehlerfaelle gegen die LEERE Liste: ein leerer Speicher "
  "machte das schlechte Argument heute harmlos und liesse es bis zu dem Tag durch, "
  "an dem es das nicht ist")
z(V, "RotationDoesNotDisturbTheCaller", "test_the_given_sequence_is_left_alone", "zeuge",
  "tests/schreiben.rs::rotate_laesst_die_liste_des_rufers_in_ruhe",
  "LUECKE AUS DIESER ZAEHLUNG: drueben ist das eine Zusicherung, die brechen KANN, "
  "hier verbietet der Typ es. Der Zeuge haelt das VERSPRECHEN fest, nicht die "
  "Gefahr: wer die Signatur eines Tages auf einen besitzenden Wert zieht, faellt hier")
z(V, "RotationDoesNotDisturbTheCaller", "test_it_takes_any_sequence_not_only_a_list",
  "keiner", "",
  "SPRACHE. Der Python-Fall reicht ein Tupel statt einer Liste und prueft, dass "
  "rotate nicht auf einen bestimmten Behaelter besteht. In Rust nimmt rotate einen "
  "Ausschnitt, und was ein Ausschnitt ist, entscheidet der Uebersetzer; es gibt "
  "keinen zweiten Behaelter, gegen den man es fahren koennte")

# --------------------------------------------------------------- test_writes
W = "test_writes"
z(W, "AWriteThatGoesThrough", "test_the_current_version_at_the_current_format", "tafel",
  SCHRIEB, "schreiben.json, decide: das gelesene Etikett, Status 200 mit Satz")
z(W, "AWriteThatGoesThrough", "test_the_first_write_of_a_document_names_no_version", "tafel",
  SCHRIEB, "schreiben.json, decide: nichts gespeichert, kein if_match, 200")
z(W, "TheStaleWriterIsToldToLookAgain", "test_a_wrong_version_is_stale", "tafel", SCHRIEB,
  "schreiben.json, decide: Fassung 2 gegen gespeicherte 3, Status 412, und der Satz "
  "nennt die genannte Fassung")
z(W, "TheStaleWriterIsToldToLookAgain", "test_naming_no_version_at_all_is_stale_too", "tafel",
  SCHRIEB,
  "schreiben.json, decide: das Dokument liegt und der Schrieb nennt keine Fassung. "
  "Schweigen ist kein Anspruch, aktuell zu sein; das ist das blinde Ueberschreiben, "
  "das iCloud nicht abweisen kann")
z(W, "TheStaleWriterIsToldToLookAgain", "test_a_version_for_a_document_that_is_not_there_is_stale",
  "tafel", SCHRIEB, "schreiben.json, decide: kein solches Dokument, aber eine genannte Fassung")
z(W, "TheOrderOfTheGates", "test_stale_beats_a_bad_format", "tafel", SCHRIEB,
  "schreiben.json, decide: gespeicherte Fassung 2, Schreiber 1, falsches Etikett, 412")
z(W, "TheOrderOfTheGates", "test_stale_beats_a_collapse", "zeuge",
  "tests/schreiben.rs::die_ordnung_der_tore_ist_die_gemessene",
  "der wichtige: ein Klient auf einer alten Fassung hat oft einen KLEINEREN Stand, "
  "und zu klein zu sagen schickte ihn das Falsche reparieren. Der Zeuge faehrt "
  "alles drei zugleich falsch und prueft, dass die Vorbedingung zuerst spricht")
z(W, "TheOrderOfTheGates", "test_a_bad_format_beats_a_collapse", "zeuge",
  "tests/schreiben.rs::die_ordnung_der_tore_ist_die_gemessene",
  "die zweite Haelfte desselben Zeugen: nur Format und Groesse falsch, dann spricht "
  "das Format")
z(W, "TheOrderOfTheGates", "test_a_collapse_is_only_reported_once_nothing_else_is_wrong",
  "tafel", SCHRIEB, "schreiben.json, decide: nur der Einbruch, Status 409 mit shrank im Satz")
z(W, "TheGatesStillWorkThroughThisDoor", "test_an_empty_state_over_a_full_one_is_refused",
  "tafel", SCHRIEB, "schreiben.json, decide: 100 auf 0 mit richtigem Etikett, 409 mit empty")
z(W, "TheGatesStillWorkThroughThisDoor", "test_an_older_writer_is_refused", "tafel", SCHRIEB,
  "schreiben.json, decide: Schreiber 1 auf gespeicherte 2, 409 mit dem Fassungssatz")
z(W, "TheGatesStillWorkThroughThisDoor", "test_a_declared_shrink_passes_all_three", "tafel",
  SCHRIEB, "schreiben.json, decide: 100 auf 0 mit declared_shrink, 200")
z(W, "TheGatesStillWorkThroughThisDoor", "test_the_floor_reaches_through", "zeuge",
  "tests/schreiben.rs::der_boden_reicht_durch_die_tuer_des_schriebs",
  "LUECKE AUS DIESER ZAEHLUNG: dass admit einen Boden kennt, heisst nicht, dass "
  "decide ihn weiterreicht. Genau so verliert eine Zusammensetzung still ein Tor. "
  "Der Gutfall daneben faehrt denselben Schrieb mit der Vorgabe und wird abgewiesen")
z(W, "NoDecisionIsSilent", "test_every_answer_carries_a_sentence", "tafel", SCHRIEB,
  "die Tafel vergleicht je Fall den ganzen SATZ, nicht seine Laenge; ein stummes "
  "Urteil faellt damit auf, und ein falsch begruendetes auch")

print(json.dumps({
    "was": "Die 126 Faelle aus der Gruppe 'reines Modul mit Gegenpart und eigenen "
           "Vektoren', auf FALL-Ebene.",
    "wie": "Je Fall ein Urteil mit Grund. Ein Urteil ist keine Messung; messbar macht es "
           "der Zaun in tests/test_abdeckung.py, der jeden genannten Zeugen gegen die "
           "Wirklichkeit haelt.",
    "arten": {"tafel": "steht in einer Vektortafel, die ein Rust-Zeuge abfaehrt",
              "zeuge": "ein eigener Rust-Zeuge",
              "draht": "gegen das laufende Erzeugnis gefahren",
              "keiner": "kein Gegenpart, Grund dabei"},
    "faelle": [{"fall": k, **v} for k, v in sorted(Z.items())]},
    indent=2, ensure_ascii=False))
