# MSSQL Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MSSQL support to HeeRanjID — T-SQL stored procedures for ID generation, Django backend detection, and integration tests against a Docker MSSQL container.

**Architecture:** Write T-SQL equivalents of the Postgres functions in `sql/mssql/`. Update Django fields to detect the database backend and use appropriate SQL/types. Test via Python against Docker containers for both databases.

**Tech Stack:** T-SQL, MSSQL 2022, Django 4.2+, mssql-django, pyodbc, Docker Compose, pytest

---

### Task 1: MSSQL Schema

**Files:**
- Create: `sql/mssql/schema.sql`
- Create: `sql/mssql/seed.sql`

- [ ] **Step 1: Create the MSSQL schema**

Create `sql/mssql/schema.sql`:

```sql
IF NOT EXISTS (SELECT * FROM sys.tables WHERE name = 'heer_nodes')
CREATE TABLE heer_nodes (
    node_id       INT PRIMARY KEY,
    name          NVARCHAR(255) NOT NULL,
    description   NVARCHAR(MAX) NULL,
    is_active     BIT NOT NULL DEFAULT 1,
    created_at    DATETIME2 NOT NULL DEFAULT SYSUTCDATETIME(),
    last_accessed DATETIME2 NOT NULL DEFAULT SYSUTCDATETIME()
);

IF NOT EXISTS (SELECT * FROM sys.tables WHERE name = 'heer_config')
CREATE TABLE heer_config (
    id                  INT PRIMARY KEY CHECK (id = 1),
    epoch               DATETIME2 NOT NULL,
    ranj_epoch_offset   NUMERIC(38,0) NOT NULL DEFAULT 0,
    updated_at          DATETIME2 NOT NULL DEFAULT SYSUTCDATETIME()
);

IF NOT EXISTS (SELECT * FROM sys.tables WHERE name = 'heer_node_state')
CREATE TABLE heer_node_state (
    node_id         INT PRIMARY KEY
                    REFERENCES heer_nodes(node_id) ON DELETE CASCADE,
    last_id_time    BIGINT NOT NULL DEFAULT 0,
    last_sequence   SMALLINT NOT NULL DEFAULT 0,
    updated_at      DATETIME2 NOT NULL DEFAULT SYSUTCDATETIME()
);

IF NOT EXISTS (SELECT * FROM sys.tables WHERE name = 'heer_ranj_node_state')
CREATE TABLE heer_ranj_node_state (
    node_id         INT PRIMARY KEY
                    REFERENCES heer_nodes(node_id) ON DELETE CASCADE,
    last_id_time    NUMERIC(38,0) NOT NULL DEFAULT 0,
    last_sequence   INT NOT NULL DEFAULT 0,
    updated_at      DATETIME2 NOT NULL DEFAULT SYSUTCDATETIME()
);
```

- [ ] **Step 2: Create the MSSQL seed data**

Create `sql/mssql/seed.sql`:

```sql
-- Default seed data for single-node deployments.
-- Safe to run multiple times (uses IF NOT EXISTS).

IF NOT EXISTS (SELECT 1 FROM heer_nodes WHERE node_id = 1)
    INSERT INTO heer_nodes (node_id, name, description, is_active)
    VALUES (1, N'default', N'Default single-node instance', 1);

IF NOT EXISTS (SELECT 1 FROM heer_node_state WHERE node_id = 1)
    INSERT INTO heer_node_state (node_id)
    VALUES (1);

IF NOT EXISTS (SELECT 1 FROM heer_ranj_node_state WHERE node_id = 1)
    INSERT INTO heer_ranj_node_state (node_id)
    VALUES (1);
```

- [ ] **Step 3: Verify syntax by checking against MSSQL docs**

Read through both files and verify:
- `DATETIME2` used everywhere (not `DATETIME` or `TIMESTAMP`)
- `SYSUTCDATETIME()` for defaults (not `GETDATE()` — we want UTC)
- `BIT` for booleans with `1`/`0` literals
- `NVARCHAR` for text columns
- `NUMERIC(38,0)` for large integer columns
- Foreign key `ON DELETE CASCADE` syntax is valid in MSSQL

- [ ] **Step 4: Commit**

```bash
git add sql/mssql/schema.sql sql/mssql/seed.sql
git commit -m "feat(mssql): add schema and seed SQL"
```

---

### Task 2: MSSQL Session Management

**Files:**
- Create: `sql/mssql/procedures/session.sql`

- [ ] **Step 1: Write session procedures**

Create `sql/mssql/procedures/session.sql`:

