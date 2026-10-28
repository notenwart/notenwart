-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Notifications: what happened for whom, shown in the app and/or sent by email (push follows later), as each person
-- sets it per kind.

CREATE TABLE notifications (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    user_id          uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    kind             text NOT NULL,
    -- What the text is made of (work, part, programme …); texts are rendered in the reader's language.
    params           jsonb NOT NULL DEFAULT '{}',
    -- A path of the web app, e.g. /works/<id>.
    link             text,
    in_app           boolean NOT NULL DEFAULT true,
    email_due        boolean NOT NULL DEFAULT false,
    emailed_at       timestamptz,
    read_at          timestamptz,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX notifications_inbox ON notifications (user_id, created_at DESC) WHERE in_app;
CREATE INDEX notifications_mail ON notifications (created_at) WHERE email_due AND emailed_at IS NULL;

-- Per person and kind; kinds without a row use the defaults in notify.rs.
CREATE TABLE notification_settings (
    user_id  uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    kind     text NOT NULL,
    in_app   boolean NOT NULL,
    email    boolean NOT NULL,
    push     boolean NOT NULL,
    PRIMARY KEY (user_id, kind)
);

-- Reminders sent for a loan: before it is due, and once when it is overdue.
ALTER TABLE loans
    ADD COLUMN reminded_at timestamptz,
    ADD COLUMN overdue_reminded_at timestamptz;
