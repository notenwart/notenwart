-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Guest links: someone without an account (a substitute player) sees chosen parts of an ensemble until a date.

CREATE TABLE guest_links (
    id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id       uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    ensemble_id           uuid NOT NULL REFERENCES ensembles ON DELETE CASCADE,
    -- Who it is for, as the leaders call them ("Max, trumpet substitute").
    name                  text NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    token_hash            bytea NOT NULL UNIQUE,
    part_ids              uuid[] NOT NULL,
    -- The last day the link works.
    expires_on            date NOT NULL,
    created_by            uuid REFERENCES users ON DELETE SET NULL,
    created_at            timestamptz NOT NULL DEFAULT now(),
    revoked_at            timestamptz,
    last_used_at          timestamptz,
    expiring_notified_at  timestamptz
);
CREATE INDEX guest_links_ensemble ON guest_links (ensemble_id);