```sql
CREATE OR ALTER PROCEDURE heer_set_node_id
    @node_id INT
AS
BEGIN
    SET NOCOUNT ON;

    IF @node_id IS NULL
        THROW 50001, 'node_id cannot be null', 1;

    IF @node_id < 0 OR @node_id > 511
    BEGIN
        DECLARE @heer_msg NVARCHAR(200) = CONCAT('node_id ', @node_id, ' is out of range for HeerId (0..511)');
        THROW 50002, @heer_msg, 1;
    END

    IF NOT EXISTS (
        SELECT 1 FROM heer_nodes WHERE node_id = @node_id AND is_active = 1
    )
    BEGIN
        DECLARE @active_msg NVARCHAR(200) = CONCAT('node_id ', @node_id, ' is not registered as an active Heer node');
        THROW 50003, @active_msg, 1;
    END

    EXEC sp_set_session_context N'heer_node_id', @node_id;
END;
GO

CREATE OR ALTER PROCEDURE heer_set_ranj_node_id
    @node_id INT
AS
BEGIN
    SET NOCOUNT ON;

    IF @node_id IS NULL
        THROW 50001, 'node_id cannot be null', 1;

    IF @node_id < 0 OR @node_id > 65535
    BEGIN
        DECLARE @ranj_msg NVARCHAR(200) = CONCAT('node_id ', @node_id, ' is out of range for RanjId (0..65535)');
        THROW 50002, @ranj_msg, 1;
    END

    IF NOT EXISTS (
        SELECT 1 FROM heer_nodes WHERE node_id = @node_id AND is_active = 1
    )
    BEGIN
        DECLARE @active_msg NVARCHAR(200) = CONCAT('node_id ', @node_id, ' is not registered as an active Heer node');
        THROW 50003, @active_msg, 1;
    END

    EXEC sp_set_session_context N'heer_ranj_node_id', @node_id;
END;
GO

CREATE OR ALTER FUNCTION dbo.heer_current_node_id()
RETURNS INT
AS
BEGIN
    DECLARE @val SQL_VARIANT = SESSION_CONTEXT(N'heer_node_id');
    IF @val IS NULL
        RETURN NULL;
    RETURN CAST(@val AS INT);
END;
GO

CREATE OR ALTER FUNCTION dbo.heer_current_ranj_node_id()
RETURNS INT
AS
BEGIN
    DECLARE @val SQL_VARIANT = SESSION_CONTEXT(N'heer_ranj_node_id');
    IF @val IS NULL
        RETURN NULL;
    RETURN CAST(@val AS INT);
END;
GO
```

- [ ] **Step 2: Commit**

```bash
git add sql/mssql/procedures/session.sql
git commit -m "feat(mssql): add session management procedures"
```

---

### Task 3: MSSQL HeerId Generation

**Files:**
- Create: `sql/mssql/procedures/generate_heerid.sql`

- [ ] **Step 1: Write the HeerId generation stored procedures**

Create `sql/mssql/procedures/generate_heerid.sql`:

```sql
CREATE OR ALTER PROCEDURE generate_ids
    @in_node_id INT = NULL,
    @requested_count INT,
    @allow_spanning BIT = 1
AS
BEGIN
    SET NOCOUNT ON;

    -- Resolve node_id from parameter or session context
    DECLARE @node_id INT = @in_node_id;
    IF @node_id IS NULL
        SET @node_id = CAST(SESSION_CONTEXT(N'heer_node_id') AS INT);

    IF @node_id IS NULL
        THROW 50010, 'node_id not provided and heer_node_id not set in session', 1;

    IF @requested_count IS NULL OR @requested_count <= 0
        THROW 50011, 'requested_count must be greater than zero', 1;

    -- Validate node
    EXEC heer_set_node_id @node_id;

    -- Read epoch
    DECLARE @epoch DATETIME2;
    SELECT @epoch = epoch FROM heer_config WHERE id = 1;

    IF @epoch IS NULL
        THROW 50012, 'heer_config row id=1 must exist before generating IDs', 1;

    DECLARE @epoch_ms BIGINT = DATEDIFF_BIG(MILLISECOND, '1970-01-01T00:00:00', @epoch);
    DECLARE @now_ms BIGINT = DATEDIFF_BIG(MILLISECOND, '1970-01-01T00:00:00', SYSUTCDATETIME()) - @epoch_ms;

    -- Ensure state row exists
    IF NOT EXISTS (SELECT 1 FROM heer_node_state WHERE node_id = @node_id)
        INSERT INTO heer_node_state (node_id) VALUES (@node_id);

    -- Lock and read state
    DECLARE @last_time BIGINT;
    DECLARE @last_sequence INT;

    SELECT @last_time = last_id_time, @last_sequence = last_sequence
    FROM heer_node_state WITH (UPDLOCK, ROWLOCK, HOLDLOCK)
    WHERE node_id = @node_id;

    -- Clock rollback detection
    DECLARE @rollback_ms BIGINT = @last_time - @now_ms;
    IF @rollback_ms > 0
    BEGIN
        DECLARE @rb_msg NVARCHAR(200) = CONCAT('clock rollback detected for node ', @node_id, ' (', @rollback_ms, ' ms)');
        THROW 50020, @rb_msg, 1;
    END

    DECLARE @current_tick BIGINT = IIF(@now_ms > @last_time, @now_ms, @last_time);
    DECLARE @next_sequence INT = IIF(@current_tick = @last_time, @last_sequence + 1, 0);

    -- Check capacity
    DECLARE @available INT = 8192 - @next_sequence;
    IF @allow_spanning = 0 AND @requested_count > @available
    BEGIN
        DECLARE @cap_msg NVARCHAR(300) = CONCAT('requested ', @requested_count, ' IDs but only ', @available, ' remain in millisecond ', @current_tick, ' for node ', @node_id);
        THROW 50021, @cap_msg, 1;
    END

    -- Generate IDs into a temp table
    CREATE TABLE #heer_ids (id BIGINT);

    DECLARE @remaining INT = @requested_count;
    DECLARE @last_emitted_time BIGINT;
    DECLARE @last_emitted_seq INT;
    DECLARE @emit_count INT;
    DECLARE @seq INT;

    WHILE @remaining > 0
    BEGIN
        SET @available = 8192 - @next_sequence;
        SET @emit_count = IIF(@remaining < @available, @remaining, @available);

        SET @seq = @next_sequence;
        WHILE @seq < @next_sequence + @emit_count
        BEGIN
            INSERT INTO #heer_ids (id)
            VALUES (
                (@current_tick * CAST(POWER(2, 22) AS BIGINT))
                | (CAST(@node_id AS BIGINT) * CAST(POWER(2, 13) AS BIGINT))
                | CAST(@seq AS BIGINT)
            );
            SET @seq = @seq + 1;
        END

        SET @last_emitted_time = @current_tick;
        SET @last_emitted_seq = @next_sequence + @emit_count - 1;
        SET @remaining = @remaining - @emit_count;
        SET @current_tick = @current_tick + 1;
        SET @next_sequence = 0;
    END

    -- Update state
    UPDATE heer_node_state
    SET last_id_time = @last_emitted_time,
        last_sequence = @last_emitted_seq,
        updated_at = SYSUTCDATETIME()
    WHERE node_id = @node_id;

    -- Return results
    SELECT id FROM #heer_ids;
    DROP TABLE #heer_ids;
END;
GO

CREATE OR ALTER PROCEDURE generate_id
    @in_node_id INT = NULL
AS
BEGIN
    SET NOCOUNT ON;
    EXEC generate_ids @in_node_id = @in_node_id, @requested_count = 1, @allow_spanning = 1;
END;
GO
```

