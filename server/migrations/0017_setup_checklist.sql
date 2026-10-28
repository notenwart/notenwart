-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Steps of setting notenwart up that admins tick off themselves (the others are recognised from the data).

ALTER TABLE organizations ADD COLUMN setup_done text[] NOT NULL DEFAULT '{}';
