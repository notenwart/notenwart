-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Avatars: the organisation's logo, an ensemble's logo or symbol (an emoji on a colour), a person's picture.

CREATE TABLE images (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    -- Whose image it is, for deciding who may see it: 'organization', 'ensemble' or 'user'.
    owner_kind       text NOT NULL CHECK (owner_kind IN ('organization', 'ensemble', 'user')),
    owner_id         uuid NOT NULL,
    content_type     text NOT NULL,
    size             integer NOT NULL,
    created_at       timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE organizations
    ADD COLUMN logo_id uuid REFERENCES images ON DELETE SET NULL,
    -- Pupils are often minors: the organisation decides whether people may show a picture.
    ADD COLUMN profile_pictures boolean NOT NULL DEFAULT true;
ALTER TABLE ensembles
    ADD COLUMN logo_id uuid REFERENCES images ON DELETE SET NULL,
    ADD COLUMN symbol text CHECK (length(symbol) BETWEEN 1 AND 16),
    ADD COLUMN color text CHECK (color ~ '^#[0-9a-fA-F]{6}$');
ALTER TABLE users ADD COLUMN avatar_id uuid REFERENCES images ON DELETE SET NULL;
