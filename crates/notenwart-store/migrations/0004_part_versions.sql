-- SPDX-License-Identifier: AGPL-3.0-or-later
-- When a part's PDF was last rendered; offline copies on devices compare it (spec §4, M1 part 3).

ALTER TABLE parts ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();
UPDATE parts SET updated_at = created_at;
