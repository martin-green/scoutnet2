-- Moduler (docs/06-moduler.md): vilka moduler som är på eller av per enhet.
--
-- Ett val gäller enheten och allt under den, tills en underliggande enhet gör ett eget val.
-- `locked` på ett förbund eller distrikt gör att underliggande enheter inte kan ändra valet.
-- Saknas val används modulens standardläge. Modulernas id och beroenden finns i koden (manifest).

CREATE TABLE module_setting (
    body_id    uuid NOT NULL REFERENCES body (id) ON DELETE CASCADE,
    module_id  text NOT NULL CHECK (module_id ~ '^[a-z][a-z_]{1,39}$'),
    enabled    boolean NOT NULL,
    locked     boolean NOT NULL DEFAULT false,
    -- Modulens egna inställningar för enheten, validerade av modulen (JSON Schema i manifestet).
    settings   jsonb NOT NULL DEFAULT '{}',
    updated_by uuid REFERENCES account (id) ON DELETE SET NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (body_id, module_id)
);
CREATE INDEX module_setting_module_idx ON module_setting (module_id);
