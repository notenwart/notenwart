-- SPDX-License-Identifier: AGPL-3.0-or-later
-- What a performing rights society's set list asks for besides composer, arranger and publisher: the lyricist and
-- the society's work number (e.g. GEMA-Werknummer).

ALTER TABLE works
    ADD COLUMN lyricist text,
    ADD COLUMN society_number text;
