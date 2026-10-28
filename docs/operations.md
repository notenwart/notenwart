---
title: Running a server
nav_order: 4
permalink: /operations/
---

# Running notenwart

How to run notenwart on a server of your own: install, HTTPS, mail, updates, backups. Everything below runs from a
checkout of this repository on the server (only `deploy/` is used; the image comes from the GitHub Container
Registry). Auf Deutsch: [Server betreiben]({{ '/de/betrieb/' | relative_url }}).

## Requirements

- A Linux server or VM with Docker and the Compose plugin; 2 GB RAM and a few GB of disk for a start (PDFs grow
  with the library).
- A domain name pointing to the server (A/AAAA record), ports 80 and 443 open, if Caddy should get the HTTPS
  certificate.
- Optional: an SMTP account for sign-in links and invitations.

## Install

```sh
git clone https://github.com/notenwart/notenwart
cd notenwart
deploy/generate-secrets.sh > deploy/.env
```

Add the server's settings to `deploy/.env`:

```sh
NOTENWART_DOMAIN=noten.example.org
NOTENWART_PUBLIC_URL=https://noten.example.org
NOTENWART_LANGUAGE=de
SMTP_URL=smtps://user:password@mail.example.org
MAIL_FROM=notenwart <no-reply@example.org>
```

Start with Caddy in front (it fetches and renews the certificate itself):

```sh
docker compose -f deploy/compose.yaml --profile https up -d
```

Open `https://noten.example.org` and create the first admin account **right away** – the first visitor gets it.

Without `--profile https`, notenwart listens on `127.0.0.1:8080` only; put your own reverse proxy in front, or set
`NOTENWART_PORT` (e.g. `0.0.0.0:8080`) for a test on the local network.

| Variable | Default | What |
|---|---|---|
| `NOTENWART_DOMAIN` | – | Domain for Caddy (`--profile https`) |
| `NOTENWART_PUBLIC_URL` | `http://localhost:8080` | Where people reach notenwart; used in emailed links, `https` makes the session cookie Secure |
| `NOTENWART_LANGUAGE` | `en` | Language of emails: `de` or `en` |
| `NOTENWART_VERSION` | `latest` | Image tag, e.g. `0.1.0` to pin a version |
| `NOTENWART_PORT` | `127.0.0.1:8080` | Host address and port of notenwart itself |
| `SMTP_URL`, `MAIL_FROM` | – | Mail; without them, admins hand out invitation links themselves |
| `POSTGRES_PASSWORD`, `GARAGE_*`, `S3_*` | generated | Secrets from `deploy/generate-secrets.sh` – keep them; the data depends on them |

While the repository is private, the image needs a login first: `docker login ghcr.io` with a GitHub token that may
read packages.

## Updates

```sh
git pull
docker compose -f deploy/compose.yaml --profile https pull
docker compose -f deploy/compose.yaml --profile https up -d
```

Database migrations run when notenwart starts. Make a backup before updating; read `docs/changelog.md`. To stay on
a version, set `NOTENWART_VERSION`.

## Backups

```sh
deploy/backup.sh /path/to/backups
```

writes `notenwart-<time>.tar.gz` with the database (`pg_dump`) and every stored PDF, while notenwart keeps running.
Run it daily (cron or a systemd timer) and copy the archives off the server – a backup on the same disk is not a
backup. `deploy/.env` is not in the archive: keep a copy of it in a safe place.

Restore into a running installation (replaces its current data):

```sh
deploy/restore.sh /path/to/backups/notenwart-….tar.gz
```

Devices that kept parts offline sync with the restored state on their next visit.

## Recognising sheet music

When music is imported, notenwart can read title, composer, arranger, lyricist, publisher and the part
("2. Trompete in B") from the first page of each PDF. The image contains what it needs: `pdftotext` for digital PDFs,
`pdftoppm` and Tesseract (German and English) for scans. Running without the image, install `poppler-utils`,
`tesseract-ocr`, `tesseract-ocr-deu` and `tesseract-ocr-eng`; `NOTENWART_OCR_LANGUAGES` (default `deu+eng`) picks
the Tesseract languages. The log says on start what is missing.

An external AI can read the pages as well – better on poor scans and handwriting, and it also reads clef, key and
time signature. It is **off** unless configured in `deploy/.env`:

| Variable | |
|---|---|
| `NOTENWART_AI_PROVIDER` | `anthropic` (Anthropic's Messages API) or `openai` (any OpenAI-compatible API: OpenAI, Mistral, a local Ollama or LM Studio …) |
| `NOTENWART_AI_KEY` | the API key (not needed for local servers without one) |
| `NOTENWART_AI_MODEL` | the model; `claude-opus-5-5` for Anthropic unless set, required for `openai` – it must read images |
| `NOTENWART_AI_URL` | only for other endpoints, e.g. `http://host.docker.internal:11434/v1` for Ollama on the host |
| `NOTENWART_AI_IMAGES` | `false` sends only the text found on the page, not its image |

With an AI configured, the image of each first page and its text go to that provider. Check its terms and data
processing before switching it on; a local model keeps everything on your own machines. Recognition only suggests:
staff see and correct everything before anything is saved.

## Single sign-on

People can sign in with the organisation's existing accounts through OpenID Connect – Microsoft 365 (Entra ID),
Google Workspace, Keycloak and other providers that speak it. Only people who already exist in notenwart get in,
matched by the email address the provider vouches for; nobody is created on the way, so add people (or import them)
with the same addresses. Passwords and sign-in links keep working alongside.

Register notenwart with the provider as a web application with the redirect URI
`<NOTENWART_PUBLIC_URL>/api/auth/sso/callback` and set in `deploy/.env`:

| Variable | |
|---|---|
| `NOTENWART_SSO_ISSUER` | the provider's issuer URL (its `/.well-known/openid-configuration` must be reachable) |
| `NOTENWART_SSO_CLIENT_ID` | the application's client id |
| `NOTENWART_SSO_CLIENT_SECRET` | its client secret |
| `NOTENWART_SSO_NAME` | the button text, e.g. `Microsoft 365` |
| `NOTENWART_SSO_EMAIL_CLAIM` | the ID token claim with the email address (`email` unless set) |

Issuers: Microsoft Entra ID `https://login.microsoftonline.com/<tenant id>/v2.0` (Entra puts the address into
`email` only when the user has a mail address; `NOTENWART_SSO_EMAIL_CLAIM=preferred_username` uses the sign-in name,
which is usually the same), Google Workspace `https://accounts.google.com` (set the OAuth app to internal users),
Keycloak `https://<host>/realms/<realm>`. IServ and other school platforms: whether and how they offer OpenID Connect
is `OPEN` – check with the provider. Tested against a stand-in provider, not yet against these services (`OPEN`).

## Privacy

People download their own data on their account page; admins export anyone's on the people page. Deleting a person
removes their account with everything attached and is recorded (without their data) under "Deletion record" on the
people page. After a school year, "Select former members" picks people in no ensemble who have not signed in for a
year, to check and delete in one go.

notenwart stores names, email addresses, ensemble memberships and, for each person, the time of the last sign-in;
no tracking, no telemetry. The organisation running a server is responsible for the data on it and for the rights
to the sheet music shared there. A hosting provider running notenwart for a school needs a data processing
agreement with it (OPEN: template).