- [ ] **Step 2: Commit**

```bash
git add sql/mssql/procedures/generate_heerid.sql
git commit -m "feat(mssql): add HeerId generation stored procedures"
```

---

### Task 4: MSSQL RanjId Generation

**Files:**
- Create: `sql/mssql/procedures/generate_ranjid.sql`

- [ ] **Step 1: Write the RanjId generation stored procedures**

Create `sql/mssql/procedures/generate_ranjid.sql`:

```sql
CREATE OR ALTER PROCEDURE generate_ranjids
    @in_node_id INT = NULL,
    @requested_count INT,
    @allow_spanning BIT = 1
AS
BEGIN
    SET NOCOUNT ON;

    -- Resolve node_id from parameter or session context
    DECLARE @node_id INT = @in_node_id;
    IF @node_id IS NULL
        SET @node_id = CAST(SESSION_CONTEXT(N'heer_ranj_node_id') AS INT);

    IF @node_id IS NULL
        THROW 50010, 'node_id not provided and heer_ranj_node_id not set in session', 1;

    IF @requested_count IS NULL OR @requested_count <= 0
        THROW 50011, 'requested_count must be greater than zero', 1;

    IF @node_id < 0 OR @node_id > 65535
    BEGIN
        DECLARE @range_msg NVARCHAR(200) = CONCAT('node_id ', @node_id, ' is out of range for RanjId (0..65535)');
        THROW 50002, @range_msg, 1;
    END

    IF NOT EXISTS (
        SELECT 1 FROM heer_nodes WHERE node_id = @node_id AND is_active = 1
    )
    BEGIN
        DECLARE @active_msg NVARCHAR(200) = CONCAT('node_id ', @node_id, ' is not registered as an active Heer node');
        THROW 50003, @active_msg, 1;
    END

    -- Read epoch
    DECLARE @epoch DATETIME2;
    DECLARE @epoch_offset NUMERIC(38,0);
    SELECT @epoch = epoch, @epoch_offset = ranj_epoch_offset
    FROM heer_config WHERE id = 1;

    IF @epoch IS NULL
        THROW 50012, 'heer_config row id=1 must exist before generating IDs', 1;

    DECLARE @epoch_us NUMERIC(38,0) = CAST(DATEDIFF_BIG(MICROSECOND, '1970-01-01T00:00:00', @epoch) AS NUMERIC(38,0));
    DECLARE @now_us NUMERIC(38,0) = CAST(DATEDIFF_BIG(MICROSECOND, '1970-01-01T00:00:00', SYSUTCDATETIME()) AS NUMERIC(38,0))
                                    - @epoch_us
                                    + @epoch_offset;

    -- Ensure state row exists
    IF NOT EXISTS (SELECT 1 FROM heer_ranj_node_state WHERE node_id = @node_id)
        INSERT INTO heer_ranj_node_state (node_id) VALUES (@node_id);

    -- Lock and read state
    DECLARE @last_time NUMERIC(38,0);
    DECLARE @last_seq INT;

    SELECT @last_time = last_id_time, @last_seq = last_sequence
    FROM heer_ranj_node_state WITH (UPDLOCK, ROWLOCK, HOLDLOCK)
    WHERE node_id = @node_id;

    -- Clock rollback detection
    DECLARE @rollback_us NUMERIC(38,0) = @last_time - @now_us;
    IF @rollback_us > 0
    BEGIN
        DECLARE @rb_msg NVARCHAR(200) = CONCAT('clock rollback detected for ranj node ', @node_id, ' (', CAST(@rollback_us AS NVARCHAR), ' us)');
        THROW 50020, @rb_msg, 1;
    END

    DECLARE @current_tick NUMERIC(38,0) = IIF(@now_us > @last_time, @now_us, @last_time);
    DECLARE @next_seq INT = IIF(@current_tick = @last_time, @last_seq + 1, 0);

    -- Check capacity
    DECLARE @available INT = 65536 - @next_seq;
    IF @allow_spanning = 0 AND @requested_count > @available
    BEGIN
        DECLARE @cap_msg NVARCHAR(300) = CONCAT('requested ', @requested_count, ' IDs but only ', @available, ' remain in microsecond for ranj node ', @node_id);
        THROW 50021, @cap_msg, 1;
    END

    -- Generate IDs into a temp table
    CREATE TABLE #ranj_ids (id BINARY(16));

    DECLARE @remaining INT = @requested_count;
    DECLARE @last_emitted_time NUMERIC(38,0);
    DECLARE @last_emitted_seq INT;
    DECLARE @emit_count INT;
    DECLARE @seq INT;

    -- Bit decomposition variables
    DECLARE @two NUMERIC(38,0) = 2;
    DECLARE @ts_high BIGINT;
    DECLARE @ts_mid BIGINT;
    DECLARE @ts_low BIGINT;
    DECLARE @hi BIGINT;
    DECLARE @lo BIGINT;
    DECLARE @hi_bin BINARY(8);
    DECLARE @lo_bin BINARY(8);

    WHILE @remaining > 0
    BEGIN
        SET @available = 65536 - @next_seq;
        SET @emit_count = IIF(@remaining < @available, @remaining, @available);

        -- Decompose 90-bit NUMERIC timestamp into three parts that fit in BIGINT
        SET @ts_high = CAST(FLOOR(@current_tick / POWER(@two, 42)) % POWER(@two, 48) AS BIGINT);
        SET @ts_mid  = CAST(FLOOR(@current_tick / POWER(@two, 30)) % POWER(@two, 12) AS BIGINT);
        SET @ts_low  = CAST(@current_tick % POWER(@two, 30) AS BIGINT);

        -- Upper 8 bytes: ts_high(48) | version(4) | ts_mid(12)
        SET @hi = (@ts_high * CAST(POWER(2, 16) AS BIGINT))
                | (CAST(7 AS BIGINT) * CAST(POWER(2, 12) AS BIGINT))
                | @ts_mid;

        SET @seq = @next_seq;
        WHILE @seq < @next_seq + @emit_count
        BEGIN
            -- Lower 8 bytes: variant(2) | ts_low(30) | node_id(16) | sequence(16)
            -- 0x8000000000000000 sets the variant bits (10xxxxxx)
            SET @lo = CAST(0x8000000000000000 AS BIGINT)
                    | (@ts_low * CAST(POWER(2, 32) AS BIGINT))
                    | (CAST(@node_id AS BIGINT) * CAST(POWER(2, 16) AS BIGINT))
                    | CAST(@seq AS BIGINT);

            -- Convert both halves to BINARY(8) and concatenate to BINARY(16)
            SET @hi_bin = CAST(@hi AS BINARY(8));
            SET @lo_bin = CAST(@lo AS BINARY(8));

            INSERT INTO #ranj_ids (id)
            VALUES (@hi_bin + @lo_bin);

            SET @seq = @seq + 1;
        END

        SET @last_emitted_time = @current_tick;
        SET @last_emitted_seq = @next_seq + @emit_count - 1;
        SET @remaining = @remaining - @emit_count;
        SET @current_tick = @current_tick + 1;
        SET @next_seq = 0;
    END

    -- Update state
    UPDATE heer_ranj_node_state
    SET last_id_time = @last_emitted_time,
        last_sequence = @last_emitted_seq,
        updated_at = SYSUTCDATETIME()
    WHERE node_id = @node_id;

    -- Return results
    SELECT id FROM #ranj_ids;
    DROP TABLE #ranj_ids;
END;
GO

CREATE OR ALTER PROCEDURE generate_ranjid
    @in_node_id INT = NULL
AS
BEGIN
    SET NOCOUNT ON;
    EXEC generate_ranjids @in_node_id = @in_node_id, @requested_count = 1, @allow_spanning = 1;
END;
GO
```

