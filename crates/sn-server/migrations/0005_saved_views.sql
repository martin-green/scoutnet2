-- Sparade vyer: en namngiven fråga (filter, sortering, kolumner) för en lista i en enhet.
-- Delade vyer syns för alla som får se listan i enheten, t.ex. "Ledare utan giltigt registerutdrag".
CREATE TABLE saved_view (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    owner_account_id uuid NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    body_id          uuid NOT NULL REFERENCES body(id) ON DELETE CASCADE,
    list             text NOT NULL CHECK (list IN ('members', 'waiting', 'organisation')),
    name             text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    query            text NOT NULL CHECK (char_length(query) <= 2000),
    shared           boolean NOT NULL DEFAULT false,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX saved_view_body_idx ON saved_view (body_id, list);
CREATE UNIQUE INDEX saved_view_owner_name_key ON saved_view (owner_account_id, body_id, list, lower(name));
