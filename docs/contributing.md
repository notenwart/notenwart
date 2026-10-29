---
title: Contributing
nav_order: 6
permalink: /contributing/
---

# Contributing

notenwart is one repository: the Rust workspace in `crates/`, the web app (Vite, Svelte 5, TypeScript) in `web/`, embedded
into the binary, the API contract in `api/`, deployment files in `deploy/` and this
documentation in `docs/`.

## Set up

Requirements: Rust (the toolchain in `rust-toolchain.toml`; rustup fetches it), Node.js 24 with pnpm, Docker.

```sh
docker compose -f deploy/compose.dev.yaml up -d   # PostgreSQL on :5432, Garage S3 on :3900
cp .env.example .env
cargo run -- serve                                # API on :8080, runs migrations first
pnpm --dir web install
pnpm --dir web dev                                # web app on :5173, proxies /api to :8080
```

Recognition needs `pdftotext`, `pdftoppm` and Tesseract with German and English (`poppler-utils`, `tesseract-ocr`,
`tesseract-ocr-deu`, `tesseract-ocr-eng`; on macOS `brew install poppler tesseract tesseract-lang`); without them
the server runs and says what is missing.

## The Rust workspace

Each layer uses only the ones below it:

| Crate | What |
|---|---|
| `notenwart-core` | The domain without I/O: roles, email texts, PDF pages and annotations |
| `notenwart-store` | Every SQL query, one function each in the feature's module; PostgreSQL (connection, `migrations/`) and the object store (S3 or a directory) |
| `notenwart-app` | Every command and query: takes the caller (`CurrentUser`) and plain input, checks the role, answers with plain data or an `AppError`; no HTTP |
| `notenwart-api` | The HTTP API (axum): routes, the session cookie, multipart, status codes; the embedded web app with its Content-Security-Policy; the API tests in `tests/` |
| `notenwart` | The binary: `serve`, `migrate` |

A new feature gets its queries in `notenwart-store`, its command (the rules, and which queries run together in a
transaction) in `notenwart-app`, and a thin handler in `notenwart-api` that only takes the request apart and turns the
answer into HTTP.

## Checks

Every change passes all of these before it is committed – CI runs the same:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                      # needs the dev database; every test gets its own
cargo deny check                # advisories, licences, sources (cargo install cargo-deny --locked)
pnpm --dir web lint
pnpm --dir web check
pnpm --dir web test
pnpm --dir web build
```

## Rules

- **English** in the repository – code, comments, docs, commits. The UI's source strings are English and translated
  in `web/src/lib/i18n/`; add every new string to the German dictionary `de.ts` (a test fails otherwise). The user
  guide exists in English (`docs/guide/`) and German (`docs/de/`) – change both.
- **The name** is always lowercase: notenwart, also at the start of a sentence.
<!-- REUSE-IgnoreStart -->
- **Licence header:** every new source file starts with `SPDX-License-Identifier: AGPL-3.0-or-later`; files in `api/`
  with `MIT`. Never copy server or web code into `api/` – it only describes the HTTP API.
<!-- REUSE-IgnoreEnd -->
- **The API is described in the code** (utoipa): a `#[utoipa::path]` on every handler in `notenwart-api`, `ToSchema`
  on every type it takes or gives. After a change, write the description and regenerate the web types:
  `cargo run -p notenwart -- openapi > api/openapi.json`, then `pnpm --dir web api:types` (CI checks both).
- **Every command checks the role** (in `notenwart-app`, via `CurrentUser`), and every new rule gets an API test in
  `crates/notenwart-api/tests/`.
- **Dependencies:** exact versions in the root `Cargo.toml`, each the newest that is at least two weeks old – run
  `scripts/check-age.py` after changing `Cargo.lock` (`--fix` sets young ones back); only the fix of a security
  advisory may be younger (listed in the script with its advisory).
- **SQL:** queries are checked at compile time against `.sqlx/`. After changing one, run
  `cargo sqlx prepare --workspace -- --all-targets` (needs the dev database and
  `cargo install sqlx-cli --no-default-features --features postgres,rustls`) and commit `.sqlx/`. Migrations are
  append-only: add a new one, never edit a committed one.
- **Privacy:** no tracking, no telemetry, no fonts or scripts from elsewhere; collect only what a feature needs –
  pupils are often minors. No personal data in tracked files, not even in tests or examples.
- **Unverified things** are marked `OPEN` in code and docs.
- **Changelog:** a line under `[Unreleased]` in `CHANGELOG.md` for every user-visible change.

## Commits

Contributions are accepted under the [Developer Certificate of Origin](https://developercertificate.org/) – sign off
every commit:

```sh
git commit -s
```

## Releases

Bump the version in `Cargo.toml` and `web/package.json` (the API description takes it from Cargo – regenerate
`api/openapi.json`), turn `[Unreleased]` in `CHANGELOG.md`
into the version's section, commit, then tag `vX.Y.Z`. The release workflow checks the tag against `Cargo.toml` and
the changelog, publishes the image
`ghcr.io/notenwart/notenwart:X.Y.Z` (and `:latest`) and creates a GitHub release from the changelog section.

## This documentation

The site on notenwart.dev is built from `docs/` with Jekyll and the Just the Docs theme and published by GitHub
Actions (`.github/workflows/pages.yml`, the shared workflow of falkoss/starterpack); pull requests only build it. Pages have
front matter with `title`, `nav_order` and `permalink`; German pages live in `docs/de/` with `parent: Deutsch`.
`docs/assets/` holds copies from [notenwart/brand](https://github.com/notenwart/brand) – copy them again after a
change there, never edit them here. The documentation's text is under CC BY 4.0.
