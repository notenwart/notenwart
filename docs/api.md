---
title: API
nav_order: 5
permalink: /api/
---

# API

Everything the web app does goes through a JSON API under `/api` on the server. Its contract is
[`api/openapi.yaml`](https://github.com/notenwart/notenwart/blob/main/api/openapi.yaml) (OpenAPI 3.1) – the source of
truth for every client. Unlike the rest of notenwart it is under the **MIT licence**, so clients – for example a
native app – can be written and generated from it under any licence.

Open the file in any OpenAPI viewer or generate a client from it (the web app uses `openapi-typescript`).

## Conventions

- **Base path:** `/api`, on the server's own address, e.g. `https://noten.example.org/api/health`.
- **Sign-in** (`POST /api/auth/login`, sign-in links, single sign-on) sets an HTTP-only session cookie. Every endpoint
  except `/health`, `/auth/*` and the guest pages needs it and acts on the signed-in person's organisation.
- **Origin:** requests that change something and carry an `Origin` header must come from the server's own origin.
- **Roles** decide what each endpoint allows: admin, librarian, leader, member – see the
  [user guide]({{ '/guide/' | relative_url }}). What a member may see is decided on the server; a client never has
  to filter.
- **JSON** with `camelCase` fields; ids are UUIDs, times RFC 3339, dates `YYYY-MM-DD`.
- **Errors** come as `{"error": "<code>", "message": "<text>"}` with the HTTP status; codes include
  `unauthorized` (401), `forbidden` (403), `not_found` (404), `conflict` (409) and `invalid` (422). Messages are
  English; things a person may not see answer 404, not 403.
- **Files** (PDFs, audio, images) are served by the API after the same checks; audio supports byte ranges.

## Areas

| Tag | What |
|---|---|
| `auth` | setup, sign-in, the signed-in person |
| `people` | the organisation's people and invitations (admin) |
| `ensembles` | ensembles, members and what they play, works, programmes, single parts |
| `library` | works, uploaded PDFs, parts, recognition, annotations |
| `catalogue` | instrument families and instruments |
| `recordings` | full and practice recordings |
| `images` | logos, symbols, pictures |
| `guests` | guest links and the guest pages |
| `notifications` | the inbox and each person's settings |
| `loans` | the paper archive's loans |
| `privacy` | data export and the deletion record |
| `system` | server facts, test email, setup checklist (admin) |

The API follows notenwart's version (`info.version`); changes are listed in the
[changelog]({{ '/changelog/' | relative_url }}).
