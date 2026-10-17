-- Alla personfält som dagens Scoutnet har (API, v5-profil och admin).
-- Mappning Scoutnet -> Scoutnet 2 finns i docs/04-faltkatalog.md.

DROP VIEW member_current;

-- Hushåll ("Länka familjemedlemmar", familjerabatt).
CREATE TABLE household (
    id   uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL
);

ALTER TABLE person RENAME COLUMN phone TO mobile_phone;
ALTER TABLE person
    ADD COLUMN alt_email                   text,
    ADD COLUMN scouterna_email             text,
    ADD COLUMN home_phone                  text,
    ADD COLUMN mailing_to_alt_email        boolean NOT NULL DEFAULT false,
    ADD COLUMN preferred_language          text NOT NULL DEFAULT 'sv' CHECK (preferred_language IN ('sv','en')),
    -- Medlemsstatus på personnivå (Scoutnet: Aktiv, Ny – inte klar för fakturering, Avregistrerad, Automatiskt
    -- avregistrerad, Avliden, Aktiv icke-medlem, Anonymiserad, Inaktiv – låst).
    ADD COLUMN person_status               text NOT NULL DEFAULT 'active'
        CHECK (person_status IN ('active','new','unregistered','auto_unregistered','deceased','active_non_member','anonymized','locked')),
    ADD COLUMN presentation                text,
    -- Intern anteckning (Scoutnet: Noteringar). Får aldrig innehålla hälsouppgifter.
    ADD COLUMN note                        text,
    ADD COLUMN photo_url                   text,
    ADD COLUMN newsletter                  boolean NOT NULL DEFAULT false,
    ADD COLUMN magazine                    boolean NOT NULL DEFAULT true,
    ADD COLUMN spar_sync                   boolean NOT NULL DEFAULT false,
    ADD COLUMN force_police_check          boolean NOT NULL DEFAULT false,
    ADD COLUMN invoice_delivery_membership text NOT NULL DEFAULT 'email' CHECK (invoice_delivery_membership IN ('email','alt_email','post','efaktura','kivra')),
    ADD COLUMN invoice_delivery_events     text NOT NULL DEFAULT 'email' CHECK (invoice_delivery_events IN ('email','alt_email','post','efaktura','kivra')),
    ADD COLUMN household_id                uuid REFERENCES household(id),
    ADD COLUMN confirmed_at                timestamptz;

-- Flera adresser per person med typ och giltighet (Scoutnet: Adressuppgifter, c/o, rad 1–3).
CREATE TABLE person_address (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id    uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    address_type text NOT NULL DEFAULT 'home' CHECK (address_type IN ('home','postal','invoice','temporary','other')),
    is_primary   boolean NOT NULL DEFAULT true,
    care_of      text,
    line1        text NOT NULL,
    line2        text,
    line3        text,
    postcode     text,
    city         text,
    country_code text NOT NULL DEFAULT 'SE',
    valid_from   date,
    valid_to     date
);
CREATE INDEX person_address_person_idx ON person_address (person_id);

INSERT INTO person_address (person_id, line1, postcode, city)
SELECT id, street, postcode, city FROM person WHERE street IS NOT NULL;
ALTER TABLE person DROP COLUMN street, DROP COLUMN postcode, DROP COLUMN city;

-- Anhörig 1 och 2 (Scoutnet: contact_mothers_name, contact_email_mum, contact_mobile_mum, contact_telephone_mum …).
ALTER TABLE guardian RENAME COLUMN phone TO mobile_phone;
ALTER TABLE guardian
    ADD COLUMN home_phone text,
    ADD COLUMN position   smallint NOT NULL DEFAULT 1 CHECK (position BETWEEN 1 AND 9);

-- Medlemskapets avgiftsgrupp och godkännandedatum (Scoutnet: feegroup, confirmed_at).
ALTER TABLE membership
    ADD COLUMN fee_group    text,
    ADD COLUMN confirmed_at date;

