# CLAUDE.md – project rules for Claude Code

## Context

notenwart: self-hosted sheet music library for music schools, orchestras and ensembles. Free software under
**AGPL-3.0-or-later**, except `api/` (the OpenAPI contract), which is **MIT**. GitHub organization `notenwart`,
repository `notenwart`. A project of
falkOSS – Falcon Lake Open Source Software gUG (haftungsbeschränkt) i. G.
Design and milestones: **docs/spec.md** – read before any work and keep it up to date.

## Working rules

- **Language:** everything in this repository is English (code, comments, docs, commits). The UI's source strings
  are English and translated in `web/src/lib/i18n/` (German first – the pilot is a German music school).
  Talk to the maintainer in German.
- **Name:** always lowercase – **notenwart**, also at the start of a sentence and in titles ([notenwart/brand](https://github.com/notenwart/brand)).
  Environment variables keep `NOTENWART_`.
<!-- REUSE-IgnoreStart -->
- **Licence header:** every new own source file (Rust, TypeScript, Svelte, SQL, YAML, shell) starts with
  `SPDX-License-Identifier: AGPL-3.0-or-later`; files in `api/` use `MIT`; texts in `docs/` are CC BY 4.0 (REUSE.toml),
  copies of the brand's artwork CC BY-ND 4.0. CI checks REUSE and a DCO sign-off on every commit.
<!-- REUSE-IgnoreEnd -->
- **The API contract is MIT:** never copy server or web code into `api/`; it may only describe the HTTP API.
  Change `api/openapi.yaml` first, then implement it in `crates/` and run `pnpm --dir web api:types`.
- **Public from day one:** no personal data in tracked files – no names, user names, home paths, hostnames, keys,
  and no real data of pilot users (pupils, teachers).
- **Copyright of sheet music is the operator's responsibility.** The software offers neutral tools and enforces
  nothing.
- **Privacy:** no tracking, no telemetry, no externally loaded fonts or scripts; collect only what a feature needs
  (pupils are often minors). Members never see other members' data; emails only for admins.
- **Layers:** `notenwart-core` (domain, no I/O) ← `notenwart-store` (PostgreSQL, objects) ← `notenwart-app` (every
  command and query; takes the caller `&CurrentUser` and plain input, returns plain data or `AppError`, no HTTP types)
  ← `notenwart-api` (axum handlers only take the request apart and map the answer to HTTP) ← `notenwart` (binary).
  Interfaces (API, later the TUI) use `notenwart-app` only.
- **Every command checks the role** via `CurrentUser` (`is_admin`, `is_staff`, `reads_library`); add an API test
  for each new rule in `crates/notenwart-api/tests/`.
- **Never delete (your own actions):** move files to `_to_delete/` (git-ignored) instead.
- **Verified before claimed:** mark what can't be verified as `OPEN` in code/docs.
- **After every change:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `cargo deny check`;
  `pnpm --dir web lint`, `pnpm --dir web check`, `pnpm --dir web test`, `pnpm --dir web build`. Commit only when
  green. After SQL changes: `cargo sqlx prepare --workspace -- --all-targets` and commit `.sqlx/` (Docker and CI
  build offline from it).
- **Commits:** signed off (`git commit -s`, DCO). Push only after asking the maintainer.
- **Dependencies:** exact versions (`=x.y.z`) in `[workspace.dependencies]`, each the newest at least 14 days old:
  run `scripts/check-age.py` (`--fix`) after changing `Cargo.lock`; a security fix may be younger (`SECURITY_FIXES`
  there). A `cargo deny` finding is ignored only with a written reason in `deny.toml`.
- **Migrations** are append-only: never edit one that has been committed; add a new one.
- **Releases:** bump the version in `Cargo.toml` and `web/package.json`, turn `[Unreleased]` in
  `CHANGELOG.md` into the version's section, commit, then tag `vX.Y.Z` (the release workflow checks both).

## Layout

```
Cargo.toml            workspace: exact dependency versions, lints; rust-toolchain.toml, rustfmt.toml, deny.toml
crates/notenwart-core/   domain without I/O: Role, email texts (email.rs), PDF pages and annotations (pdf.rs, lopdf)
crates/notenwart-store/  PostgreSQL connection and migrations/ (append-only), object store: S3 or local directory
                         (objects.rs), the default organisation (organization.rs)
crates/notenwart-app/    every command and query – the only layer interfaces use; App (state) and Config in lib.rs
  src/auth.rs         setup, sign-in, one-time links, sessions, roles (CurrentUser = the caller)
  src/people.rs       users and invitations (admin), bulk actions on people
  src/catalogue.rs    each organisation's instrument families and instruments, copied from the built-in ones
  src/ensembles.rs    ensembles, members, works, "my music", part visibility
  src/mail.rs         SMTP (optional; links go to the log without it)
  src/import.rs       import of people from a table (CSV in the web app)
  src/programs.rs     programmes of an ensemble (works and breaks), single parts granted to members
  src/loans.rs        paper archive: loans of printed parts
  src/privacy.rs      data export per person, deleting people with a deletion record
  src/sso.rs          sign-in through the organisation's identity provider (OpenID Connect)
  src/system.rs       admins: server facts, test email, setup checklist
  src/images.rs       avatars: organisation logo, ensemble logos and symbols, profile pictures
  src/recordings.rs   audio recordings of works and parts, with byte ranges
  src/guests.rs       guest links: parts for substitutes without an account
  src/notify.rs       notifications: recording, inbox, settings, email digests and loan reminders (worker)
  src/annotations.rs  each person's strokes on a part
  src/library.rs      works, suggestions (facets), tags, source files, parts
  src/recognize.rs    reading first pages: pdftotext / Tesseract, rules for title, composer, part …
  src/ai.rs           optional external AI for reading pages (Anthropic or OpenAI-compatible)
  src/files.rs        uploads as interfaces hand them in
crates/notenwart-api/    axum: one module per feature with its routes and thin handlers; auth.rs (session cookie,
                         SignedIn extractor), error.rs (AppError → status + JSON), files.rs (multipart, PDF
                         responses), web.rs (embedded web app from web/dist, CSP and security headers, SPA fallback)
  tests/              API tests (#[sqlx::test], in-memory store), PDF tests
crates/notenwart/        the binary (clap): `serve`, `migrate`
scripts/check-age.py  checks that every locked crate is at least 14 days old
.sqlx/                offline query data for sqlx
web/                  Vite + Svelte 5 SPA/PWA, built to web/dist; index.html → src/main.ts → src/App.svelte (shell, route guard)
  src/routes.ts       the routes (path → page in src/pages/, lazily loaded); src/lib/router.svelte.ts: page, goto, links
  src/service-worker.ts  offline shell; built as its own entry by service-worker.plugin.ts (files to cache injected)
  public/             copied as is: theme.js (before first paint), manifest, robots.txt, copies from notenwart/brand
  src/lib/components/ SourceSplitter (split editor), Thumbnail, WorkForm, MemberRow; shared: DataTable (sort, columns, pages,
                      selection), Select (searchable choice), Autocomplete, TagInput, Dialogs (confirm, notices)
  src/lib/session.svelte.ts  signed-in user, role helpers; route guard in App.svelte
  src/lib/viewer.ts   page turning (modes, keys, taps) – pure, unit-tested; components/Viewer.svelte renders
  src/lib/offline.ts  IndexedDB store and sync of offline parts; wiped on sign-out
  src/lib/i18n/       t('English text') and the German dictionary de.ts – add every new UI string there
  src/lib/csv.ts      reading spreadsheet exports for the import
  src/lib/bulk-import.ts  reading a folder/ZIP of sheet music and planning works and parts (pages/WorksImport.svelte)
  src/pages/View.svelte  the viewer page (stored copy first if current, else the server)
api/openapi.yaml      API contract (MIT)
deploy/               compose.yaml (full stack, optional Caddy), compose.dev.yaml (PostgreSQL + Garage for development),
                      garage/ (config, init script), generate-secrets.sh, backup.sh, restore.sh
docs/                 the site for notenwart.dev (GitHub Pages, Jekyll + Just the Docs, docs/_config.yml): guide/ (user
                      guide) and de/ (the same in German – change both), operations.md (running a server; German:
                      de/betrieb.md), api.md, contributing.md, spec.md (specification and milestones);
                      published by GitHub Actions (pages.yml); assets/ are copies from notenwart/brand
CHANGELOG.md          a line under [Unreleased] for every user-visible change
```
