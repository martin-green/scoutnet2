-- Bidragsunderlag: uppgifter som kommunerna kräver i DAK (Digitalt Aktivitetskort) och
-- kommunala bidragsregler som versionerad data (en rad per kommun och år).
ALTER TABLE body
    ADD COLUMN municipality_code text CHECK (municipality_code ~ '^[0-9]{4}$'),
    ADD COLUMN association_id    text,
    ADD COLUMN org_number        text;

CREATE TABLE grant_ruleset (
    id                     uuid PRIMARY KEY DEFAULT uuidv7(),
    municipality_code      text NOT NULL CHECK (municipality_code ~ '^[0-9]{4}$'),
    year                   integer NOT NULL,
    name                   text NOT NULL,
    -- Ålder räknas som det år deltagaren fyller (sammankomstens år minus födelseår).
    min_age                smallint NOT NULL,
    max_age                smallint NOT NULL,
    min_leader_age         smallint NOT NULL,
    min_participants       smallint NOT NULL,
    min_leaders            smallint NOT NULL,
    min_minutes            smallint NOT NULL,
    amount_per_female_ore  integer NOT NULL,
    amount_per_male_ore    integer NOT NULL,
    source                 text,
    UNIQUE (municipality_code, year)
);

-- Göteborgs kommun 2026, samma värden som Skojjt (AktivitetsbidragKnownSettings).
INSERT INTO grant_ruleset (municipality_code, year, name, min_age, max_age, min_leader_age, min_participants, min_leaders,
                           min_minutes, amount_per_female_ore, amount_per_male_ore, source)
VALUES ('1480', 2026, 'Göteborgs kommun – aktivitetsbidrag 2026', 7, 25, 13, 3, 1, 60, 989, 802,
        'Skojjt-v2 AktivitetsbidragKnownSettings (Göteborg 2026)');
