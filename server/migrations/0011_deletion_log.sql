-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Proof that a person's data was deleted (GDPR accountability) without keeping the data: the account's id, who
-- deleted it, when, and how many records went with it.

CREATE TABLE deletion_log (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    user_id          uuid NOT NULL,
    deleted_by       uuid REFERENCES users ON DELETE SET NULL,
    deleted_at       timestamptz NOT NULL DEFAULT now(),
    records          jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX deletion_log_organization ON deletion_log (organization_id, deleted_at);
