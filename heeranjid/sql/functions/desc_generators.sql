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
