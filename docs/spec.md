---
title: Design (spec)
nav_order: 7
permalink: /spec/
---

# notenwart – specification

Status: draft, 2026-10-23. Items marked `OPEN` are undecided or unverified.

## 1. Goal

An organisation (music school, orchestra, choir, band) runs notenwart itself or has a service provider host it. It
uploads sheet music as PDF, splits it into parts and assigns it to ensembles. Every person sees only the parts they
need – on phone, tablet or music stand, also offline.

First pilot: a regional music school that today shares its sheet music through cloud-drive folders.

notenwart provides software only. **The operator is responsible for the rights to the sheet music** uploaded and
shared; the software enforces nothing but offers neutral tools (an optional source/licence field, operator control
over share links, an optional watermark).

## 2. Roles

| Role | May |
|---|---|
| **admin** (organisation) | people and invitations, everything below |
| **librarian** | change the library (works, files, parts); create, change and delete every ensemble; decide who leads |
| **leader** (teacher, conductor) | read the whole library; in the ensembles they lead: members and what they play, works |
| **member** (pupil, player) | see their own parts ("My music"), their ensembles, themselves as a member |
| guest (link, later) | one shared work or programme, time-limited |

The role is per organisation; *leading* is per ensemble (`ensemble_members.leader`) and needs the role leader or
higher. Leaders cannot change who leads. Members of an ensemble only see themselves in its member list (pupils are
often minors). Admins cannot remove their own admin role or delete themselves.

**Who sees a part:** a member sees a part when one of their ensembles has the part's work and they play the part's
instrument in that ensemble – any part number (none given) or the same one; a part without a number is for every
player of the instrument. Parts without an instrument (full score) are for leaders. Leaders see every part of their
ensembles' works; library readers (leader role and up) may open any part. The rule lives in the SQL view
`visible_parts`. Leaders can also grant a single member a single part of the ensemble's works ("single parts", e.g. the
full score for an accompanist); the grant ends when the member leaves or the work leaves the ensemble. Parent
accounts for minors: `OPEN`.

## 3. Data model (sketch)

- **Organization** – tenant. Self-hosted usually one, a provider several; multi-tenant in the schema from the start.
- **User**, **Membership** (user ↔ organisation, role)
- **Ensemble** (orchestra, class, band) and **EnsembleMember** (user, instrument(s), part(s), e.g. "Clarinet 2")
- **Instrument** – each organisation's own catalogue: families (named and ordered by the organisation) and
  instruments with transposition and other names; starts as a copy of the built-in catalogue
- **Work** – title, composer, arranger, publisher, instrumentation, difficulty, tags, duration, optional
  source/licence
- **SourceFile** – uploaded original PDF in object storage (immutable, checksum)
- **Part** – label, optional instrument and part number, and pages of one source file in reading order (each at
  most once); rendered into its own PDF (the full score is a part too). Parts built from several source files:
  `OPEN`.
- **Assignment** – work ↔ ensemble; visibility follows automatically (member plays X → sees parts for X), manual
  overrides possible
- **Program** – ordered list of works for an ensemble (concert, set list), with an optional date
- **PartGrant** – one more part for one member of an ensemble
- **ShareLink** – token, target, expiry, optional password
- **Annotation** (later) – layer above the PDF, personal or ensemble-wide

## 4. Milestones

### M0 – skeleton ✅

Rust server (axum, sqlx) with migrations and `/api/health`; SvelteKit SPA embedded into the binary; service worker
caches the app shell; OpenAPI contract; Docker image and Compose; CI.

### M1 – MVP for the pilot

1. **Upload and split** ✅
   - PDF upload (several files, drag and drop, with progress) into S3-compatible storage or a local directory;
     duplicates (same SHA-256 within a work) are refused; upload limit `NOTENWART_MAX_UPLOAD_MB` (default 200)
   - split editor: page thumbnails rendered in the browser (pdf.js, lazily), click and Shift-click to select pages in
     any order → instrument, part number, label; instrument and part number guessed from the file name
     ("Trompete_2.pdf" → Trumpet 2); pages already used by other parts are marked
   - part PDFs rendered on the server: the selected pages get a new page tree, nothing is re-encoded; outlines and
     forms of the source are dropped
   - work metadata, tags, search (title, composer, arranger) and tag filter
   - built-in instrument catalogue (39 instruments, English names with German and English aliases), copied per
     organisation