-- Terminsavgifter (Scoutnet: prev_term, current_term, current_term_due_date, kid, Betalstatus).
CREATE TABLE term_fee (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id  uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    group_id   uuid NOT NULL REFERENCES body(id),
    term_id    integer NOT NULL,
    amount_ore integer NOT NULL CHECK (amount_ore >= 0),
    fee_group  text,
    due_date   date NOT NULL,
    kid        text,
    paid_on    date,
    UNIQUE (person_id, group_id, term_id)
);

-- Registerutdrag: endast att det visats, aldrig innehållet.
CREATE TABLE police_check (
    id                    uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id             uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    issued_on             date NOT NULL,
    shown_on              date NOT NULL,
    verified_by_person_id uuid REFERENCES person(id),
    body_id               uuid REFERENCES body(id),
    created_at            timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX police_check_person_idx ON police_check (person_id, shown_on DESC);

-- Uppföljningssamtal (Scoutnet: registrerad av, datum, kopplad roll, organisationsled).
CREATE TABLE follow_up_talk (
    id                      uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id               uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    held_on                 date NOT NULL,
    role_key                text,
    body_id                 uuid REFERENCES body(id),
    registered_by_person_id uuid REFERENCES person(id)
);

-- Utmärkelser, märken, kurser och certifieringar (Scoutnet: Utmärkelser, Märken, Loggbok/CV).
CREATE TABLE person_merit (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id    uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    kind         text NOT NULL CHECK (kind IN ('award','badge','course','certification')),
    name         text NOT NULL,
    note         text,
    issued_by    text,
    completed_on date NOT NULL,
    place        text,
    organizer    text
);
CREATE INDEX person_merit_person_idx ON person_merit (person_id, kind);

-- Samtycken (Scoutnet: Publicering av personlig information, Publicera roller, nyhetsbrev, SPAR).
CREATE TABLE consent (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id  uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    kind       text NOT NULL CHECK (kind IN ('photo_publication','publish_profile','publish_roles')),
    granted    boolean NOT NULL,
    decided_at timestamptz NOT NULL DEFAULT now(),
    decided_by text NOT NULL DEFAULT 'self' CHECK (decided_by IN ('self','guardian','registrar')),
    UNIQUE (person_id, kind)
);

-- Kårspecifika anpassade fält (Scoutnet: custom_data / Anpassade fält).
CREATE TABLE custom_field (
    id      uuid PRIMARY KEY DEFAULT uuidv7(),
    body_id uuid NOT NULL REFERENCES body(id),
    key     text NOT NULL,
    label   text NOT NULL,
    UNIQUE (body_id, key)
);
CREATE TABLE custom_value (
    custom_field_id uuid NOT NULL REFERENCES custom_field(id) ON DELETE CASCADE,
    person_id       uuid NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    value           text NOT NULL,
    PRIMARY KEY (custom_field_id, person_id)
);

-- Aktuell bild per medlem (uppdaterad med nya fält, registerutdrag och terminsavgift).
CREATE VIEW member_current AS
SELECT
    p.*,
    m.id            AS membership_id,
    m.status        AS membership_status,
    lower(m.valid)  AS member_since,
    m.confirmed_at  AS membership_confirmed_at,
    m.waiting_since,
    m.wished_troop_type,
    m.fee_group,
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

-- Registerutdragsstatus: krävs för ledare, rollinnehavare och vid "tvinga registerutdrag".
-- Ett uppvisat utdrag gäller i tre år (Scouternas regel: visas minst vart tredje år).
CREATE VIEW police_check_status AS
SELECT
    p.id AS person_id,
    (p.force_police_check
        OR EXISTS (SELECT 1 FROM placement pl WHERE pl.person_id = p.id AND pl.is_leader AND upper_inf(pl.valid))
        OR EXISTS (SELECT 1 FROM role_assignment ra WHERE ra.person_id = p.id AND ra.valid @> current_date)) AS required,
    last.shown_on,
    last.issued_on,
    (last.shown_on + interval '3 years')::date AS valid_until
FROM person p
LEFT JOIN LATERAL (
    SELECT pc.shown_on, pc.issued_on FROM police_check pc WHERE pc.person_id = p.id ORDER BY pc.shown_on DESC LIMIT 1
) last ON true;