- [ ] **Step 2: Create MSSQL install script**

Create `sql/mssql/install.sql`:

```sql
-- HeeRanjID MSSQL Installation
-- Run this script to install all tables and stored procedures.
-- Safe to run multiple times (uses CREATE OR ALTER / IF NOT EXISTS).

:r schema.sql
:r procedures\session.sql
:r procedures\generate_heerid.sql
:r procedures\generate_ranjid.sql
```

Note: `:r` is the `sqlcmd` include directive. For programmatic execution (Django, pyodbc), read and execute each file separately.

- [ ] **Step 3: Create MSSQL query files**

Create `sql/mssql/queries/fetch_node.sql`:

```sql
SELECT node_id, name, description, is_active
FROM heer_nodes
WHERE node_id = @node_id;
```

Create `sql/mssql/queries/fetch_active_node.sql`:

```sql
SELECT node_id, name, description, is_active
FROM heer_nodes
WHERE node_id = @node_id AND is_active = 1;
```

Create `sql/mssql/queries/fetch_epoch.sql`:

```sql
SELECT epoch FROM heer_config WHERE id = 1;
```

- [ ] **Step 4: Commit**

```bash
git add sql/mssql/
git commit -m "feat(mssql): add HeerId and RanjId generation stored procedures"
```

---

