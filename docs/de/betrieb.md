---
title: Server betreiben
parent: Deutsch
nav_order: 6
permalink: /de/betrieb/
lang: de
---

# Server betreiben

Wie Sie notenwart auf einem eigenen Server betreiben: Installation, HTTPS, E-Mail, Updates, Sicherungen. Alles hier
läuft aus einem Checkout dieses Repositorys auf dem Server (gebraucht wird nur `deploy/`; das Image kommt aus der
GitHub Container Registry). Die englische Fassung ist [Running a server]({{ '/operations/' | relative_url }}); bei
Abweichungen gilt sie.

## Voraussetzungen

- Ein Linux-Server oder eine VM mit Docker und dem Compose-Plugin; für den Anfang 2 GB RAM und einige GB Platte
  (die PDFs wachsen mit der Bibliothek).
- Ein Domainname, der auf den Server zeigt (A/AAAA-Eintrag), Ports 80 und 443 offen, wenn Caddy das
  HTTPS-Zertifikat holen soll.
- Optional: ein SMTP-Konto für Anmeldelinks und Einladungen.

## Installation

```sh
git clone https://github.com/notenwart/notenwart
cd notenwart
deploy/generate-secrets.sh > deploy/.env
```

Die Einstellungen des Servers in `deploy/.env` ergänzen:

```sh
NOTENWART_DOMAIN=noten.example.org
NOTENWART_PUBLIC_URL=https://noten.example.org
NOTENWART_LANGUAGE=de
SMTP_URL=smtps://user:password@mail.example.org
MAIL_FROM=notenwart <no-reply@example.org>
```

Mit Caddy davor starten (es holt und erneuert das Zertifikat selbst):

```sh
docker compose -f deploy/compose.yaml --profile https up -d
```

`https://noten.example.org` öffnen und **sofort** das erste Admin-Konto anlegen – das bekommt, wer zuerst kommt.

Ohne `--profile https` lauscht notenwart nur auf `127.0.0.1:8080`; davor einen eigenen Reverse Proxy setzen oder
für einen Test im lokalen Netz `NOTENWART_PORT` setzen (z. B. `0.0.0.0:8080`).

| Variable | Standard | Was |
|---|---|---|
| `NOTENWART_DOMAIN` | – | Domain für Caddy (`--profile https`) |
| `NOTENWART_PUBLIC_URL` | `http://localhost:8080` | Wo notenwart erreichbar ist; für Links in E-Mails, `https` macht das Sitzungs-Cookie Secure |
| `NOTENWART_LANGUAGE` | `en` | Sprache der E-Mails: `de` oder `en` |
| `NOTENWART_VERSION` | `latest` | Image-Tag, z. B. `0.1.0`, um eine Version festzuhalten |
| `NOTENWART_PORT` | `127.0.0.1:8080` | Adresse und Port von notenwart selbst |
| `SMTP_URL`, `MAIL_FROM` | – | E-Mail; ohne sie geben Admins Einladungslinks selbst weiter |
| `POSTGRES_PASSWORD`, `GARAGE_*`, `S3_*` | erzeugt | Geheimnisse aus `deploy/generate-secrets.sh` – aufbewahren; die Daten hängen daran |

Solange das Repository privat ist, braucht das Image zuerst eine Anmeldung: `docker login ghcr.io` mit einem
GitHub-Token, das Pakete lesen darf.

## Updates

```sh
git pull
docker compose -f deploy/compose.yaml --profile https pull
docker compose -f deploy/compose.yaml --profile https up -d
```

Datenbank-Migrationen laufen beim Start von notenwart. Vor einem Update sichern und das
[Changelog]({{ '/changelog/' | relative_url }}) lesen. Um bei einer Version zu bleiben, `NOTENWART_VERSION` setzen.

## Sicherungen

```sh
deploy/backup.sh /pfad/zu/sicherungen
```

schreibt `notenwart-<zeit>.tar.gz` mit der Datenbank (`pg_dump`) und allen gespeicherten Dateien, während notenwart
weiterläuft. Täglich ausführen (cron oder ein systemd-Timer) und die Archive vom Server wegkopieren – eine Sicherung
auf derselben Platte ist keine Sicherung. `deploy/.env` ist nicht im Archiv: eine Kopie davon sicher aufbewahren.

In eine laufende Installation zurückspielen (ersetzt deren aktuelle Daten):

```sh
deploy/restore.sh /pfad/zu/sicherungen/notenwart-….tar.gz
```

Geräte mit offline gespeicherten Stimmen gleichen sich beim nächsten Besuch mit dem zurückgespielten Stand ab.

## Noten erkennen

Beim Import kann notenwart Titel, Komponist, Arrangeur, Texter, Verlag und die Stimme („2. Trompete in B“) von der
ersten Seite jeder PDF lesen. Das Image enthält, was dafür nötig ist: `pdftotext` für digitale PDFs, `pdftoppm` und
Tesseract (Deutsch und Englisch) für Scans. Ohne das Image `poppler-utils`, `tesseract-ocr`, `tesseract-ocr-deu` und
`tesseract-ocr-eng` installieren; `NOTENWART_OCR_LANGUAGES` (Standard `deu+eng`) wählt die Sprachen für Tesseract.
Beim Start steht im Protokoll, was fehlt.

