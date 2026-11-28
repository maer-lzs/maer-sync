# Tests und Messungen

Wie du die Suiten laufen lässt, und was gemessen ist und was nicht. Zurück zur [README](../README.de.md).

## Die Tests laufen lassen

    werkzeug/vor-dem-commit.sh          # everything, and stops on red
    werkzeug/bau.sh test --offline      # the Rust suite alone

Die Rust-Suite läuft im festgenagelten Build-Image und gibt ihre Zahlen je Datei aus.
Die Zäune des Repositorys laufen auf dem Wirt, weil sie git fragen. Ein Fall, der
seine Frage an dieser Kopie nicht stellen kann, wird mit seinem Grund übersprungen, statt zu bestehen.

## Was gemessen ist, und was nicht

- Der Dienst läuft als statische Binärdatei hinter einem TLS-Proxy auf einem Wirt
  dessen, der ihn betreibt; die Anmeldung der App, der Hin- und Rückweg eines Dokuments und der
  Changes-Socket werden dort geübt.
- Die Übernahme von Ende zu Ende (ein gesperrtes, spielendes Gerät stoppt innerhalb
  des Rhythmus des Anspruchs, wenn das andere startet) braucht zwei Geräte und gehört in
  den Gerätetest.
- Der Ersatz-Rhythmus, wenn der Changes-Socket unten ist (die App fragt alle
  dreißig Sekunden, bis er wieder offen ist), ist gewählt, nicht gemessen; die Messung
  gehört in denselben Gerätetest.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
