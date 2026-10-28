---
title: Looking after the library
parent: User guide
nav_order: 4
permalink: /guide/library/
---

# Looking after the library

Librarians (and admins) keep the library: works, their PDFs and parts, the instrument list and the paper archive.
They can also create, change and delete every ensemble and decide who leads it.

## Works

*Library* lists every work as a table: sort by a column, choose columns, filter by title, composer, tag and more, pages
of 25 to 100 – remembered per device. Select several works to *Give to ensemble …* or delete them.

*Title of a new work* and *Add work* create one. On its page, *Edit details*: title, composer, arranger, lyricist,
publisher, instrumentation, difficulty, duration (min:sec), tags, place in the archive, work number (GEMA), source /
licence, notes. While you type, names and tags already used in the library are suggested.

## PDFs and parts

A work keeps its **files** – the PDFs as they were uploaded – and its **parts**, each made from pages of a file.

1. **Upload:** drop PDFs onto the work's page or choose them; several at a time are fine. The same file twice is
   refused. The server sets a size limit per file (200 MB unless changed).
2. **Details from the PDF:** the first file of a work without composer, arranger and publisher is read straight away;
   *Read details from the PDF* does it on demand. notenwart offers what it found field by field – *Take over* or
   *Discard*; fields that already have a value are only replaced when you tick them.
3. **Split into parts:** for a file with several parts:
   - *Recognise parts* reads the top of every page and proposes the parts ("2. Trompete in B", "Direktion"): a new
     heading starts a part, pages without one continue it. Check, rename, change instrument or number, *Adjust* a
     proposal page by page, and create the ticked ones in one go.
   - Or by hand: click pages to select them (Shift-click for a range, in any order), give instrument, part number
     and label, *Create part*. Pages already used in other parts are marked.

   A file that is just one part becomes a part with all its pages. A part without an instrument is a score – for the
   leaders.

Recognition works on digital PDFs and on scans (OCR). It only suggests – you see and correct everything before it
is saved. Your server may also use an external AI for poor scans; see [Running a
server]({{ '/operations/#recognising-sheet-music' | relative_url }}).

## Importing a folder

To move an existing collection – for example from Google Drive (right-click the folder → *Download* gives a ZIP) –
open *Import a folder …* in the library and drop a ZIP file, PDFs or a folder.

- Each folder with PDFs becomes a **work**, named after the folder; the folders above become **tags**. PDFs directly
  in the chosen folder become works of their own.
- A file named after an instrument ("Trompete 2.pdf") becomes that part; one that looks like a score ("Partitur",
  "Score") a part without instrument; anything else is only uploaded and split later on the work's page.
- *Recognise from contents* also reads the first page of every PDF – title, composer, arranger, publisher, the part.
  The file name keeps naming the part when it does; for a folder's title the page only makes a suggestion.
- **Works that exist already** (same title) are left out until you decide: *Add the files to the existing work* or
  *Create a separate work*.
- In the preview you change titles, tags, what each file becomes, instrument, number and label, or leave things out.
  Files that are too large or not PDFs are listed as skipped.

The import runs file by file; keep the page open until it says it is finished. Files already in the library are
reported and skipped; files with several parts are listed under *Still to split*.

## Instruments

*Instruments* is your organisation's own list, grouped in families: rename, add (with key – B♭, E♭, F … – and other
names, e.g. "Alto Sax, A-Sax"), group and order. It starts as a copy of notenwart's built-in list. Instruments in
use cannot be deleted; **merge** a duplicate into the instrument that stays instead – parts, members and recordings
move over and its names become other names of the one that stays.

## Paper archive and loans

A work's *place in the archive* ("cabinet 3, shelf B") is set under *Edit details*. To lend printed parts, use the
work's *Paper archive* section: what ("complete set" or a part), how many copies, to whom – a person of the
organisation or anyone by name – and until when; then *Lend*.

*Loans* (in the library) lists everything out, overdue loans marked; *Mark as returned* takes back one or several.
Borrowers see their loans on *My music* and are reminded the day before and once when overdue.
