-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Sign-in, ensembles and who plays what (spec §2, §3, M1 part 2).

-- Everyone signs in with an email address.
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
ALTER TABLE users ADD COLUMN last_login_at timestamptz;

-- Only hashes of tokens are stored; the token itself exists in the cookie or link.
CREATE TABLE sessions (
    token_hash       bytea PRIMARY KEY,
    user_id          uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    created_at       timestamptz NOT NULL DEFAULT now(),
    last_seen_at     timestamptz NOT NULL DEFAULT now(),
    expires_at       timestamptz NOT NULL
);
CREATE INDEX sessions_user ON sessions (user_id);

CREATE TYPE token_purpose AS ENUM ('login', 'invitation');

CREATE TABLE login_tokens (
    token_hash  bytea PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    purpose     token_purpose NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    used_at     timestamptz
);
CREATE INDEX login_tokens_user ON login_tokens (user_id);

CREATE TABLE ensembles (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    name             text NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    description      text,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ensembles_name ON ensembles (organization_id, lower(name));

CREATE TABLE ensemble_members (
    ensemble_id  uuid NOT NULL REFERENCES ensembles ON DELETE CASCADE,
    user_id      uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    leader       boolean NOT NULL DEFAULT false,
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (ensemble_id, user_id)
);
CREATE INDEX ensemble_members_user ON ensemble_members (user_id);

-- What a member plays in an ensemble; voice NULL means every part number of the instrument.
CREATE TABLE ensemble_member_parts (
    ensemble_id    uuid NOT NULL,
    user_id        uuid NOT NULL,
    instrument_id  uuid NOT NULL REFERENCES instruments ON DELETE CASCADE,
    voice          smallint CHECK (voice >= 1),
    FOREIGN KEY (ensemble_id, user_id) REFERENCES ensemble_members ON DELETE CASCADE
);
CREATE UNIQUE INDEX ensemble_member_parts_unique
    ON ensemble_member_parts (ensemble_id, user_id, instrument_id, coalesce(voice, 0));

CREATE TABLE ensemble_works (
    ensemble_id  uuid NOT NULL REFERENCES ensembles ON DELETE CASCADE,
    work_id      uuid NOT NULL REFERENCES works ON DELETE CASCADE,
    added_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (ensemble_id, work_id)
);
CREATE INDEX ensemble_works_work ON ensemble_works (work_id);

-- Parts a user may see as a member: their ensemble has the work and they play the instrument there (any part
-- number, or the same one). Leaders see every part of their ensembles' works. Admins and librarians see everything;
-- that is checked in the server, not here.
CREATE VIEW visible_parts AS
SELECT DISTINCT m.user_id, p.id AS part_id, m.ensemble_id
FROM ensemble_members m
JOIN ensemble_works ew ON ew.ensemble_id = m.ensemble_id
JOIN parts p ON p.work_id = ew.work_id
WHERE m.leader
   OR EXISTS (
        SELECT 1 FROM ensemble_member_parts mp
        WHERE mp.ensemble_id = m.ensemble_id AND mp.user_id = m.user_id
          AND mp.instrument_id = p.instrument_id
          AND (mp.voice IS NULL OR p.voice IS NULL OR mp.voice = p.voice));
