-- Varje enhet har ett nummer som är unikt per enhetstyp. Numret används i webbadresser (/kar/1001,
-- /avdelning/5003) och som nyckel i API:t (group-1001). Vid migrering bevaras Scoutnets id för kårer,
-- avdelningar och patruller, alltså samma nummer som Skojjt använder. Nya enheter får nästa nummer ur
-- sekvensen, som börjar över Scoutnets högsta id.
CREATE SEQUENCE body_number_seq START 900000;
UPDATE body SET number = nextval('body_number_seq') WHERE number IS NULL;
ALTER TABLE body
    ALTER COLUMN number SET DEFAULT nextval('body_number_seq'),
    ALTER COLUMN number SET NOT NULL;
ALTER SEQUENCE body_number_seq OWNED BY body.number;
CREATE UNIQUE INDEX body_kind_number_key ON body (kind, number);