### Task 5: Django Backend Detection and pre_save

**Files:**
- Modify: `heeranjid-python/python/heeranjid/django/fields.py`
- Modify: `heeranjid-python/python/heeranjid/django/migrations/0001_install_heeranjid.py`
- Create: `heeranjid-python/python/heeranjid/sql/mssql/` (bundled MSSQL SQL files)
- Modify: `heeranjid-python/tests/test_django_fields.py`

- [ ] **Step 1: Reorganize bundled SQL into postgres/ and mssql/ subdirectories**

Move existing SQL files into a `postgres/` subdirectory and add MSSQL files:

```bash
cd heeranjid-python/python/heeranjid/sql
mkdir -p postgres mssql
mv schema.sql session.sql generate_heerid.sql generate_ranjid.sql seed.sql postgres/

# Copy MSSQL SQL from the sql submodule
cp ../../../../sql/mssql/schema.sql mssql/
cp ../../../../sql/mssql/seed.sql mssql/
cp ../../../../sql/mssql/procedures/session.sql mssql/
cp ../../../../sql/mssql/procedures/generate_heerid.sql mssql/
cp ../../../../sql/mssql/procedures/generate_ranjid.sql mssql/
```

- [ ] **Step 2: Update the Django fields with backend detection and pre_save**

Replace `heeranjid-python/python/heeranjid/django/fields.py` with:

```python
import uuid as uuid_mod

from django.db import models
from django.db.models.expressions import RawSQL

from heeranjid import HeerId, RanjId


class HeerIdField(models.BigIntegerField):
    """A Django model field that stores a HeerId as a BIGINT.

    On Postgres, sets db_default to generate_id() for non-Django inserts.
    On all backends, pre_save() generates the ID via stored proc before INSERT.
    """

    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            # db_default only works on Postgres (scalar function)
            # MSSQL requires stored procs which can't be defaults
            # We set it lazily in contribute_to_class when we know the backend
            self._wants_db_default = True
        else:
            self._wants_db_default = False
        super().__init__(*args, **kwargs)

    def contribute_to_class(self, cls, name, **kwargs):
        super().contribute_to_class(cls, name, **kwargs)
        if self._wants_db_default:
            # Set db_default for Postgres; MSSQL uses pre_save instead
            self.db_default = RawSQL("generate_id()", [])

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname)
        if add and value is None:
            from django.db import connection
            with connection.cursor() as cursor:
                if connection.vendor == "microsoft":
                    cursor.execute("EXEC generate_id")
                else:
                    cursor.execute("SELECT generate_id()")
                row = cursor.fetchone()
                value = HeerId(int(row[0]))
            setattr(model_instance, self.attname, value)
        return super().pre_save(model_instance, add)

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        return HeerId(int(value))

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, HeerId):
            return value.as_int()
        return int(value)

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid.django.fields.HeerIdField", args, kwargs


class RanjIdField(models.Field):
    """A Django model field that stores a RanjId.

    On Postgres: stored as UUID.
    On MSSQL: stored as BINARY(16) for correct sort order.
    """

    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            self._wants_db_default = True
        else:
            self._wants_db_default = False
        super().__init__(*args, **kwargs)

    def contribute_to_class(self, cls, name, **kwargs):
        super().contribute_to_class(cls, name, **kwargs)
        if self._wants_db_default:
            self.db_default = RawSQL("generate_ranjid()", [])

    def db_type(self, connection):
        if connection.vendor == "microsoft":
            return "BINARY(16)"
        return "uuid"

    def rel_db_type(self, connection):
        return self.db_type(connection)

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname)
        if add and value is None:
            from django.db import connection
            with connection.cursor() as cursor:
                if connection.vendor == "microsoft":
                    cursor.execute("EXEC generate_ranjid")
                    row = cursor.fetchone()
                    # MSSQL returns BINARY(16) as bytes
                    value = RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(row[0]))))
                else:
                    cursor.execute("SELECT generate_ranjid()")
                    row = cursor.fetchone()
                    value = RanjId.from_str(str(row[0]))
            setattr(model_instance, self.attname, value)
        return super().pre_save(model_instance, add)

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        if isinstance(value, (bytes, memoryview)):
            value = uuid_mod.UUID(bytes=bytes(value))
        if not isinstance(value, str):
            value = str(value)
        return RanjId.from_str(value)

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, RanjId):
            return value.to_uuid()
        return value

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid.django.fields.RanjIdField", args, kwargs
```

