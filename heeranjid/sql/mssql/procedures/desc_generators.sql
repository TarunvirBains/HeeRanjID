-- heeranjid v0.3.1 — MSSQL desc generators.
--
-- Thin wrappers around the existing v0.2.x generate_id /
-- generate_ranjid procs. Those return a single-row result set rather
-- than expose an OUTPUT parameter, so we capture via
-- `INSERT ... EXEC` into a table variable and then flip with the
-- appropriate desc function.
--
-- Returning a single-row result set matches the calling convention of
-- generate_id / generate_ranjid, so the Django binding can dispatch on
-- connection.vendor and use `cursor.execute("EXEC heerid_next_desc
-- @in_node_id=%s"); row = cursor.fetchone()`.

CREATE OR ALTER PROCEDURE dbo.heerid_next_desc
    @in_node_id int = NULL
AS
BEGIN
    SET NOCOUNT ON;

    DECLARE @asc TABLE (id bigint);
    INSERT INTO @asc (id)
    EXEC dbo.generate_id @in_node_id = @in_node_id;

    SELECT dbo.heerid_to_desc(id) AS id FROM @asc;
END;
GO

CREATE OR ALTER PROCEDURE dbo.ranjid_next_desc
    @in_node_id int = NULL
AS
BEGIN
    SET NOCOUNT ON;

    DECLARE @asc TABLE (id BINARY(16));
    INSERT INTO @asc (id)
    EXEC dbo.generate_ranjid @in_node_id = @in_node_id;

    SELECT dbo.ranjid_to_desc(id) AS id FROM @asc;
END;
GO
