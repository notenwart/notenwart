-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Each organisation keeps its own instrument catalogue: families it can rename and order, and instruments it can add,
-- rename and remove. The built-in instruments (organization_id NULL) stay as the template for new organisations; the
-- server copies them on start (catalogue.rs) and moves existing references over to the copies.

CREATE TABLE instrument_families (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    name             text NOT NULL,
    sort             integer NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX instrument_families_name ON instrument_families (organization_id, lower(name));

-- An organisation's instruments belong to one of its families; the built-in template keeps the enum.
ALTER TABLE instruments
    ADD COLUMN family_id uuid REFERENCES instrument_families ON DELETE RESTRICT,
    ALTER COLUMN family DROP NOT NULL,
    ADD CONSTRAINT instruments_family CHECK ((organization_id IS NULL) = (family_id IS NULL));
