-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Markings a leader shares with everyone of their ensemble who plays a part (bowings, breaths, cuts). One layer per
-- ensemble and part, so two ensembles playing the same work keep their own.

CREATE TABLE shared_annotations (
    ensemble_id  uuid NOT NULL REFERENCES ensembles ON DELETE CASCADE,
    part_id      uuid NOT NULL REFERENCES parts ON DELETE CASCADE,
    pages        jsonb NOT NULL DEFAULT '{}',
    updated_by   uuid REFERENCES users ON DELETE SET NULL,
    updated_at   timestamptz NOT NULL DEFAULT now(),
    -- Players hear about changes at most twice an hour, not after every stroke.
    notified_at  timestamptz,
    PRIMARY KEY (ensemble_id, part_id)
);