2. **Roles and ensembles** ✅
   - first-run setup creates the first admin and names the organisation (only while there are no users)
   - sign-in with email and password (argon2id) or a one-time sign-in link by email (20 minutes); sessions in an
     HTTP-only, SameSite=Lax cookie (30 days, sliding); state-changing requests with a foreign `Origin` are refused;
     sign-in and link requests are rate-limited per address (in memory)
   - admins add people and get an invitation link (7 days, one use) to hand over or have emailed; the invited person
     signs in with it and chooses a password; a new link replaces the old one
   - SMTP is optional (`SMTP_URL`, `MAIL_FROM`); without it, links go to the log and admins hand them over
   - ensembles with members, the instruments and part numbers they play, works; "My music" lists the visible parts
     by ensemble and work
   - still open: OIDC (`OPEN`: which providers the pilot uses)
3. **Viewer and offline** ✅
   - full-screen viewer (pdf.js, Fullscreen API where the browser has it, screen kept on with the Wake Lock API):
     tap the left or right third or swipe to turn, tap the middle for the controls (they hide after 4 s)
   - Bluetooth page turners: arrow keys, Page Up/Down, Space and Enter – the keys common pedals send
   - layouts: one page; half-page turns (the top half already shows the next page while the bottom half still
     shows the end of the current one, with a dashed line between); two pages side by side (default on wide
     landscape screens); the choice is remembered per device; changing it keeps the page being read
   - pages are rendered at screen resolution and the next ones ahead, so turns are instant
   - turning past the last page opens the ensemble's next part, before the first page the previous one (at its end)
   - "My music" grouped by ensemble and work; programmes (set lists) come later
   - offline per ensemble ("Available offline"): its visible parts are stored in IndexedDB; every visit while
     online syncs – new and re-rendered parts (`updatedAt`) are fetched, parts no longer visible are deleted;
     offline, "My music" and the viewer work from the stored copies and the cached app shell
   - the device forgets everything on sign-out, when another user signs in, and when the server answers 401
   - limits: copies on a device that never comes online again stay there; storage persistence is requested but
     the browser decides

Not in M1: cloud-drive import, OMR, annotations, public links, native apps, audio.

A server has one organisation for now (created on first start, named during setup); several organisations per
server (hosting providers): `OPEN`.

### M2 – pilot readiness

