# Mär Sync Service — Abbild.
#
# Gebaut und gefahren am 16.09.2026 auf dem Tiny, nicht geschrieben und gehofft.
#
# Das Grundabbild ist auf den Punkt gepinnt, nicht auf "3.13": ein wanderndes
# Grundabbild macht aus --require-hashes in requirements.txt eine halbe Zusage.
# Der Sinn des Pinnens ist, dass sich die Laufzeit unter einem TLS-terminierten
# Dienst nicht ändert, und das Grundabbild IST Laufzeit.
FROM python:3.13.7-slim-bookworm

# Kein root. Die feste Nummer ist Absicht: das benannte Volume gehört einer UID,
# und die muss über Neubauten des Abbilds hinweg dieselbe bleiben, sonst steht
# der Dienst nach einem Update vor seinem eigenen Speicher und darf nicht hinein.
RUN groupadd --system --gid 10001 maersync \
 && useradd --system --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin maersync

WORKDIR /opt/maer-sync

# Erst die Abhängigkeiten, dann der Code: so muss die Installationsschicht nicht
# bei jeder Code-Änderung neu gebaut werden.
COPY requirements.txt ./
RUN python -m venv .venv \
 && .venv/bin/python -m pip install --no-cache-dir --require-hashes -r requirements.txt

COPY core ./core
COPY service ./service
COPY run-tests.sh CONTRACT.md README.md ./
COPY tests ./tests

# Der Speicherort ist ein VOLUME, kein Pfad im Abbild. Läge er im Abbild, wäre die
# Ablage beim nächsten Abbild weg — das ist der Fehler, der Daten kostet, und er
# fällt beim Lesen nicht auf.
RUN mkdir -p /var/lib/maer-sync && chown maersync:maersync /var/lib/maer-sync
VOLUME ["/var/lib/maer-sync"]

ENV MAER_SYNC_DB=/var/lib/maer-sync/store.sqlite \
    MAER_SYNC_HOST=0.0.0.0 \
    MAER_SYNC_PORT=8099

USER maersync
EXPOSE 8099

# Kein Shell-Formular: so bekommt der Dienst das Signal von docker stop direkt und
# beendet sich sauber, statt nach zehn Sekunden getötet zu werden.
ENTRYPOINT ["/opt/maer-sync/.venv/bin/python", "-m", "service"]
