-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Tenants, accounts and roles (spec §2, §3). Works, parts and ensembles follow with milestone 1.

CREATE TABLE organizations (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9][a-z0-9-]{1,62}$'),
    name        text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email          text UNIQUE,
    display_name   text NOT NULL,
    password_hash  text,
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX users_email_lower ON users (lower(email));

CREATE TYPE organization_role AS ENUM ('admin', 'librarian', 'leader', 'member');

CREATE TABLE memberships (
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    user_id          uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    role             organization_role NOT NULL DEFAULT 'member',
    created_at       timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, user_id)
);
