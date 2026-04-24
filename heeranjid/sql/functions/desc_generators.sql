-- heeranjid v0.3.0 — single-row generator wrappers plus desc generators.
--
-- `generate_ids(node_id, 1)` is the existing batch API from v0.2.x. These
-- single-row wrappers ease `DEFAULT` expressions and match the naming the
-- library's docs use.

CREATE OR REPLACE FUNCTION heerid_next() RETURNS bigint AS $$
    SELECT id FROM generate_ids(current_heer_node_id(), 1);
$$ LANGUAGE sql VOLATILE;

CREATE OR REPLACE FUNCTION ranjid_next() RETURNS uuid AS $$
    SELECT id FROM generate_ranjids(current_heer_ranj_node_id(), 1);
$$ LANGUAGE sql VOLATILE;

CREATE OR REPLACE FUNCTION heerid_next_desc() RETURNS bigint AS $$
    SELECT heerid_to_desc(heerid_next());
$$ LANGUAGE sql VOLATILE;

CREATE OR REPLACE FUNCTION ranjid_next_desc() RETURNS uuid AS $$
    SELECT ranjid_to_desc(ranjid_next());
$$ LANGUAGE sql VOLATILE;

-- v0.3.4 — bulk descending generators.
--
-- Batch counterparts to `heerid_next_desc()` / `ranjid_next_desc()`. Each
-- overload composes the matching `generate_ids` / `generate_ranjids`
-- overload with the desc flip, so callers get descending-shape IDs in a
-- single round-trip without reaching for the flip primitives directly.
-- Mirrors the 3-variant shape of `generate_ids` and `generate_ranjids`.

CREATE OR REPLACE FUNCTION generate_ids_desc(
    in_node_id INTEGER,
    requested_count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
)
RETURNS TABLE(id BIGINT)
LANGUAGE sql
AS $$
    SELECT heerid_to_desc(id)
    FROM generate_ids(in_node_id, requested_count, allow_spanning);
$$;

CREATE OR REPLACE FUNCTION generate_ids_desc(
    requested_count INTEGER,
    allow_spanning BOOLEAN
)
RETURNS TABLE(id BIGINT)
LANGUAGE sql
AS $$
    SELECT heerid_to_desc(id)
    FROM generate_ids(current_heer_node_id(), requested_count, allow_spanning);
$$;

CREATE OR REPLACE FUNCTION generate_ids_desc(requested_count INTEGER)
RETURNS TABLE(id BIGINT)
LANGUAGE sql
AS $$
    SELECT heerid_to_desc(id)
    FROM generate_ids(current_heer_node_id(), requested_count, true);
$$;

CREATE OR REPLACE FUNCTION generate_ranjids_desc(
    in_node_id INTEGER,
    requested_count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
)
RETURNS TABLE(id UUID)
LANGUAGE sql
AS $$
    SELECT ranjid_to_desc(id)
    FROM generate_ranjids(in_node_id, requested_count, allow_spanning);
$$;

CREATE OR REPLACE FUNCTION generate_ranjids_desc(
    requested_count INTEGER,
    allow_spanning BOOLEAN
)
RETURNS TABLE(id UUID)
LANGUAGE sql
AS $$
    SELECT ranjid_to_desc(id)
    FROM generate_ranjids(current_heer_ranj_node_id(), requested_count, allow_spanning);
$$;

CREATE OR REPLACE FUNCTION generate_ranjids_desc(requested_count INTEGER)
RETURNS TABLE(id UUID)
LANGUAGE sql
AS $$
    SELECT ranjid_to_desc(id)
    FROM generate_ranjids(current_heer_ranj_node_id(), requested_count, true);
$$;
