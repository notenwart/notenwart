-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Recordings of a work: full recordings (a performance, or generated from the score) and practice recordings of one
-- part or voice, recorded or generated (e.g. from MIDI). Audio only.

CREATE TABLE recordings (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id   uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    work_id           uuid NOT NULL REFERENCES works ON DELETE CASCADE,
    -- 'full' (the whole work) or 'practice' (one part or voice).
    kind              text NOT NULL CHECK (kind IN ('full', 'practice')),
    -- 'recorded' or 'generated'.
    origin            text NOT NULL CHECK (origin IN ('recorded', 'generated')),
    title             text NOT NULL CHECK (length(title) BETWEEN 1 AND 120),
    -- A practice recording belongs to a part, or to an instrument/voice (choir scores have all voices in one part).
    part_id           uuid REFERENCES parts ON DELETE SET NULL,
    instrument_id     uuid REFERENCES instruments ON DELETE SET NULL,
    voice             smallint CHECK (voice >= 1),
    file_name         text NOT NULL,
    content_type      text NOT NULL,
    size              bigint NOT NULL,
    sha256            text NOT NULL,
    duration_seconds  real CHECK (duration_seconds >= 0),
    storage_key       text NOT NULL UNIQUE,
    created_by        uuid REFERENCES users ON DELETE SET NULL,
    created_at        timestamptz NOT NULL DEFAULT now(),
    UNIQUE (work_id, sha256)
);
CREATE INDEX recordings_work ON recordings (work_id);
