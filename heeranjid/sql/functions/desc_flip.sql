-- heeranjid v0.3.0 — flip primitives for the descending-sort variants.
-- Copy the function bodies verbatim from spec §5.1 lines 310–361:
--   heerid_flip_mask (returns 9223372036850589695::bigint = 0x7FFFFFFFFFC01FFF)
--   heerid_to_desc / heerid_to_asc (IMMUTABLE PARALLEL SAFE SQL, XOR with mask)
--   ranjid_to_desc (PL/pgSQL, byte-wise XOR against decode('FFFFFFFFFFFF0FFF0FFFFFFF8000FFFF','hex'))
--   ranjid_to_asc (SQL wrapper delegating to ranjid_to_desc)

CREATE OR REPLACE FUNCTION heerid_flip_mask() RETURNS bigint AS $$
    SELECT 9223372036850589695::bigint;  -- 0x7FFFFFFFFFC01FFF
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION heerid_to_desc(bits bigint) RETURNS bigint AS $$
    SELECT (bits # heerid_flip_mask())::bigint;
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION heerid_to_asc(bits bigint) RETURNS bigint AS $$
    SELECT (bits # heerid_flip_mask())::bigint;
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;

DO $install$
DECLARE
    _sch text := COALESCE(current_schema(), 'public');
BEGIN
    EXECUTE format($sql$
CREATE OR REPLACE FUNCTION ranjid_to_desc(id uuid) RETURNS uuid AS $func$
DECLARE
    b    bytea := uuid_send(id);
    mask bytea := decode('FFFFFFFFFFFF0FFF0FFFFFFF8000FFFF', 'hex');
    r    bytea := '';
    i    int;
BEGIN
    FOR i IN 0..15 LOOP
        r := r || set_byte('\x00'::bytea, 0, get_byte(b, i) # get_byte(mask, i));
    END LOOP;
    RETURN encode(r, 'hex')::uuid;
END;
$func$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE SET search_path = %I, pg_catalog;
    $sql$, _sch);
END;
$install$;

CREATE OR REPLACE FUNCTION ranjid_to_asc(id uuid) RETURNS uuid AS $$
    SELECT ranjid_to_desc(id);
$$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;
