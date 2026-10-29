
# Changelog

Every user-visible change gets a line under `## [Unreleased]`; a release turns
that section into the version's.

## [Unreleased]

- The web app is built with Vite and Svelte, without SvelteKit; the server is a workspace of crates (core, store,
  app, API, terminal interface) with the API description generated from the code (`api/openapi.json`).
- Strict Content-Security-Policy for the web app.
- Scans with JPEG 2000 or JBIG2 images show their pages: pdf.js gets its image decoders, standard fonts and
  character maps from the app itself (also offline).

## [0.1.0] – 2026-10-29

First public release.

- Library: works with details (composer, arranger, lyricist, publisher, work number, duration, tags, place in the
  paper archive), PDF upload to S3-compatible storage or a local directory, splitting PDFs into parts; search and
  tables with sorting, columns, filters and bulk actions.
- Import a whole folder of sheet music (a Google Drive download as ZIP, a ZIP or a folder), with "Recognise from
  contents" reading title, composer, arranger, publisher and part from the first page of each PDF – digital ones
  and scans (OCR); "Recognise parts" splits a PDF with all parts. Operators can add an external AI (Anthropic or any
  OpenAI-compatible API, also local); it is off unless configured.
- People and roles (admin, librarian, leader, member): sign-in with password, one-time links or single sign-on
  (OpenID Connect), invitations, import from a CSV, avatars.
- Ensembles with members, their instruments and part numbers, and works; "My music" lists one's parts; single
  parts for one member; each organisation's own instrument catalogue.
- Full-screen viewer with half-page turns, two pages side by side and page-turner pedals; parts kept on the device
  for playing offline; personal annotations and shared markings by leaders; printing with annotations.
- Programmes with start time, venue, breaks, durations and a printable running order; set list for the GEMA (or
  another performing rights society) as print and CSV.
- Paper archive: loans of printed parts with due dates and reminders.
- Recordings for works and parts, played in the viewer slower or faster, with A–B repeat.
- Guest links for substitutes, notifications in the app and by email, a system page with a setup checklist.
- Data protection: data export per person, deletion record, selecting former members.
- The web app speaks German and English; emails follow `NOTENWART_LANGUAGE`.
- Running a server: container image `ghcr.io/notenwart/notenwart`, `deploy/compose.yaml` with optional Caddy for
  HTTPS, backup and restore scripts; documentation site for notenwart.dev with a user guide in English and German.
