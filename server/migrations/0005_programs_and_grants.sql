-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Programmes (concerts, set lists) of an ensemble, and parts granted to single members (spec M2).

CREATE TABLE programs (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    ensemble_id  uuid NOT NULL REFERENCES ensembles ON DELETE CASCADE,
    name         text NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    date         date,
    notes        text,
    created_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX programs_ensemble ON programs (ensemble_id, date);

-- Works of a programme in playing order; they are always works of the programme's ensemble.
CREATE TABLE program_works (
    program_id  uuid NOT NULL REFERENCES programs ON DELETE CASCADE,
    work_id     uuid NOT NULL REFERENCES works ON DELETE CASCADE,
    position    integer NOT NULL,
    PRIMARY KEY (program_id, work_id)
);

-- A part one member of an ensemble may see in addition to what they play. Leaving the ensemble removes it.
CREATE TABLE part_grants (
    ensemble_id  uuid NOT NULL,
    user_id      uuid NOT NULL,
    part_id      uuid NOT NULL REFERENCES parts ON DELETE CASCADE,
    granted_by   uuid REFERENCES users ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (ensemble_id, user_id, part_id),
    FOREIGN KEY (ensemble_id, user_id) REFERENCES ensemble_members ON DELETE CASCADE
);

-- As in 0003, plus grants. A grant only counts while the ensemble has the part's work.
CREATE OR REPLACE VIEW visible_parts AS
SELECT DISTINCT m.user_id, p.id AS part_id, m.ensemble_id
FROM ensemble_members m
JOIN ensemble_works ew ON ew.ensemble_id = m.ensemble_id
JOIN parts p ON p.work_id = ew.work_id
WHERE m.leader
   OR EXISTS (
        SELECT 1 FROM ensemble_member_parts mp
        WHERE mp.ensemble_id = m.ensemble_id AND mp.user_id = m.user_id
          AND mp.instrument_id = p.instrument_id
          AND (mp.voice IS NULL OR p.voice IS NULL OR mp.voice = p.voice))
   OR EXISTS (
        SELECT 1 FROM part_grants g
        WHERE g.ensemble_id = m.ensemble_id AND g.user_id = m.user_id AND g.part_id = p.id);