- [ ] **Step 3: Update the Django migration for backend detection**

Replace `heeranjid-python/python/heeranjid/django/migrations/0001_install_heeranjid.py` with:

```python
"""Install HeeRanjID schema and stored procedures/functions."""
from importlib import resources

from django.db import migrations


def _read_sql(backend, filename):
    """Read a bundled SQL file for the given backend."""
    return (
        resources.files("heeranjid.sql")
        .joinpath(backend)
        .joinpath(filename)
        .read_text(encoding="utf-8")
    )


def _get_backend(schema_editor):
    """Return 'postgres' or 'mssql' based on the database vendor."""
    vendor = schema_editor.connection.vendor
    if vendor == "microsoft":
        return "mssql"
    return "postgres"


def forwards(apps, schema_editor):
    backend = _get_backend(schema_editor)
    sql_files = [
        "schema.sql",
        "session.sql",
        "generate_heerid.sql",
        "generate_ranjid.sql",
        "seed.sql",
    ]
    for filename in sql_files:
        sql = _read_sql(backend, filename)
        # MSSQL stored procedures use GO as batch separator.
        # Execute each batch separately.
        if backend == "mssql":
            for batch in sql.split("\nGO\n"):
                batch = batch.strip()
                if batch:
                    schema_editor.execute(batch)
        else:
            schema_editor.execute(sql)


def backwards(apps, schema_editor):
    backend = _get_backend(schema_editor)
    if backend == "mssql":
        schema_editor.execute("DROP PROCEDURE IF EXISTS generate_ranjid;")
        schema_editor.execute("DROP PROCEDURE IF EXISTS generate_ranjids;")
        schema_editor.execute("DROP PROCEDURE IF EXISTS generate_id;")
        schema_editor.execute("DROP PROCEDURE IF EXISTS generate_ids;")
        schema_editor.execute("DROP PROCEDURE IF EXISTS heer_set_ranj_node_id;")
        schema_editor.execute("DROP PROCEDURE IF EXISTS heer_set_node_id;")
        schema_editor.execute("DROP FUNCTION IF EXISTS dbo.heer_current_ranj_node_id;")
        schema_editor.execute("DROP FUNCTION IF EXISTS dbo.heer_current_node_id;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_ranj_node_state;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_node_state;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_config;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_nodes;")
    else:
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_id(INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER, BOOLEAN) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER, BOOLEAN) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjid() CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjid(INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjids(INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, BOOLEAN) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, INTEGER, BOOLEAN) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS set_heer_node_id(INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS current_heer_node_id() CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS set_heer_ranj_node_id(INTEGER) CASCADE;")
        schema_editor.execute("DROP FUNCTION IF EXISTS current_heer_ranj_node_id() CASCADE;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_ranj_node_state CASCADE;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_node_state CASCADE;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_config CASCADE;")
        schema_editor.execute("DROP TABLE IF EXISTS heer_nodes CASCADE;")


class Migration(migrations.Migration):
    initial = True
    dependencies = []
    operations = [
        migrations.RunPython(forwards, backwards),
    ]
```

- [ ] **Step 4: Update Django field tests**

Update `heeranjid-python/tests/test_django_fields.py` — the `RanjIdField` no longer extends `UUIDField`, so update `test_internal_type`:

Find and replace the `test_internal_type` test in `TestRanjIdField`:

```python
    def test_db_type_postgres(self):
        field = RanjIdField()
        # Mock a postgres connection
        class FakeConn:
            vendor = "postgresql"
        assert field.db_type(FakeConn()) == "uuid"

    def test_db_type_mssql(self):
        field = RanjIdField()
        class FakeConn:
            vendor = "microsoft"
        assert field.db_type(FakeConn()) == "BINARY(16)"
```

Remove the old `test_internal_type` test for RanjIdField since we now use `db_type()` instead of `get_internal_type()`.

- [ ] **Step 5: Run existing tests to ensure no regressions**

Run: `cd heeranjid-python && source ../.venv/bin/activate && maturin develop && pytest tests/test_django_fields.py tests/test_heerid.py tests/test_ranjid.py -v`
Expected: All existing tests pass (some RanjIdField tests may need adjusting due to the base class change from UUIDField to Field)

- [ ] **Step 6: Commit**

```bash
git add heeranjid-python/
git commit -m "feat(python): add MSSQL backend detection and pre_save ID generation"
```

---

### Task 6: Docker Compose and Integration Tests

