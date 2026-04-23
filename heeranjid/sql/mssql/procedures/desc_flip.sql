-- heeranjid v0.3.1 — MSSQL descending-sort flip primitives.
--
-- Pure T-SQL, no CLR dependency. Mirrors the Postgres v0.3.0 flip
-- functions in heeranjid/sql/functions/desc_flip.sql.
--
-- HeerId (bigint): XOR against 0x7FFFFFFFFFC01FFF preserves the node_id
-- and (inverted) timestamp bits so that DESC ordering of the asc form
-- equals ASC ordering of the desc form.
--
-- RanjId (BINARY(16)): same principle, 128-bit mask 0xFFFFFFFFFFFF0FFF
-- 0FFFFFFF8000FFFF preserves UUIDv8 version/variant nibbles and the
-- node_id field. T-SQL has no native `^` on varbinary, so we XOR byte
-- by byte via SUBSTRING + CAST AS tinyint + STUFF.

CREATE OR ALTER FUNCTION dbo.heerid_flip_mask()
RETURNS bigint
WITH SCHEMABINDING
AS
BEGIN
    RETURN CAST(0x7FFFFFFFFFC01FFF AS bigint);
END;
GO

CREATE OR ALTER FUNCTION dbo.heerid_to_desc(@bits bigint)
RETURNS bigint
WITH SCHEMABINDING
AS
BEGIN
    RETURN @bits ^ CAST(0x7FFFFFFFFFC01FFF AS bigint);
END;
GO

CREATE OR ALTER FUNCTION dbo.heerid_to_asc(@bits bigint)
RETURNS bigint
WITH SCHEMABINDING
AS
BEGIN
    -- XOR is its own inverse.
    RETURN @bits ^ CAST(0x7FFFFFFFFFC01FFF AS bigint);
END;
GO

CREATE OR ALTER FUNCTION dbo.ranjid_to_desc(@id BINARY(16))
RETURNS BINARY(16)
WITH SCHEMABINDING
AS
BEGIN
    DECLARE @mask BINARY(16) = 0xFFFFFFFFFFFF0FFF0FFFFFFF8000FFFF;
    DECLARE @out  BINARY(16) = 0x00000000000000000000000000000000;
    DECLARE @i    int = 1;
    DECLARE @b    tinyint;
    WHILE @i <= 16
    BEGIN
        SET @b =
            CAST(SUBSTRING(@id,   @i, 1) AS tinyint) ^
            CAST(SUBSTRING(@mask, @i, 1) AS tinyint);
        SET @out = CAST(STUFF(@out, @i, 1, CAST(@b AS binary(1))) AS BINARY(16));
        SET @i = @i + 1;
    END
    RETURN @out;
END;
GO

CREATE OR ALTER FUNCTION dbo.ranjid_to_asc(@id BINARY(16))
RETURNS BINARY(16)
WITH SCHEMABINDING
AS
BEGIN
    -- XOR is its own inverse; delegate to ranjid_to_desc.
    RETURN dbo.ranjid_to_desc(@id);
END;
GO
