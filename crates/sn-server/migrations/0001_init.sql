-- Scoutnet 2 – prototypschema. All data i prototypen är syntetisk.

CREATE EXTENSION IF NOT EXISTS ltree;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Organisationsträdet. path = förälderns path + egen etikett (ltree) ger snabba
-- frågor av typen "allt under X" (path <@ X).
CREATE TABLE body (
    id                   uuid PRIMARY KEY DEFAULT uuidv7(),
    kind                 text NOT NULL CHECK (kind IN ('organisation','region','network','district','corps','corps_district','group','section','troop','patrol')),
    parent_id            uuid REFERENCES body(id),
    path                 ltree NOT NULL UNIQUE,
    number               integer,
    name                 text NOT NULL,
    troop_type           text CHECK (troop_type IN ('familj','baver','sparare','upptackare','aventyrare','utmanare','rover','annan')),
    min_age              smallint,
    max_age              smallint,
    municipality         text,
    meeting_weekday      smallint CHECK (meeting_weekday BETWEEN 1 AND 7),
    meeting_start        time,
    meeting_duration_min smallint,
    meeting_location     text,
    status               text NOT NULL DEFAULT 'active' CHECK (status IN ('active','inactive')),
    created_at           timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX body_path_gist ON body USING gist (path);
CREATE INDEX body_parent_idx ON body (parent_id);

CREATE TABLE person (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    member_no          integer NOT NULL UNIQUE,
    first_name         text NOT NULL,
    last_name          text NOT NULL,
    nickname           text,
    birth_date         date NOT NULL,
    -- Prototyp: endast testpersonnummer i klartext. Produktion: envelope-krypterat + blindindex.
    national_id        text,
    national_id_kind   text NOT NULL DEFAULT 'personnummer' CHECK (national_id_kind IN ('personnummer','samordningsnummer','none')),
    sex                text NOT NULL DEFAULT 'unknown' CHECK (sex IN ('unknown','male','female','other')),
    email              text,
    phone              text,
    street             text,
    postcode           text,
    city               text,
    protected_identity boolean NOT NULL DEFAULT false,
    can_help           boolean NOT NULL DEFAULT false,
    created_at         timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX person_name_trgm ON person USING gin (lower(first_name || ' ' || last_name) gin_trgm_ops);
CREATE INDEX person_birth_idx ON person (birth_date);

-- Medlemskap i en kår, temporalt (valid). Öppet intervall = pågående.
CREATE TABLE membership (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id         uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    group_id          uuid NOT NULL REFERENCES body(id),
    status            text NOT NULL CHECK (status IN ('active','waiting','pending','ended')),
    is_primary        boolean NOT NULL DEFAULT true,
    valid             daterange NOT NULL,
    waiting_since     date,
    wished_troop_type text,
    end_reason        text
);
CREATE INDEX membership_group_idx ON membership (group_id, status) WHERE upper_inf(valid);
CREATE INDEX membership_person_idx ON membership (person_id);

-- Placering i avdelning eller patrull, temporal.
CREATE TABLE placement (
    id        uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    body_id   uuid NOT NULL REFERENCES body(id),
    is_leader boolean NOT NULL DEFAULT false,
    valid     daterange NOT NULL
);
CREATE INDEX placement_person_idx ON placement (person_id);
CREATE INDEX placement_body_idx ON placement (body_id);

-- Rolltilldelning. Rollkatalogen (behörigheter, räckvidd) finns i koden.
CREATE TABLE role_assignment (
    id        uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    body_id   uuid NOT NULL REFERENCES body(id),
    role_key  text NOT NULL,
    valid     daterange NOT NULL
);
CREATE INDEX role_assignment_person_idx ON role_assignment (person_id);
CREATE INDEX role_assignment_body_idx ON role_assignment (body_id);

-- Vårdnadshavare/anhöriga. account_person_id kopplar till en person som kan logga in.
CREATE TABLE guardian (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),
    child_id          uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    name              text NOT NULL,
    email             text,
    phone             text,
    relation          text NOT NULL DEFAULT 'vårdnadshavare',
    account_person_id uuid REFERENCES person(id)
);
CREATE INDEX guardian_child_idx ON guardian (child_id);
CREATE INDEX guardian_account_idx ON guardian (account_person_id);

CREATE TABLE meeting (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    troop_id     uuid NOT NULL REFERENCES body(id),
    starts_at    timestamptz NOT NULL,
    duration_min integer NOT NULL CHECK (duration_min > 0),
    name         text NOT NULL,
    location     text,
    kind         text NOT NULL DEFAULT 'meeting' CHECK (kind IN ('meeting','hike','camp','activity')),
    locked       boolean NOT NULL DEFAULT false
);
CREATE INDEX meeting_troop_time_idx ON meeting (troop_id, starts_at);

CREATE TABLE attendance (
    meeting_id  uuid NOT NULL REFERENCES meeting(id) ON DELETE CASCADE,
    person_id   uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    status      text NOT NULL CHECK (status IN ('present','absent','excused')),
    is_leader   boolean NOT NULL DEFAULT false,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (meeting_id, person_id)
);
CREATE INDEX attendance_person_idx ON attendance (person_id);

-- Konto = koppling mellan inloggningsidentitet (ScoutID sub) och person. Inga lösenord.
CREATE TABLE account (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id     uuid NOT NULL UNIQUE REFERENCES person(id) ON DELETE CASCADE,
    idp_subject   text UNIQUE,
    dev_persona   text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz
);

CREATE TABLE session (
    token_hash bytea PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    auth_level text NOT NULL DEFAULT 'dev',
    auth_time  timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE INDEX session_expires_idx ON session (expires_at);

-- Append-only revisionslogg, inklusive läsning av känsliga uppgifter.
CREATE TABLE audit_event (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    at           timestamptz NOT NULL DEFAULT now(),
    account_id   uuid REFERENCES account(id) ON DELETE SET NULL,
    action       text NOT NULL,
    subject_type text NOT NULL,
    subject_id   uuid,
    detail       jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX audit_subject_idx ON audit_event (subject_type, subject_id, at DESC);

-- Aktuell bild per medlem: primärt medlemskap, avdelning och patrull.
-- effective_path = djupaste placeringen och används för behörighet och enhetsfilter.
CREATE VIEW member_current AS
SELECT
    p.*,
    m.id            AS membership_id,
    m.status        AS membership_status,
    lower(m.valid)  AS member_since,
    m.waiting_since,
    m.wished_troop_type,
    g.id            AS group_id,
    g.name          AS group_name,
    g.path          AS group_path,
    t.id            AS troop_id,
    t.name          AS troop_name,
    t.troop_type    AS troop_type,
    t.path          AS troop_path,
    coalesce(t.is_leader, false) AS is_leader,
    pa.id           AS patrol_id,
    pa.name         AS patrol_name,
    coalesce(pa.path, t.path, g.path) AS effective_path
FROM person p
JOIN membership m ON m.person_id = p.id AND m.is_primary AND upper_inf(m.valid)
JOIN body g ON g.id = m.group_id
LEFT JOIN LATERAL (
    SELECT b.id, b.name, b.troop_type, b.path, pl.is_leader
    FROM placement pl JOIN body b ON b.id = pl.body_id
    WHERE pl.person_id = p.id AND b.kind = 'troop' AND upper_inf(pl.valid)
    ORDER BY lower(pl.valid) DESC LIMIT 1
) t ON true
LEFT JOIN LATERAL (
    SELECT b.id, b.name, b.path
    FROM placement pl JOIN body b ON b.id = pl.body_id
    WHERE pl.person_id = p.id AND b.kind = 'patrol' AND upper_inf(pl.valid)
    ORDER BY lower(pl.valid) DESC LIMIT 1
) pa ON true;