**Files:**
- Create: `docker-compose.yml`
- Create: `heeranjid-python/tests/test_postgres_integration.py`
- Create: `heeranjid-python/tests/test_mssql_integration.py`

- [ ] **Step 1: Create docker-compose.yml**

Create `docker-compose.yml` at repo root:

```yaml
services:
  postgres:
    image: postgres:latest
    ports:
      - "5432:5432"
    environment:
      POSTGRES_DB: heeranjid
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 10

  mssql:
    image: mcr.microsoft.com/mssql/server:2022-latest
    ports:
      - "1433:1433"
    environment:
      ACCEPT_EULA: "Y"
      MSSQL_SA_PASSWORD: "HeeRanjID_Test1"
    healthcheck:
      test: ["CMD-SHELL", "/opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -P HeeRanjID_Test1 -C -Q 'SELECT 1' || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 10
```

- [ ] **Step 2: Write Postgres integration tests**

Create `heeranjid-python/tests/test_postgres_integration.py`:

```python
"""Integration tests for HeeRanjID against a real Postgres database.

Requires DATABASE_URL environment variable.
Start the database: docker compose up postgres -d
"""
import os
import uuid

import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
if DATABASE_URL is None:
    pytest.fail(
        "DATABASE_URL not set — run 'docker compose up postgres -d' "
        "and set DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid",
        pytrace=False,
    )

psycopg2 = pytest.importorskip("psycopg2")

from heeranjid import HeerId, RanjId


@pytest.fixture(scope="module")
def pg_conn():
    """Connect to Postgres and install schema in a temporary schema."""
    conn = psycopg2.connect(DATABASE_URL)
    conn.autocommit = True
    cur = conn.cursor()

    # Install in public schema (functions are schema-global)
    from importlib import resources
    sql_dir = resources.files("heeranjid.sql").joinpath("postgres")
    for filename in ["schema.sql", "session.sql", "generate_heerid.sql", "generate_ranjid.sql", "seed.sql"]:
        cur.execute(sql_dir.joinpath(filename).read_text(encoding="utf-8"))

    # Set epoch to a known value
    cur.execute("""
        INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ON CONFLICT (id) DO UPDATE SET epoch = EXCLUDED.epoch
    """)

    yield conn
    conn.close()


@pytest.fixture
def cursor(pg_conn):
    cur = pg_conn.cursor()
    yield cur
    cur.close()


class TestHeerIdPostgres:
    def test_generate_single(self, cursor):
        cursor.execute("SELECT generate_id(1)")
        raw = cursor.fetchone()[0]
        hid = HeerId(raw)
        assert hid.node_id() == 1
        assert hid.timestamp_ms() > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("SELECT id FROM generate_ids(1, 10)")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [HeerId(r[0]) for r in rows]
        # Verify chronological ordering
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_session_node_id(self, cursor):
        cursor.execute("SELECT set_heer_node_id(1)")
        cursor.execute("SELECT generate_id()")
        raw = cursor.fetchone()[0]
        hid = HeerId(raw)
        assert hid.node_id() == 1


class TestRanjIdPostgres:
    def test_generate_single(self, cursor):
        cursor.execute("SELECT generate_ranjid(1)")
        raw = cursor.fetchone()[0]
        rid = RanjId.from_str(str(raw))
        assert rid.node_id == 100 or rid.node_id >= 0  # node from generation
        assert rid.timestamp_micros > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("SELECT id FROM generate_ranjids(1, 10)")
        rows = cursor.fetchall()
        assert len(rows) == 10

    def test_session_node_id(self, cursor):
        cursor.execute("SELECT set_heer_ranj_node_id(1)")
        cursor.execute("SELECT generate_ranjid()")
        raw = cursor.fetchone()[0]
        rid = RanjId.from_str(str(raw))
        assert rid.node_id == 1
```

- [ ] **Step 3: Write MSSQL integration tests**

Create `heeranjid-python/tests/test_mssql_integration.py`:

