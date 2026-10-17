-- Prestanda (vecka 6): läsmodell för aktuell avdelning och patrull per person.
--
-- member_current räknade tidigare fram avdelning och patrull med två LATERAL-uppslag per person.
-- Det gör att filter på en enhet (effective_path <@ ...) inte kan använda index, och att listor
-- över ett helt distrikt eller förbund blir långsamma (cirka 1 s vid 100 000 personer).
-- current_placement hålls aktuell av triggers på placement, så vyn blir vanliga joinar.

CREATE TABLE current_placement (
    person_id uuid PRIMARY KEY REFERENCES person (id) ON DELETE CASCADE,
    troop_id  uuid REFERENCES body (id),
    patrol_id uuid REFERENCES body (id),
    is_leader boolean NOT NULL DEFAULT false
);
CREATE INDEX current_placement_troop_idx ON current_placement (troop_id);
CREATE INDEX current_placement_patrol_idx ON current_placement (patrol_id);

-- Samma regel som vyn hade: senast påbörjade öppna placering i en avdelning respektive patrull.
CREATE FUNCTION refresh_current_placement(persons uuid[]) RETURNS void LANGUAGE sql AS $$
    DELETE FROM current_placement WHERE person_id = ANY (persons);
    INSERT INTO current_placement (person_id, troop_id, patrol_id, is_leader)
    SELECT p.id, t.id, pa.id, coalesce(t.is_leader, false)
    FROM person p
    LEFT JOIN LATERAL (
        SELECT b.id, pl.is_leader FROM placement pl JOIN body b ON b.id = pl.body_id
        WHERE pl.person_id = p.id AND b.kind = 'troop' AND upper_inf(pl.valid)
        ORDER BY lower(pl.valid) DESC LIMIT 1
    ) t ON true
    LEFT JOIN LATERAL (
        SELECT b.id FROM placement pl JOIN body b ON b.id = pl.body_id
        WHERE pl.person_id = p.id AND b.kind = 'patrol' AND upper_inf(pl.valid)
        ORDER BY lower(pl.valid) DESC LIMIT 1
    ) pa ON true
    WHERE p.id = ANY (persons) AND (t.id IS NOT NULL OR pa.id IS NOT NULL);
$$;

CREATE FUNCTION placement_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM refresh_current_placement(ARRAY(SELECT DISTINCT person_id FROM new_rows));
    ELSIF TG_OP = 'DELETE' THEN
        PERFORM refresh_current_placement(ARRAY(SELECT DISTINCT person_id FROM old_rows));
    ELSE
        PERFORM refresh_current_placement(ARRAY(SELECT person_id FROM new_rows UNION SELECT person_id FROM old_rows));
    END IF;
    RETURN NULL;
END
$$;

CREATE TRIGGER placement_current_ins AFTER INSERT ON placement
    REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION placement_changed();
CREATE TRIGGER placement_current_upd AFTER UPDATE ON placement
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION placement_changed();
CREATE TRIGGER placement_current_del AFTER DELETE ON placement
    REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION placement_changed();

SELECT refresh_current_placement(ARRAY(SELECT DISTINCT person_id FROM placement));

CREATE OR REPLACE VIEW member_current AS
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
    coalesce(cp.is_leader, false) AS is_leader,
    pa.id           AS patrol_id,
    pa.name         AS patrol_name,
    coalesce(pa.path, t.path, g.path) AS effective_path
FROM person p
JOIN membership m ON m.person_id = p.id AND m.is_primary AND upper_inf(m.valid)
JOIN body g ON g.id = m.group_id
LEFT JOIN current_placement cp ON cp.person_id = p.id
LEFT JOIN body t ON t.id = cp.troop_id
LEFT JOIN body pa ON pa.id = cp.patrol_id;

-- Medlemmar hör till en kår, och effective_path räknas fram per person. För att ett filter på
-- en enhet ska kunna använda index (body.path) begränsas först kåren: enheter på kårnivå eller
-- högre filtrerar på sin egen sökväg, enheter under en kår på kåren. Används som
-- `mc.group_path <@ group_scope($1) AND mc.effective_path <@ $1`.
CREATE FUNCTION group_scope(p ltree) RETURNS ltree LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT coalesce(
        (SELECT b.path FROM body b WHERE b.kind = 'group' AND b.path @> p ORDER BY nlevel(b.path) DESC LIMIT 1),
        p)
$$;
