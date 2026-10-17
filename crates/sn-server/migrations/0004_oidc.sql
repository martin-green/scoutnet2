-- Inloggning via ScoutID (OIDC). Pågående inloggningar: state (hashat), nonce och PKCE-verifierare
-- sparas på servern i högst tio minuter. Webbläsaren binds till inloggningen med en kortlivad cookie.
CREATE TABLE oidc_login (
    state_hash    bytea PRIMARY KEY,
    nonce         text NOT NULL,
    pkce_verifier text NOT NULL,
    return_to     text NOT NULL DEFAULT '/',
    created_at    timestamptz NOT NULL DEFAULT now()
);

-- ID-token används som id_token_hint vid utloggning från ScoutID och raderas med sessionen.
ALTER TABLE session ADD COLUMN id_token text;
