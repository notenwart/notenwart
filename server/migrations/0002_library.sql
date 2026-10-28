-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Library: instruments, works, uploaded source PDFs and the parts split from them (spec §3, M1 part 1).

CREATE TYPE instrument_family AS ENUM
    ('woodwind', 'brass', 'percussion', 'keyboard', 'strings', 'plucked', 'voice', 'other');

-- organization_id NULL: built-in, shared by every organisation.
CREATE TABLE instruments (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id  uuid REFERENCES organizations ON DELETE CASCADE,
    name             text NOT NULL,
    family           instrument_family NOT NULL,
    transposition    text,
    aliases          text[] NOT NULL DEFAULT '{}',
    sort             integer NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX instruments_name ON instruments (coalesce(organization_id, '00000000-0000-0000-0000-000000000000'), lower(name));

CREATE TABLE works (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id   uuid NOT NULL REFERENCES organizations ON DELETE CASCADE,
    title             text NOT NULL CHECK (length(title) BETWEEN 1 AND 300),
    composer          text,
    arranger          text,
    publisher         text,
    instrumentation   text,
    difficulty        smallint CHECK (difficulty BETWEEN 1 AND 6),
    duration_seconds  integer CHECK (duration_seconds >= 0),
    source            text,
    notes             text,
    tags              text[] NOT NULL DEFAULT '{}',
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX works_organization ON works (organization_id, lower(title));
CREATE INDEX works_tags ON works USING gin (tags);

CREATE TABLE source_files (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id      uuid NOT NULL REFERENCES works ON DELETE CASCADE,
    file_name    text NOT NULL,
    size         bigint NOT NULL,
    sha256       text NOT NULL,
    page_count   integer NOT NULL CHECK (page_count > 0),
    storage_key  text NOT NULL UNIQUE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (work_id, sha256)
);

CREATE TABLE parts (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id        uuid NOT NULL REFERENCES works ON DELETE CASCADE,
    source_id      uuid NOT NULL REFERENCES source_files ON DELETE CASCADE,
    label          text NOT NULL CHECK (length(label) BETWEEN 1 AND 120),
    instrument_id  uuid REFERENCES instruments ON DELETE SET NULL,
    voice          smallint CHECK (voice >= 1),
    pages          integer[] NOT NULL CHECK (cardinality(pages) > 0),
    storage_key    text NOT NULL UNIQUE,
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX parts_work ON parts (work_id);

INSERT INTO instruments (name, family, transposition, aliases, sort) VALUES
    ('Piccolo',          'woodwind',   NULL, '{Pikkolo,Piccoloflöte,Picc}', 10),
    ('Flute',            'woodwind',   NULL, '{Flöte,Querflöte,Fl}', 11),
    ('Recorder',         'woodwind',   NULL, '{Blockflöte}', 12),
    ('Oboe',             'woodwind',   NULL, '{Ob}', 13),
    ('English horn',     'woodwind',   'F',  '{Englischhorn,Cor anglais}', 14),
    ('Clarinet',         'woodwind',   'Bb', '{Klarinette,Clar,Cl,Klar}', 15),
    ('Eb clarinet',      'woodwind',   'Eb', '{Es-Klarinette}', 16),
    ('Bass clarinet',    'woodwind',   'Bb', '{Bassklarinette,Bass Clar}', 17),
    ('Bassoon',          'woodwind',   NULL, '{Fagott,Bsn,Fg}', 18),
    ('Soprano saxophone','woodwind',   'Bb', '{Sopransaxophon,Sopransax,Sop Sax}', 19),
    ('Alto saxophone',   'woodwind',   'Eb', '{Altsaxophon,Altsax,Alto Sax,A Sax}', 20),
    ('Tenor saxophone',  'woodwind',   'Bb', '{Tenorsaxophon,Tenorsax,Tenor Sax,T Sax}', 21),
    ('Baritone saxophone','woodwind',  'Eb', '{Baritonsaxophon,Baritonsax,Bari Sax}', 22),
    ('Horn',             'brass',      'F',  '{Waldhorn,French horn,Hn,Hr}', 30),
    ('Trumpet',          'brass',      'Bb', '{Trompete,Tpt,Trp}', 31),
    ('Flugelhorn',       'brass',      'Bb', '{Flügelhorn,Flgh}', 32),
    ('Cornet',           'brass',      'Bb', '{Kornett}', 33),
    ('Trombone',         'brass',      NULL, '{Posaune,Pos,Tbn}', 34),
    ('Bass trombone',    'brass',      NULL, '{Bassposaune}', 35),
    ('Euphonium',        'brass',      NULL, '{Bariton,Baritone,Tenorhorn,Euph}', 36),
    ('Tuba',             'brass',      NULL, '{Tb}', 37),
    ('Timpani',          'percussion', NULL, '{Pauken,Pauke,Timp}', 40),
    ('Percussion',       'percussion', NULL, '{Schlagwerk,Schlagzeug,Perc,Drums,Drum set}', 41),
    ('Mallets',          'percussion', NULL, '{Stabspiele,Glockenspiel,Xylophon,Xylophone,Vibraphon,Marimba}', 42),
    ('Piano',            'keyboard',   NULL, '{Klavier,Pno,Pf}', 50),
    ('Organ',            'keyboard',   NULL, '{Orgel}', 51),
    ('Keyboard',         'keyboard',   NULL, '{Keys,Synthesizer}', 52),
    ('Violin',           'strings',    NULL, '{Violine,Geige,Vl,Vn}', 60),
    ('Viola',            'strings',    NULL, '{Bratsche,Va}', 61),
    ('Cello',            'strings',    NULL, '{Violoncello,Vc}', 62),
    ('Double bass',      'strings',    NULL, '{Kontrabass,Bass,Kb,Cb}', 63),
    ('Guitar',           'plucked',    NULL, '{Gitarre,Git,Gtr}', 70),
    ('Electric bass',    'plucked',    NULL, '{E-Bass,Bass guitar,Bassgitarre}', 71),
    ('Harp',             'plucked',    NULL, '{Harfe}', 72),
    ('Ukulele',          'plucked',    NULL, '{Uke}', 73),
    ('Soprano',          'voice',      NULL, '{Sopran}', 80),
    ('Alto',             'voice',      NULL, '{Alt}', 81),
    ('Tenor',            'voice',      NULL, '{}', 82),
    ('Bass voice',       'voice',      NULL, '{Bass (Chor),Bariton (Chor)}', 83);