1. **German UI** ✅ – every screen in German and English, part labels suggested from the instrument names
   ("Trompete 2"; instruments are named in the organisation's catalogue, not translated in the UI); emails (sign-in links, invitations) in the server's language,
   `NOTENWART_LANGUAGE=de|en`.
2. **Import people from a table** ✅ – admins upload a CSV (Excel, LibreOffice, Numbers; semicolon, comma
   or tab; UTF-8 or Windows-1252) with name and email, optionally role, ensemble, instrument and part number, in
   English or German column names (first and last name columns are joined). The server checks every row first and
   changes nothing unless all rows are fine; missing ensembles are created, existing people are only assigned, one
   person may appear once per ensemble. New people get invitations; the links download as a CSV for handing out.
   Up to 2000 rows per import.
3. **Programmes** ✅ – an ensemble's concerts or set lists: name, optional date, works in playing order
   (adding a work to a programme gives it to the ensemble; taking it from the ensemble takes it out of its
   programmes). "My music" shows each programme with the user's parts in playing order before all works, and the
   viewer turns on to the next piece of the programme.
4. **Single parts** ✅ – see §2.
5. **Annotations** ✅ – pen (three colours), highlighter, eraser and undo in the viewer, on whole pages
   (not during a half-page turn). Strokes are stored per person and part in page coordinates (0–1) and line widths
   as fractions of the page width, so they fit every screen. Each change is saved on the device at once and sent to
   the server after 1.5 s; offline edits are sent on the next visit and win; across devices the last write wins.
   Annotations shared with an ensemble (by its leaders): `OPEN`. When a part's pages change, annotations stay by
   page number (`OPEN`: move them with the pages).
6. **Releases and operations** ✅ – a tag `v*` publishes the image to the GitHub Container Registry and a
   GitHub release with the changelog section; Compose with optional Caddy for HTTPS; backup and restore scripts
   (`pg_dump` and an S3 sync of the bucket); `docs/operations.md`. arm64 images: `OPEN`.
7. **Everyday use** ✅
   - colours light, dark or as on the device (header button, account page), remembered per device
   - lists (people, library, ensembles, instruments) as tables: sortable, choose columns, filter, pages of 25/50/100;
     the settings are remembered per device
   - bulk actions: people – add to or remove from an ensemble (with instrument and part number), change role, new
     invitation links (CSV), delete; library – give to an ensemble, delete
   - every choice is a searchable list with a maximum height that shows at most 150 matches (large part lists)
   - suggestions while typing: composer, arranger, publisher, instrumentation and tags from the works already
     entered; tags as chips
   - own instrument catalogue: admins and librarians rename, add, group and order instruments and families. On
     the first start each organisation gets a copy of the built-in catalogue in the server's language
     (`NOTENWART_LANGUAGE`, German keys B/Es), with the English names kept as other names; existing parts and
     members move to the copies. Instruments still used by parts or members, and families with instruments, cannot
     be deleted. Merging folds a duplicate into the instrument that stays (`POST /instruments/{id}/merge`): parts,
     members' assignments (one per part number) and recordings move over, its names become other names, it goes.
   - confirmations in the app's own dialog, short notices after actions, a menu on narrow screens
8. **Import a folder** ✅ – for moving an existing collection (the pilot keeps its music in Google Drive): drop or
   choose a ZIP (Drive's folder download), PDFs or a folder. Read in the browser (ZIP with `fflate`); the folder of a
   PDF names its work, the folders above become tags, one surrounding folder is ignored, PDFs directly in the chosen
   folder are works of their own. A file becomes a part with all its pages when its name names an instrument
   ("Trompete 2"), a part without instrument when it looks like a score ("Partitur", "Score"), otherwise it is only
   uploaded and split later on the work page. When the library has a work with the same title, nothing happens
   until someone decides: add the files to it, create a separate work, or leave it out (the default). A preview lets staff
   change titles, tags, what each file becomes, instrument, part number and label, or leave things out; files over
   the upload limit and non-PDFs are listed as skipped. The upload runs file by file through the normal API and
   reports files already in the library and failures. Reading Google Drive directly: `OPEN`.
9. **Recognising sheet music** ✅ – "Recognise from contents" in the import reads the first page of each PDF on the
   server (`POST /recognize`, nothing stored): the text layer of digital PDFs (`pdftotext -bbox-layout`), otherwise
   OCR (`pdftoppm` at 300 dpi + Tesseract, `deu+eng`). Rules on the positioned lines: labels ("Musik:", "Komponist:",
   "Arr.", "Bearb.", "Satz:", "Text:" and English ones) give composer, arranger and lyricist; the largest line in
   the top third the title, a smaller centred line below it the subtitle; an unlabelled name flush right near the
   top the composer; a short line near the top naming an instrument of the catalogue the part, with its number
   ("2. Trompete", "Clarinet II", "1st Trumpet"); a footer with ©, "Verlag" or "Edition" the publisher. An optional
   external AI chosen by the operator (Anthropic or any OpenAI-compatible API, `NOTENWART_AI_*`) gets the page image
   and the lines and answers in a fixed JSON form; its fields win, and only it reads clef, key and time signature
   from the start of the first staff. In the preview, the file name keeps naming the part when it does; the page
   fills in parts the name does not give, composer, arranger and publisher of the work, and the title of works made
   from single files – a folder's name is only challenged by a suggestion. Two pages are read at a time. Clef and
   key without an AI, and storing them with the parts: `OPEN`. On a work's page, the first file uploaded to a work
   without composer, arranger and publisher is read (`POST /sources/{id}/recognize`), and "Read details from the
   PDF" does it on demand: title, composer, arranger and publisher are offered field by field – empty fields
   ticked, existing ones only replaced when ticked.
10. **Splitting a PDF with all parts** ✅ – "Recognise parts" in the split editor (`POST /sources/{id}/split`) reads the
   top fifth of every page (text layer; OCR of just that strip for scanned pages; at most 400 pages) and finds the
   part names as on first pages ("2. Trompete in B", "Direktion"). A page with a header starts a part unless it
   repeats the previous header; pages without a header continue the current part; pages before the first header
   form their own group (left unchecked). Staff check, rename, change instrument or number, move a proposal into the
   page editor ("Adjust"), and create the checked ones in one go. Pages already in parts start unchecked. No AI per
   page (cost); `OPEN` as an option for poor scans.
11. **Programme planning** ✅ – a programme has a start time and a venue besides name, date and notes, and its items
   are works and breaks (interval, speech; with an optional duration) in playing order, each with a note ("with
   repeat", "soloist: …"). Leaders reorder by dragging or with arrows; the card shows each item's start time, the
   total and the expected end, and names works without a duration, which can be entered right there (it is the
   work's own duration). "Running order" is a printable page (`/ensembles/{id}/programs/{programId}`) with numbers,
   times, composer and arranger, durations and the total. `PUT /programs/{id}/items` replaces the former
   `PUT /programs/{id}/works`.
12. **Set list for the performing rights society** ✅ (in Germany the GEMA "Musikfolge") – per programme a printable
   page and a CSV (semicolon, UTF-8 with BOM for Excel): event (name, date, start, venue, organiser = organisation,
   ensemble) and per work number, title, composer, lyricist, arranger, publisher, the society's work number and
   duration; breaks are left out. Works get the fields lyricist and work number for it. Works missing composer,
   duration or (when arranged) publisher are listed with links. The fields follow GEMA's paper forms for single
   events with live music (work number, title, composer, arranger, publisher); the exact fields and import formats of
   GEMA's online portal are `OPEN`, so the page says it is not GEMA's own form.
13. **Paper archive and loans** ✅ – a work has a place in the archive ("Schrank 3, Fach B"). Staff lend printed parts
   from a work's page: what (suggested: "complete set" and the work's part labels), how many copies, to whom (a
   person of the organisation – `GET /borrowers` gives staff names only – or anyone by name), until when. The loans
   page lists open loans (returned ones on request), overdue ones marked, and returns one or several at once. Every
   user sees their own open loans on "My music" (`GET /me/loans`). Reminders by email: `OPEN`.
14. **Data protection tools** ✅ – every user downloads everything stored about them as JSON from their account page
   (`GET /me/export`: account, ensembles with instruments, single parts, annotations, loans, sign-ins; GDPR art. 15
   and 20), admins do it for anyone (`GET /users/{id}/export`). Deleting people (singly or in bulk) goes through one
   path: memberships, sessions, annotations and single parts go with the account; names and notes on their returned
   loans are erased; people with parts still out cannot be deleted (409 / skipped). Each deletion is recorded in
   `deletion_log` with the account id, who deleted it, when and how many records went – no name or email
   (`GET /deletions`, admin). "Select former members" on the people page picks people in no ensemble who have not
   signed in for a year (never admins), for bulk deletion after the school year. Automatic deletion after a set
   time: `OPEN`. Legal review of the export's completeness: `OPEN` (draft, no legal advice).
15. **Single sign-on** ✅ – OpenID Connect (authorization code with PKCE, `NOTENWART_SSO_*`), so schools sign in with
   Microsoft 365, Google Workspace and the like. Discovery on first use; state, nonce and PKCE verifier in a
   short-lived HttpOnly cookie scoped to `/api/auth/sso`; the ID token from the token endpoint is checked for issuer,
   audience, expiry, nonce and a verified email (signature check replaced by TLS to the token endpoint as OIDC Core
   3.1.3.7 allows). Only existing people get in, found by email; unknown accounts end at `/login?sso=unknown`. No
   automatic creation of accounts (deliberate: minors, invitations decide who is in). Tested with a stand-in
   provider; real providers (Entra ID, Google, IServ) `OPEN`.
16. **Notifications** ✅ – kinds: new parts for me (a part I see was created, a work came to my ensemble, a single part
   was granted), programme created or changed, loan due tomorrow and once when overdue, markings shared by a leader,
   guest link running out. Recipients are decided in the request, records written in the background (never failing
   the action); whoever caused it is not notified. Each person sets per kind: in the app (bell with count, list,
   read/unread), by email, push (shown, disabled: `OPEN`). Defaults: everything in the app, email for all but
   markings. A worker every minute sends loan reminders and one email per person with everything collected for at
   least two minutes, in the server's language. Texts are rendered from kind and parameters in the reader's
   language (app) or the server's (email).
17. **Guest links** ✅ – leaders and staff make a link for a substitute without an account: name, chosen parts of the
   ensemble's works (one by one, or all of an instrument and part number in a programme), last day (default: the
   day after the next concert, else two weeks; at most a year). The token is random, stored only as a hash and shown
   once. The guest page (`/guest/<token>`) lists the parts, opens them in the viewer (no annotations, no offline
   copies) and offers downloads; it records when it was last opened. Expired, revoked and unknown links look the
   same (404). The maker is notified the day before it expires. Watermarking guest downloads: `OPEN`.
18. **Recordings** ✅ – audio only (MP3, AAC/M4A incl. voice memos, WAV, FLAC, Ogg/Opus, WebM audio, AIFF; detected
   from the file's first bytes, MP4/WebM only with an audio name; videos refused). A recording belongs to a work and
   is either a full recording (recorded performance or generated) or a practice recording of one part or of an
   instrument/voice (choir scores hold all voices in one part); duration measured by the uploading browser.
   Whoever sees a part of the work hears its recordings; staff and leaders of an ensemble with the work add, rename
   and delete them. Files are served with HTTP ranges (seeking, Safari). The work page lists them with a player; the
   viewer shows a player bar while reading (the part's own practice recordings first): play, ±5 s, position, speed
   0.5–1.25× with the pitch kept, A–B loop. `OPEN`: playing MIDI files directly (needs a bundled synthesizer; export
   audio from notation software meanwhile), pages turning along with a recording (time markers per page),
   recordings offline, guests hearing recordings, ogg/opus on Safari before iOS 17.
19. **Avatars** ✅ – the organisation has a logo (header, sign-in page) and a name admins change on the organisation
   page; ensembles a logo (larger ones) or a symbol – an emoji from a set, on one of eight colours – set by their
   managers; people a picture or their initials on a colour derived from the name. Images are PNG, JPEG or WebP,
   at most 2 MB, judged by content (no SVG: scripts); the web app scales them down (512 px pictures, 1024 px logos)
   and re-encodes as WebP before uploading. Visibility: the organisation's logo to anyone, ensemble logos to the
   organisation's people, a person's picture to themselves, staff and the leaders of their ensembles. Admins can
   switch profile pictures off (pupils are often minors): then nobody's picture is shown and none uploaded. Images
   are served as immutable (a new image gets a new id).
20. **Shared markings** ✅ – leaders (and staff) write markings for everyone of an ensemble who plays a part: one layer
   per ensemble and part (two ensembles with the same work keep theirs apart), same strokes as personal annotations,
   an overlay above the PDF – the file itself never changes. In the viewer's annotation tools leaders switch
   between "For me" and "For everyone in <ensemble>"; players see the layers of their ensembles beneath their own
   strokes, read only, and can hide them ("Leaders' markings"). Saved 1.5 s after the last stroke; players are
   notified at most twice an hour. Shared markings offline: `OPEN`. Touch and stylus work in the web app (pointer
   events); the native app follows later.
21. **Printing with annotations** ✅ – "Print / PDF" in the viewer opens `GET /parts/{id}/annotated` in a new tab: a
   copy of the part with the leaders' markings (if shown) and one's own drawn in as vector paths (lopdf: the page's
   content wrapped in `q … Q`, a stream of paths appended, the highlighter through an ExtGState with 35 % opacity and
   multiply; positions converted from the shown page for CropBox/MediaBox and Rotate 0/90/180/270). Nothing is
   stored; the original stays as it was.
22. **System page and setup checklist** ✅ (admins, nav "System") – version, public URL, email language and sender,
   storage kind and bytes stored (sources, recordings, images), upload limit, how sheet music is read (text, OCR,
   AI), single sign-on, counts. A test email to oneself. The checklist: logo (optional), instruments checked
   (ticked by hand), ensembles, people, music, email configured, test email (ticked by a successful one), single
   sign-on (optional), backups (ticked by hand); recognised steps cannot be ticked. "My music" reminds admins of open
   steps. The organisation page (name, logo, profile pictures) is reached from the checklist.
23. **Large libraries** ✅ – measured with 5,000 works (2026-10-23, local): the list's JSON 2 MB in 166 ms, the page
   ready in 217 ms, filtering 35 ms, sorting 82 ms in the browser. Tables therefore keep sorting and filtering in the
   browser; instead responses are compressed (Brotli/gzip above 1 KB; not PDFs, audio or images, which are already
   compressed – and audio keeps byte ranges): the 5,000-work list goes over the wire as 130 KB. The server's search
   (`GET /works?q=`) also looks into publisher, lyricist, instrumentation, work number and tags. Paging on the server:
   `OPEN`, should a library pass about 20,000 works.

### Later

- OMR with Audiveris (AGPL-3.0) → MusicXML: transposition, part extraction, playback; optional worker container
- scan clean-up (deskew, crop, contrast)
- annotations; cloud-drive import, ZIP export; lending of paper copies
- native app (likely Flutter) as a separate MIT-licensed repository built on `api/openapi.yaml`

## 5. Architecture

```
Browser/PWA (TypeScript) ──HTTPS──► notenwart (one Rust binary) ──► PostgreSQL
                                     ├─ REST API (/api, OpenAPI)    └─► S3-compatible storage (M1)
                                     ├─ embedded web app
                                     └─ worker (thumbnails, part PDFs; M1)
```

- **Server:** axum, sqlx (PostgreSQL; queries checked at compile time against `.sqlx/`), migrations on start.
  Part PDFs are rendered within the request (`spawn_blocking`); a job queue in PostgreSQL
  (`FOR UPDATE SKIP LOCKED`, no Redis) and a `worker` mode follow when work gets slow (OCR/OMR, thumbnails).
- **PDF:** page extraction with `lopdf`. Thumbnails are rendered by the browser; server-side thumbnails (pdfium or
  MuPDF) only if needed – `OPEN`.
- **Storage:** `STORAGE=s3` (Garage, MinIO, AWS …) or `STORAGE=fs` (a local directory). Keys:
  `<organization>/sources/<id>.pdf`, `<organization>/parts/<id>.pdf`.
- **Files** are streamed through the server after an access check, so the object store never has to be reachable
  from browsers. Presigned URLs as an option for large installations: `OPEN`.
- **Web app:** SvelteKit with `adapter-static` (SPA, no Node on the server), pdf.js, service worker + IndexedDB for
  offline parts. UI in German and English (`web/src/lib/i18n/`): English source strings as keys, German
  dictionary, a unit test fails when a UI string lacks a translation; the browser language decides until the user
  picks one in their account. Server error messages stay English and are translated in the web app where they are
  fixed strings.
- **API contract:** `api/openapi.yaml` (MIT) is the source of truth; web types are generated with
  `openapi-typescript`. Checking the server against it in CI: `OPEN` (e.g. `utoipa`-generated spec compared to the
  file).
- **Deployment:** one container image; Compose with PostgreSQL and a single-node Garage, which a one-shot
  `garage-init` (Python, admin API v2) prepares idempotently: layout, bucket, access key. A Nix module later.

## 6. Privacy

Pupils are often minors: store only name, sign-in, ensembles, instruments. Self-hosting keeps data with the school;
a hosting provider needs a data processing agreement with it. No tracking, no telemetry, no external fonts or CDNs.
Per-person export and deletion.
