-- Kalenderprenumeration (iCal): en slumpmässig, återkallbar token per konto och enhet.
-- Bara hashen sparas. Flödet innehåller möten men inga personuppgifter.
CREATE TABLE calendar_token (
    token_hash   bytea PRIMARY KEY,
    account_id   uuid NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    body_id      uuid NOT NULL REFERENCES body(id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    UNIQUE (account_id, body_id)
);