Eine externe KI kann die Seiten zusätzlich lesen – besser bei schlechten Scans und Handschrift, und sie liest auch
Schlüssel, Tonart und Takt. Sie ist **aus**, solange sie nicht in `deploy/.env` eingerichtet ist:

| Variable | |
|---|---|
| `NOTENWART_AI_PROVIDER` | `anthropic` (Messages API von Anthropic) oder `openai` (jede OpenAI-kompatible API: OpenAI, Mistral, ein lokales Ollama oder LM Studio …) |
| `NOTENWART_AI_KEY` | der API-Schlüssel (für lokale Server ohne Schlüssel nicht nötig) |
| `NOTENWART_AI_MODEL` | das Modell; für Anthropic `claude-opus-5-5`, wenn nicht gesetzt, für `openai` Pflicht – es muss Bilder lesen können |
| `NOTENWART_AI_URL` | nur für andere Endpunkte, z. B. `http://host.docker.internal:11434/v1` für Ollama auf dem Host |
| `NOTENWART_AI_IMAGES` | `false` schickt nur den Text der Seite, nicht ihr Bild |

Mit eingerichteter KI gehen das Bild jeder ersten Seite und ihr Text an diesen Anbieter. Prüfen Sie vorher seine
Bedingungen und die Auftragsverarbeitung; ein lokales Modell lässt alles auf den eigenen Rechnern. Die Erkennung macht
nur Vorschläge: Die Notenverwaltung sieht und korrigiert alles, bevor etwas gespeichert wird.

## Anmeldung mit Schulkonten

Personen können sich über OpenID Connect mit den vorhandenen Konten der Einrichtung anmelden – Microsoft 365 (Entra
ID), Google Workspace, Keycloak und andere Anbieter, die es sprechen. Hinein kommt nur, wer in notenwart schon
existiert, erkannt an der E-Mail-Adresse, die der Anbieter bestätigt; unterwegs wird niemand angelegt, also Personen
mit denselben Adressen anlegen (oder importieren). Passwörter und Anmeldelinks funktionieren weiter.

notenwart beim Anbieter als Webanwendung registrieren, mit der Redirect-URI
`<NOTENWART_PUBLIC_URL>/api/auth/sso/callback`, und in `deploy/.env` setzen:

| Variable | |
|---|---|
| `NOTENWART_SSO_ISSUER` | die Issuer-URL des Anbieters (seine `/.well-known/openid-configuration` muss erreichbar sein) |
| `NOTENWART_SSO_CLIENT_ID` | die Client-ID der Anwendung |
| `NOTENWART_SSO_CLIENT_SECRET` | ihr Client-Secret |
| `NOTENWART_SSO_NAME` | der Text auf dem Knopf, z. B. `Microsoft 365` |
| `NOTENWART_SSO_EMAIL_CLAIM` | der Claim im ID-Token mit der E-Mail-Adresse (`email`, wenn nicht gesetzt) |

Issuer: Microsoft Entra ID `https://login.microsoftonline.com/<Tenant-ID>/v2.0` (Entra schreibt die Adresse nur in
`email`, wenn die Person ein Postfach hat; `NOTENWART_SSO_EMAIL_CLAIM=preferred_username` nimmt den Anmeldenamen, der
meist gleich ist), Google Workspace `https://accounts.google.com` (die OAuth-App auf interne Nutzer stellen), Keycloak
`https://<host>/realms/<realm>`. IServ und andere Schulplattformen: ob und wie sie OpenID Connect anbieten, ist
OFFEN – beim Anbieter nachfragen. Getestet mit einem Ersatz-Anbieter, noch nicht mit diesen Diensten (OFFEN).

## Datenschutz

Jede Person lädt ihre Daten auf der Kontoseite herunter; Admins exportieren die von allen auf der Personen-Seite. Das
Löschen einer Person entfernt ihr Konto mit allem, was daran hängt, und wird (ohne ihre Daten) im *Löschprotokoll* auf
der Personen-Seite vermerkt. Nach einem Schuljahr markiert *Ehemalige auswählen* Personen, die in keinem Ensemble sind
und sich seit einem Jahr nicht angemeldet haben, um sie zu prüfen und auf einmal zu löschen.

notenwart speichert Namen, E-Mail-Adressen, Mitgliedschaften in Ensembles und für jede Person den Zeitpunkt der
letzten Anmeldung; kein Tracking, keine Telemetrie. Die Einrichtung, die einen Server betreibt, ist für die Daten
darauf verantwortlich und für die Rechte an den Noten, die dort geteilt werden. Betreibt ein Dienstleister notenwart
für eine Schule, braucht er mit ihr einen Vertrag zur Auftragsverarbeitung (OFFEN: Vorlage).
