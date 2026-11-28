# Konten und Anmeldung

Konten anlegen, und wie sich ein Gerät seit Vertragsversion 6 anmeldet. Zurück zur [README](../README.de.md).

## Konten

Jeder Pfad außer `/v1/health`, `/v2/key`, `/v1/session` und den drei Anmelde-Pfaden
braucht den Kopf `Authorization: Bearer`, gefolgt vom Token, und das Konto, auf das die Anfrage wirkt, kommt
aus der Antwort des Speichers auf dieses Token, nie aus irgendetwas, das der Aufrufer geschrieben hat.

    maer-sync account <owner>            # password on stdin, never in argv
    maer-sync account-remove <owner>     # the account and its documents leave,
                                         # after a backup that rehearsed its restore
    maer-sync reset <owner>              # a one-time password that only changes the password
    maer-sync devices <owner>            # how each device of an account signed in last

### Anmelden (Version 6)

Die genaue Leitung steht in `CONTRACT.md`; hier steht, was jemand, der den Dienst betreibt, davon
wissen muss.

- **Drei Pfade, nur innerhalb einer Sitzung.** `/v1/salt` nennt einem Gerät das Salz
  und die Form eines Kontos, `/v1/token` tauscht einen Nachweis gegen ein Token,
  `/v1/password` ändert das Passwort. Eine reine Anfrage bekommt `426`.
- **Der Dienst hält einen Nachweis, noch einmal mit scrypt gestreckt**, nie ein
  Passwort. Wer den Speicher stiehlt, kann weiter Passwörter raten, zum Preis
  beider Streckungen je Versuch; der laufende Dienst erfährt nichts, womit er
  ein versiegeltes Dokument öffnen könnte. Tokens liegen als Digests im Speicher, also
  gibt ein entwendeter Speicher keine gültige Zugangsberechtigung heraus.
- **Ein falscher Nachweis und ein Besitzer, den es hier nicht gibt, bekommen dieselbe Antwort**, und
  `/v1/salt` antwortet jedem Namen mit einem Salz derselben Form, also
  lassen sich die Pfade nicht nutzen, um herauszufinden, wer ein Konto hat.
- **Eine Passwortänderung zieht jedes andere Token des Besitzers zurück**, und
  ein Gerät, das eines hält, erfährt, warum (`X-Maer-Refusal: password-changed`).
- **Zehn Fehlversuche je Besitzername und Adresse in fünfzehn Minuten**, und
  der nächste ist `429` mit `Retry-After`. Zur Adresse siehe Schritt 7.
- **`reset <owner>` macht ein Einmalpasswort und zeigt es einmal.** Es meldet
  niemanden an (`428`); es lässt ein Gerät nur ein neues Passwort setzen, und jedes
  Token des Kontos wird zurückgezogen. Wer den Dienst betreibt, kennt es, und genau
  deshalb kann es nie der Schlüssel der Feed-Zugangsdaten werden.

Das Token reist in einem Header, nie in der Adresse: ein Query-String wird
in jedes Zugriffs-Log und jeden Proxy-Eintrag zwischen dem Gerät und dem Dienst kopiert.
Der WebSocket-Handshake nimmt auch Header: ein nativer Client setzt sie, und
nur Browser können es nicht, weshalb so viele Dienste am Ende
Tokens in URLs stecken.

---

Weitere Seiten: [Von Hand einrichten](setup.de.md) · [Im Container betreiben](container.de.md) · [Der Katalog-Index (optional)](catalog.de.md) · [Einen laufenden Wirt betreiben](operation.de.md) · [Konten und Anmeldung](accounts.de.md) · [Wie es gebaut ist](design.de.md) · [Tests und Messungen](testing.de.md)
