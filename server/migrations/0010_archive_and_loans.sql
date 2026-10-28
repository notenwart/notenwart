-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The paper archive: where a work's printed copies are kept, and who has borrowed which of them.

ALTER TABLE works ADD COLUMN archive_location text;

CREATE TABLE loans (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    work_id          uuid NOT NULL REFERENCES works ON DELETE CASCADE,
    -- What was lent, as written: "Trompete 2", "complete set" …
    item             text NOT NULL CHECK (length(item) BETWEEN 1 AND 120),
    quantity         integer NOT NULL DEFAULT 1 CHECK (quantity BETWEEN 1 AND 999),
    -- A member of the organisation, or someone outside it by name.
    borrower_id      uuid REFERENCES users ON DELETE SET NULL,
    borrower_name    text NOT NULL CHECK (length(borrower_name) BETWEEN 1 AND 120),
    lent_on          date NOT NULL DEFAULT current_date,
    due_on           date,
    returned_on      date,
    note             text,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX loans_open ON loans (organization_id) WHERE returned_on IS NULL;
CREATE INDEX loans_work ON loans (work_id);
CREATE INDEX loans_borrower ON loans (borrower_id);
