-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Each person's annotations on a part: strokes by page, stored as JSON (spec M2).

CREATE TABLE annotations (
    user_id     uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    part_id     uuid NOT NULL REFERENCES parts ON DELETE CASCADE,
    pages       jsonb NOT NULL DEFAULT '{}',
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, part_id)
);