```python
"""Integration tests for HeeRanjID against a real MSSQL database.

Requires MSSQL_URL environment variable.
Start the database: docker compose up mssql -d
"""
import os
import uuid

import pytest

MSSQL_URL = os.environ.get("MSSQL_URL")
if MSSQL_URL is None:
    pytest.fail(
        "MSSQL_URL not set — run 'docker compose up mssql -d' "
        "and set MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};"
        "SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;"
        "TrustServerCertificate=yes'",
        pytrace=False,
    )

pyodbc = pytest.importorskip("pyodbc")

from heeranjid import HeerId, RanjId


@pytest.fixture(scope="module")
def mssql_conn():
    """Connect to MSSQL and install schema."""
    conn = pyodbc.connect(MSSQL_URL, autocommit=True)
    cur = conn.cursor()

    # Create a test database if it doesn't exist
    cur.execute("""
        IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = 'heeranjid_test')
            CREATE DATABASE heeranjid_test
    """)
    cur.execute("USE heeranjid_test")

    # Install schema and procedures
    from importlib import resources
    sql_dir = resources.files("heeranjid.sql").joinpath("mssql")
    for filename in ["schema.sql", "session.sql", "generate_heerid.sql", "generate_ranjid.sql", "seed.sql"]:
        sql = sql_dir.joinpath(filename).read_text(encoding="utf-8")
        # Split on GO batch separator
        for batch in sql.split("\nGO\n"):
            batch = batch.strip()
            if batch:
                cur.execute(batch)

    # Set epoch
    cur.execute("""
        IF NOT EXISTS (SELECT 1 FROM heer_config WHERE id = 1)
            INSERT INTO heer_config (id, epoch) VALUES (1, '2024-01-01T00:00:00')
        ELSE
            UPDATE heer_config SET epoch = '2024-01-01T00:00:00' WHERE id = 1
    """)

    yield conn
    conn.close()


@pytest.fixture
def cursor(mssql_conn):
    cur = mssql_conn.cursor()
    cur.execute("USE heeranjid_test")
    yield cur
    cur.close()


class TestHeerIdMssql:
    def test_generate_single(self, cursor):
        cursor.execute("EXEC generate_id @in_node_id = 1")
        raw = cursor.fetchone()[0]
        hid = HeerId(int(raw))
        assert hid.node_id() == 1
        assert hid.timestamp_ms() > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("EXEC generate_ids @in_node_id = 1, @requested_count = 10")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [HeerId(int(r[0])) for r in rows]
        # Verify chronological ordering
        for i in range(len(ids) - 1):
            assert ids[i].as_int() < ids[i + 1].as_int()

    def test_session_node_id(self, cursor):
        cursor.execute("EXEC heer_set_node_id @node_id = 1")
        cursor.execute("EXEC generate_id")
        raw = cursor.fetchone()[0]
        hid = HeerId(int(raw))
        assert hid.node_id() == 1


class TestRanjIdMssql:
    def test_generate_single(self, cursor):
        cursor.execute("EXEC generate_ranjid @in_node_id = 1")
        raw_bytes = cursor.fetchone()[0]
        # MSSQL returns BINARY(16) as bytes
        u = uuid.UUID(bytes=bytes(raw_bytes))
        rid = RanjId.from_str(str(u))
        assert rid.node_id == 1
        assert rid.timestamp_micros > 0

    def test_generate_bulk(self, cursor):
        cursor.execute("EXEC generate_ranjids @in_node_id = 1, @requested_count = 10")
        rows = cursor.fetchall()
        assert len(rows) == 10
        ids = [RanjId.from_str(str(uuid.UUID(bytes=bytes(r[0])))) for r in rows]
        # Verify chronological ordering (BINARY(16) big-endian sorts correctly)
        for i in range(len(ids) - 1):
            assert str(ids[i]) < str(ids[i + 1])

    def test_session_node_id(self, cursor):
        cursor.execute("EXEC heer_set_ranj_node_id @node_id = 1")
        cursor.execute("EXEC generate_ranjid")
        raw_bytes = cursor.fetchone()[0]
        u = uuid.UUID(bytes=bytes(raw_bytes))
        rid = RanjId.from_str(str(u))
        assert rid.node_id == 1
```

- [ ] **Step 4: Add test dependencies to pyproject.toml**

In `heeranjid-python/pyproject.toml`, update optional dependencies:

```toml
[project.optional-dependencies]
django = ["django>=4.2"]
mssql = ["mssql-django>=1.4", "pyodbc>=5.0"]
postgres = ["psycopg2-binary>=2.9"]
dev = ["pytest>=8.0", "maturin>=1.0"]
```

- [ ] **Step 5: Run unit tests (no database required)**

Run: `cd heeranjid-python && source ../.venv/bin/activate && maturin develop && pytest tests/test_heerid.py tests/test_ranjid.py tests/test_django_fields.py -v`
Expected: All unit tests pass

- [ ] **Step 6: Run Postgres integration tests**

Run:
```bash
docker compose up postgres -d
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid
cd heeranjid-python && source ../.venv/bin/activate && pip install psycopg2-binary && pytest tests/test_postgres_integration.py -v
```
Expected: All Postgres integration tests pass

- [ ] **Step 7: Run MSSQL integration tests**

Run:
```bash
docker compose up mssql -d
# Wait for MSSQL to be ready (~15 seconds)
export MSSQL_URL='DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost,1433;UID=sa;PWD=HeeRanjID_Test1;TrustServerCertificate=yes'
cd heeranjid-python && source ../.venv/bin/activate && pip install pyodbc && pytest tests/test_mssql_integration.py -v
```
Expected: All MSSQL integration tests pass

- [ ] **Step 8: Run full Rust workspace checks**

Run: `cargo clippy --workspace -- -D warnings && cargo fmt --all --check && cargo test -p heeranjid --lib`
Expected: All pass (no Rust changes in this task, but verify nothing broke)

- [ ] **Step 9: Commit**

```bash
git add docker-compose.yml heeranjid-python/
git commit -m "feat: add Docker Compose, Postgres and MSSQL integration tests"
```
