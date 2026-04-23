-- heeranjid v0.3.1 — MSSQL bulk backfill procedure.
--
-- Mirrors the Postgres v0.3.0 heeranjid_bulk_backfill procedure. Two
-- loops: a fast path with UPDLOCK + READPAST (MSSQL's equivalent of
-- FOR UPDATE SKIP LOCKED) followed by a cleanup pass that uses plain
-- UPDLOCK to catch rows the fast path skipped.
--
-- Transaction model note: T-SQL has no PL/pgSQL-style COMMIT inside a
-- loop, so each batch opens its own BEGIN TRAN / COMMIT TRAN. SET
-- LOCK_TIMEOUT is session-scoped (inverts Postgres's SET LOCAL which
-- is per-transaction), so we set it once at proc entry.
--
-- @@TRANCOUNT pre-flight check: the proc must run at the top level —
-- committing inside a user-open transaction is illegal in MSSQL.

CREATE OR ALTER PROCEDURE dbo.heeranjid_bulk_backfill
    @table_name sysname,
    @src_col    sysname,
    @dst_col    sysname,
    @kind       varchar(8),
    @batch_size int = 10000
AS
BEGIN
    SET NOCOUNT ON;

    IF @@TRANCOUNT > 0
        THROW 50310, 'heeranjid_bulk_backfill must run at the top level (no open tx).', 1;

    DECLARE @flip_fn sysname =
        CASE @kind
            WHEN 'heer' THEN N'heerid_to_desc'
            WHEN 'ranj' THEN N'ranjid_to_desc'
            ELSE NULL
        END;
    IF @flip_fn IS NULL
        THROW 50311, 'heeranjid_bulk_backfill: unknown kind; expected heer or ranj', 1;

    -- Session-scoped (inverts v0.3.0 Postgres's per-tx SET LOCAL).
    -- 30 s matches the Postgres playbook's lock_timeout.
    SET LOCK_TIMEOUT 30000;

    DECLARE @rows_done int = 1;
    DECLARE @sql nvarchar(max);

    -- Fast loop: UPDLOCK + READPAST is MSSQL's FOR UPDATE SKIP LOCKED.
    -- Rows locked by concurrent writers are skipped; we will revisit
    -- them in the cleanup loop below.
    WHILE @rows_done > 0
    BEGIN
        BEGIN TRAN;
        SET @sql = N'
            UPDATE TOP (@bs) t
            SET t.' + QUOTENAME(@dst_col) + N' = dbo.' + QUOTENAME(@flip_fn) + N'(t.' + QUOTENAME(@src_col) + N')
            FROM ' + QUOTENAME(@table_name) + N' AS t WITH (UPDLOCK, READPAST)
            WHERE t.' + QUOTENAME(@dst_col) + N' IS NULL
              AND t.' + QUOTENAME(@src_col) + N' IS NOT NULL;';
        EXEC sp_executesql @sql, N'@bs int', @bs = @batch_size;
        SET @rows_done = @@ROWCOUNT;
        COMMIT TRAN;
    END

    -- Cleanup loop: plain UPDLOCK (no READPAST). Blocks on locked rows
    -- but LOCK_TIMEOUT aborts the statement after 30 s so we don't hang
    -- forever behind a stuck writer. Residue is typically small by the
    -- time we get here.
    SET @rows_done = 1;
    WHILE @rows_done > 0
    BEGIN
        BEGIN TRAN;
        SET @sql = N'
            UPDATE TOP (@bs) t
            SET t.' + QUOTENAME(@dst_col) + N' = dbo.' + QUOTENAME(@flip_fn) + N'(t.' + QUOTENAME(@src_col) + N')
            FROM ' + QUOTENAME(@table_name) + N' AS t WITH (UPDLOCK)
            WHERE t.' + QUOTENAME(@dst_col) + N' IS NULL
              AND t.' + QUOTENAME(@src_col) + N' IS NOT NULL;';
        EXEC sp_executesql @sql, N'@bs int', @bs = @batch_size;
        SET @rows_done = @@ROWCOUNT;
        COMMIT TRAN;
    END
END;
GO
