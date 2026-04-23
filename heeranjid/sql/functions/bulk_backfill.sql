CREATE OR REPLACE PROCEDURE heeranjid_bulk_backfill(
    table_name text,
    src_col    text,
    dst_col    text,
    kind       text,                       -- 'heer' or 'ranj'
    batch_size int DEFAULT 10000
) LANGUAGE plpgsql AS $$
DECLARE
    flip_fn   text;
    rows_done int;
BEGIN
    flip_fn := CASE kind
        WHEN 'heer' THEN 'heerid_to_desc'
        WHEN 'ranj' THEN 'ranjid_to_desc'
        ELSE NULL
    END;
    IF flip_fn IS NULL THEN
        RAISE EXCEPTION 'heeranjid_bulk_backfill: unknown kind %', kind;
    END IF;

    LOOP
        -- SET LOCAL must be reissued after each COMMIT because its scope is
        -- the current transaction (PG docs: plpgsql-transactions). Issuing
        -- it at the top of the loop body means every batch transaction runs
        -- with the 30s lock_timeout.
        SET LOCAL lock_timeout = '30s';
        EXECUTE format(
            'WITH batch AS (
                 SELECT ctid FROM %I
                 WHERE %I IS NULL AND %I IS NOT NULL
                 LIMIT %s
                 FOR UPDATE SKIP LOCKED
             )
             UPDATE %I t SET %I = %I(t.%I)
             FROM batch
             WHERE t.ctid = batch.ctid',
            table_name, dst_col, src_col, batch_size,
            table_name, dst_col, flip_fn, src_col
        );
        GET DIAGNOSTICS rows_done = ROW_COUNT;
        COMMIT;
        RAISE NOTICE 'backfill: % rows this batch (skip-locked)', rows_done;
        EXIT WHEN rows_done = 0;
    END LOOP;

    -- Cleanup pass: catch rows that were locked by long-running transactions
    -- every time the loop saw them (SKIP LOCKED hides them indefinitely). This
    -- pass does NOT use SKIP LOCKED and can block indefinitely on a live
    -- locker unless lock_timeout aborts the statement first; by this point the
    -- residue is usually small. Still filters src IS NOT NULL so nullable FKs
    -- are not touched.
    LOOP
        -- Same rationale as above: SET LOCAL is transaction-scoped and must
        -- be reissued per batch.
        SET LOCAL lock_timeout = '30s';
        EXECUTE format(
            'WITH batch AS (
                 SELECT ctid FROM %I
                 WHERE %I IS NULL AND %I IS NOT NULL
                 LIMIT %s
                 FOR UPDATE
             )
             UPDATE %I t SET %I = %I(t.%I)
             FROM batch
             WHERE t.ctid = batch.ctid',
            table_name, dst_col, src_col, batch_size,
            table_name, dst_col, flip_fn, src_col
        );
        GET DIAGNOSTICS rows_done = ROW_COUNT;
        COMMIT;
        RAISE NOTICE 'backfill: % rows this batch (cleanup)', rows_done;
        EXIT WHEN rows_done = 0;
    END LOOP;
END;
$$;
