-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Planning a concert: when and where it starts, breaks and announcements between the works, a note per item.

ALTER TABLE programs
    ADD COLUMN start_time time,
    ADD COLUMN venue text;

ALTER TABLE program_works ADD COLUMN note text;

-- Items of a programme that are not works (interval, speech). They share the positions with program_works.
CREATE TABLE program_breaks (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    program_id        uuid NOT NULL REFERENCES programs ON DELETE CASCADE,
    position          integer NOT NULL,
    label             text NOT NULL CHECK (length(label) BETWEEN 1 AND 120),
    duration_seconds  integer CHECK (duration_seconds BETWEEN 0 AND 86400),
    note              text
);
CREATE INDEX program_breaks_program ON program_breaks (program_id);
