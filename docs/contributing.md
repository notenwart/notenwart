---
title: Contributing
nav_order: 6
permalink: /contributing/
---

# Contributing

notenwart is one repository: the Rust server in `server/`, the web app (SvelteKit, TypeScript) in `web/`, embedded
into the server binary, the API contract in `api/`, deployment files in `deploy/`, the brand kit in `brand/` and this
documentation in `docs/`.

## Set up

Requirements: Rust (stable), Node.js 24 with pnpm, Docker.

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

## Checks

Every change passes all of these before it is committed – CI runs the same:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                      # needs the dev database; every test gets its own
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
- **Licence header:** every new source file starts with `SPDX-License-Identifier: AGPL-3.0-or-later`; files in `api/`
  with `MIT`. Never copy server or web code into `api/` – it only describes the HTTP API.
- **API first:** change `api/openapi.yaml`, then implement it in `server/` and regenerate the web types with
  `pnpm --dir web api:types`.
- **Every handler checks the role**, and every new rule gets an API test in `server/tests/`.
- **SQL:** queries are checked at compile time against `.sqlx/`. After changing one, run
  `cargo sqlx prepare --workspace -- --all-targets` (needs the dev database and
  `cargo install sqlx-cli --no-default-features --features postgres,rustls`) and commit `.sqlx/`. Migrations are
  append-only: add a new one, never edit a committed one.
- **Privacy:** no tracking, no telemetry, no fonts or scripts from elsewhere; collect only what a feature needs –
  pupils are often minors. No personal data in tracked files, not even in tests or examples.
- **Unverified things** are marked `OPEN` in code and docs.
- **Changelog:** a line under `[Unreleased]` in `docs/changelog.md` for every user-visible change.

## Commits

Contributions are accepted under the [Developer Certificate of Origin](https://developercertificate.org/) – sign off
every commit:

```sh
git commit -s
```

## Releases

Bump the version in `Cargo.toml`, `web/package.json` and `api/openapi.yaml`, turn `[Unreleased]` in the changelog
into the version's section, commit, then tag `vX.Y.Z`. The release workflow checks the tag against `Cargo.toml` and
the changelog, publishes the image
`ghcr.io/notenwart/notenwart:X.Y.Z` (and `:latest`) and creates a GitHub release from the changelog section.

## This documentation

The site on notenwart.dev is built by GitHub Pages from `docs/` (Jekyll with the Just the Docs theme). Pages have
front matter with `title`, `nav_order` and `permalink`; German pages live in `docs/de/` with `parent: Deutsch`.
`docs/assets/` holds copies of the brand files.
