-- API-nycklar för integrationer (t.ex. Skojjt, Google-synk) och det Scoutnet-kompatibla API:t.
-- Bara hashen sparas. Nyckeln visas en gång när den skapas. Behörighet styrs per nyckel (scopes).
CREATE TABLE api_key (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    body_id      uuid NOT NULL REFERENCES body(id) ON DELETE CASCADE,
    name         text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    key_hash     bytea NOT NULL UNIQUE,
    -- memberlist, memberlist:contact, memberlist:ssno
    scopes       text[] NOT NULL,
    created_by   uuid REFERENCES account(id) ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX api_key_body_idx ON api_key (body_id);

-- Revisionsloggen: händelser som rör många personer (massåtgärder, exporter) söks per person.
CREATE INDEX audit_persons_idx ON audit_event USING gin ((detail -> 'persons'));
