<p align="center">
  <picture>
    <source srcset="docs/assets/notenwart-lockup-on-dark.svg" media="(prefers-color-scheme: dark)">
    <img src="docs/assets/notenwart-lockup-on-light.svg" alt="notenwart" width="420">
  </picture>
</p>

# notenwart

Self-hosted sheet music library for music schools, orchestras and ensembles. Upload scores as PDF, split them into
parts, organise them by ensemble, and let every player see exactly the parts they need – on phone, tablet or music
stand, online and offline.

> **Status:** 0.1.0, the first public release – made for a pilot at a music school. Library with PDF import,
> splitting and recognition, roles and ensembles, a full-screen viewer that works offline, annotations, programmes,
> loans, recordings and guest links. See [CHANGELOG.md](CHANGELOG.md) and [docs/spec.md](docs/spec.md).

A project of [falkOSS](https://falkoss.org).

## Running it

```sh
deploy/generate-secrets.sh > deploy/.env
docker compose -f deploy/compose.yaml up --build -d
```

Running it on a server – HTTPS, mail, updates, backups – is described in [docs/operations.md](docs/operations.md).
How to use it – for players, leaders, the library and admins – is in the user guide, in English
([docs/guide/](docs/guide/index.md)) and German ([docs/de/](docs/de/index.md)).

Then open http://localhost:8080 and set up the first admin account – do this right away, the first visitor gets
it. This starts notenwart, PostgreSQL and Garage (S3-compatible storage). Any other S3 service works too
(`STORAGE=s3`, `S3_ENDPOINT`, `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`), or a local directory
(`STORAGE=fs`, `STORAGE_PATH`).

Put notenwart behind a reverse proxy with HTTPS and set these in `deploy/.env`:

| Variable | What |
|---|---|
| `NOTENWART_PUBLIC_URL` | Where people reach the server, e.g. `https://noten.example.org`; used in emailed links, and `https` marks the session cookie Secure |
| `SMTP_URL` | Optional, for sign-in links and invitations, e.g. `smtps://user:password@mail.example.org` or `smtp://mail.example.org:587?tls=required`. Without it, admins hand out invitation links themselves |
| `MAIL_FROM` | Sender, e.g. `notenwart <noten@example.org>` |
| `NOTENWART_LANGUAGE` | Language of emails: `de` or `en` (default). The web app follows each user's browser or choice |

## Development

Requirements: Rust (the toolchain in `rust-toolchain.toml`; rustup fetches it), Node.js 24 with pnpm, Docker (for PostgreSQL).

```sh
docker compose -f deploy/compose.dev.yaml up -d   # PostgreSQL on :5432, Garage S3 on :3900
cp .env.example .env
cargo run -- serve                                # API on :8080, runs migrations first
pnpm --dir web install
pnpm --dir web dev                                # web app on :5173, proxies /api to :8080
```

| Path | What |
|---|---|
| `crates/notenwart-core/` | The domain without I/O: roles, email texts, PDF pages and annotations |
| `crates/notenwart-store/` | PostgreSQL (connection, migrations in `migrations/`, applied on start) and the object store |
| `crates/notenwart-app/` | Every command and query, with the caller's role checked – the one layer the interfaces use |
| `crates/notenwart-api/` | The HTTP API under `/api` (axum) and the embedded web app with its Content-Security-Policy |
| `crates/notenwart-tui/` | The operator console in the terminal (`notenwart tui`) |
| `crates/notenwart/` | The binary: `serve`, `migrate`, `openapi`, `tui` |
| `web/` | Web app / PWA (Vite, Svelte 5, TypeScript), built to `web/dist` and embedded into the binary |
| `api/openapi.json` | The API description, generated from the code (`notenwart openapi`) – **MIT-licensed**, the base for every client |
| `deploy/` | Docker Compose files |
| `docs/` | Documentation for notenwart.dev – user guide (English and German), operations, API, contributing, spec; `docs/assets/` holds copies from [notenwart/brand](https://github.com/notenwart/brand) |

The API is described in the code (utoipa). After changing it: `cargo run -p notenwart -- openapi > api/openapi.json`,
then `pnpm --dir web api:types`.
After changing SQL, refresh the offline query data (needs the dev database and
`cargo install sqlx-cli --no-default-features --features postgres,rustls`):

```sh
cargo sqlx prepare --workspace -- --all-targets
```

Tests: `cargo test` (needs the dev database; every test gets its own) and `pnpm --dir web test`. Dependencies:
`cargo deny check`, and `scripts/check-age.py` after changing `Cargo.lock` (each crate at least two weeks old).

## Licence

notenwart is free software under the [GNU AGPL-3.0-or-later](LICENSE). The API specification in `api/` is under the
[MIT licence](api/LICENSE), so clients can be written under any licence. The name and logo are not covered by these
licences.

Operators are responsible for the rights to the sheet music they upload and share; notenwart only provides the
software.

Contributions are welcome under the [Developer Certificate of Origin](https://developercertificate.org/): sign off
every commit (`git commit -s`).
