# MSSQL Support Design

## Overview

Add Microsoft SQL Server support to HeeRanjID by writing T-SQL stored procedures that mirror the existing Postgres functions, updating the Django binding to detect the database backend and adapt accordingly, and adding integration tests against a Docker MSSQL container.

## Scope

**In scope:**
- T-SQL stored procedures in `sql/mssql/` — full parity with Postgres (HeerId + RanjId generation, session management, schema, seed)
- Django field updates for backend detection (`BINARY(16)` on MSSQL, `UUID` on Postgres)
- Django `pre_save()` for ID generation on MSSQL (stored procs can't be column defaults)
- Python integration tests via pytest + `mssql-django` + `pyodbc`
- `docker-compose.yml` with Postgres + MSSQL containers for local dev
- Documentation for raw SQL users on MSSQL

**Out of scope:**
- Rust MSSQL driver (sqlx doesn't support MSSQL)
- Changes to core types (already database-agnostic)
- JS/Prisma MSSQL adaptation (follow-up work)
- .NET or C API changes (they don't touch the DB)
- Custom Python Docker image (TODO for later)

## SQL Submodule Structure

```
sql/
├── postgres/
│   ├── schema.sql
│   ├── seed.sql
│   ├── install.sql
│   ├── functions/
│   │   ├── session.sql
│   │   ├── generate_heerid.sql
│   │   └── generate_ranjid.sql
│   └── queries/
│       ├── fetch_node.sql
│       ├── fetch_active_node.sql
│       └── fetch_epoch.sql
├── mssql/
│   ├── schema.sql
│   ├── seed.sql
│   ├── install.sql
│   ├── procedures/
│   │   ├── session.sql
│   │   ├── generate_heerid.sql
│   │   └── generate_ranjid.sql
│   └── queries/
│       ├── fetch_node.sql
│       ├── fetch_active_node.sql
│       └── fetch_epoch.sql
```

The `mssql/` directory uses `procedures/` instead of `functions/` because T-SQL stored procedures are required for stateful operations (updating state tables).

## MSSQL Schema

Same four tables as Postgres with type adaptations:

| Postgres Type | MSSQL Type | Used For |
|---|---|---|
| `BOOLEAN` | `BIT` | `is_active` |
| `TEXT` | `NVARCHAR(255)` / `NVARCHAR(MAX)` | `name`, `description` |
| `TIMESTAMP` | `DATETIME2` | `created_at`, `epoch` |
| `UUID` | `BINARY(16)` | RanjId values |
| `NUMERIC(30,0)` | `NUMERIC(38,0)` | `ranj_epoch_offset`, `last_id_time` for RanjId |
| `BIGINT` | `BIGINT` | HeerId values, `last_id_time` for HeerId |
| `SMALLINT` | `SMALLINT` | `last_sequence` for HeerId |
| `INTEGER` | `INT` | `node_id`, `last_sequence` for RanjId |
| `ON CONFLICT DO NOTHING` | `IF NOT EXISTS` pattern | Idempotent seed inserts |

**RanjId is stored as `BINARY(16)` (not `UNIQUEIDENTIFIER`)** to preserve big-endian byte order for correct chronological sorting. MSSQL's `UNIQUEIDENTIFIER` uses mixed-endian storage which breaks time-based sort order.

## T-SQL Stored Procedures

### Session Management

Session-level node IDs stored via `SESSION_CONTEXT`:

- `heer_set_node_id @node_id INT` — validates node exists and is active, stores via `sp_set_session_context N'heer_node_id', @node_id`
- `heer_set_ranj_node_id @node_id INT` — same for RanjId (0-65535 range), stores via `sp_set_session_context N'heer_ranj_node_id', @node_id`
- `heer_current_node_id()` — scalar function, reads `SESSION_CONTEXT(N'heer_node_id')`
- `heer_current_ranj_node_id()` — scalar function, reads `SESSION_CONTEXT(N'heer_ranj_node_id')`

Session getters are scalar functions (read-only, no side effects). Session setters are stored procedures (validate + write).

### HeerId Generation

- `generate_id @node_id INT = NULL` — returns single-row result set with `BIGINT` id. If `@node_id` is NULL, reads from session context.
- `generate_ids @node_id INT = NULL, @count INT, @allow_spanning BIT = 1` — returns multi-row result set of `BIGINT` id values.

Algorithm (identical logic to Postgres):
1. Validate node_id (0-511 range)
2. Read epoch from `heer_config`, convert to milliseconds: `DATEDIFF_BIG(MILLISECOND, '1970-01-01', epoch)`
3. Get current time: `DATEDIFF_BIG(MILLISECOND, '1970-01-01', SYSUTCDATETIME())`
4. Compute elapsed: `current_ms - epoch_ms`
5. Lock state row: `SELECT ... FROM heer_node_state WITH (UPDLOCK, ROWLOCK, HOLDLOCK)`
6. Detect clock rollback (50ms threshold), `THROW 50001` if detected
7. Construct ID via bit shifting:
   - `(@elapsed_ms * POWER(CAST(2 AS BIGINT), 22)) | (@node_id * POWER(CAST(2 AS BIGINT), 13)) | @sequence`
8. Handle sequence overflow by incrementing timestamp (if `@allow_spanning = 1`)
9. Emit IDs via `WHILE` loop (replaces Postgres `generate_series()`)
10. Update state atomically

### RanjId Generation

- `generate_ranjid @node_id INT = NULL` — returns single-row result set with `BINARY(16)` id
- `generate_ranjids @node_id INT = NULL, @count INT, @allow_spanning BIT = 1` — returns multi-row result set of `BINARY(16)` id values

Algorithm (identical logic to Postgres):
1. Validate node_id (0-65535 range)
2. Read epoch + `ranj_epoch_offset` from `heer_config`
3. Compute elapsed microseconds as `NUMERIC(38,0)`: `DATEDIFF_BIG(MICROSECOND, '1970-01-01', SYSUTCDATETIME()) - epoch_us + epoch_offset`
4. Lock state row: `SELECT ... FROM heer_ranj_node_state WITH (UPDLOCK, ROWLOCK, HOLDLOCK)`
5. Detect clock rollback (50000μs threshold)
6. Decompose 90-bit timestamp using `NUMERIC(38,0)` arithmetic:
   - `@ts_high = CAST(FLOOR(@current_tick / POWER(CAST(2 AS NUMERIC(38,0)), 42)) % POWER(CAST(2 AS NUMERIC(38,0)), 48) AS BIGINT)`
   - `@ts_mid = CAST(FLOOR(@current_tick / POWER(CAST(2 AS NUMERIC(38,0)), 30)) % POWER(CAST(2 AS NUMERIC(38,0)), 12) AS BIGINT)`
   - `@ts_low = CAST(@current_tick % POWER(CAST(2 AS NUMERIC(38,0)), 30) AS BIGINT)`
7. Construct 128-bit value as two `BIGINT` halves:
   - `@hi = (@ts_high * POWER(CAST(2 AS BIGINT), 16)) | (CAST(7 AS BIGINT) * POWER(CAST(2 AS BIGINT), 12)) | @ts_mid`
   - `@lo = CAST(0x8000000000000000 AS BIGINT) | (@ts_low * POWER(CAST(2 AS BIGINT), 32)) | (@node_id * POWER(CAST(2 AS BIGINT), 16)) | @sequence`
8. Convert to `BINARY(16)` via hex string: `CONVERT(BINARY(16), CONVERT(VARCHAR(16), @hi, 2) + CONVERT(VARCHAR(16), @lo, 2), 2)`
9. Emit via `WHILE` loop, update state

### Locking Strategy

Postgres `FOR UPDATE` maps to `WITH (UPDLOCK, ROWLOCK, HOLDLOCK)`:
- `UPDLOCK` — acquire update lock (prevents other transactions from taking update/exclusive locks)
- `ROWLOCK` — lock at row level (prevents escalation to page/table lock)
- `HOLDLOCK` — hold lock until end of transaction (equivalent to SERIALIZABLE for this row)

### Clock Rollback Detection

Same 50ms threshold as Postgres. Both HeerId and RanjId:
- If `current_time < last_id_time`: clock went backwards
- If rollback ≤ 50ms (or 50000μs for RanjId): `THROW 50001, 'Clock rollback detected', 1`
- If rollback > 50ms: hard error (same behavior as Postgres)

## Django Integration Changes

### RanjIdField Backend Detection

`RanjIdField` currently extends `UUIDField`. It needs to become backend-aware:

```python
class RanjIdField(models.Field):
    def db_type(self, connection):
        if connection.vendor == 'microsoft':
            return 'BINARY(16)'
        return 'uuid'

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        if isinstance(value, (bytes, memoryview)):
            value = uuid.UUID(bytes=bytes(value))
        if not isinstance(value, str):
            value = str(value)
        return RanjId.from_str(value)
```

### ID Generation via pre_save()

On MSSQL, stored procedures can't be used as column defaults. Both fields use `pre_save()` to generate IDs before INSERT:

```python
def pre_save(self, model_instance, add):
    value = getattr(model_instance, self.attname)
    if add and value is None:
        from django.db import connection
        with connection.cursor() as cursor:
            if connection.vendor == 'microsoft':
                cursor.execute("EXEC generate_id")
            else:
                cursor.execute("SELECT generate_id()")
            value = cursor.fetchone()[0]
            # Wrap in HeerId/RanjId type
            ...
        setattr(model_instance, self.attname, value)
    return value
```

Both backends use `pre_save()` as the primary mechanism for Django. On Postgres, `db_default` is additionally kept as a safety net for non-Django inserts (direct SQL, admin tools). On MSSQL, `pre_save()` is the only mechanism — raw SQL users must call the stored proc explicitly (documented in the SQL submodule README).

### Django Migration

The migration becomes backend-aware, reading SQL from `sql/postgres/` or `sql/mssql/` based on `connection.vendor`:

```python
def get_install_sql():
    from django.db import connection
    if connection.vendor == 'microsoft':
        sql_dir = files("heeranjid") / "sql" / "mssql"
    else:
        sql_dir = files("heeranjid") / "sql" / "postgres"
    # ... read and return SQL
```

The bundled SQL files in `heeranjid-python/python/heeranjid/sql/` will need both `postgres/` and `mssql/` subdirectories.

## Testing Strategy

### Docker Compose

`docker-compose.yml` at repo root for local dev:

```yaml
services:
  postgres:
    image: postgres:latest
    ports: ["5432:5432"]
    environment:
      POSTGRES_DB: heeranjid
      POSTGRES_PASSWORD: postgres

  mssql:
    image: mcr.microsoft.com/mssql/server:2022-latest
    ports: ["1433:1433"]
    environment:
      ACCEPT_EULA: "Y"
      MSSQL_SA_PASSWORD: "HeeRanjID_Test1"
```

Databases only — Python runs on host via uv. TODO: custom Python Docker image for fully containerized dev workflow.

### Python Test Structure

```
heeranjid-python/
├── tests/
│   ├── test_heerid.py              # existing (no DB)
│   ├── test_ranjid.py              # existing (no DB)
│   ├── test_django_fields.py       # existing (SQLite)
│   ├── test_postgres.py            # NEW: Postgres integration
│   └── test_mssql.py               # NEW: MSSQL integration
```

### Test Requirements

- Tests **fail** (not skip) if the database container is unavailable
- `DATABASE_URL` env var for Postgres connection
- `MSSQL_URL` env var for MSSQL connection
- Both integration test files verify:
  1. Schema installation succeeds
  2. Seed data inserts correctly
  3. HeerId generation returns valid IDs that decode correctly
  4. RanjId generation returns valid IDs that decode correctly
  5. Bulk generation returns correct count with chronological ordering
  6. Session node ID management works
  7. Clock rollback detection fires appropriately
  8. Django `pre_save()` populates IDs on model save

### Test Dependencies

Added to `pyproject.toml`:
```toml
[project.optional-dependencies]
django = ["django>=4.2"]
mssql = ["mssql-django>=1.4", "pyodbc>=5.0"]
dev = ["pytest>=8.0", "maturin>=1.0"]
```

System dependency: ODBC Driver 18 for SQL Server (must be installed on test host).

## Documentation

Raw SQL usage for MSSQL must be clearly documented in the SQL submodule README:

```sql
-- 1. Install schema and procedures
-- Run sql/mssql/install.sql against your database

-- 2. Seed a default node
-- Run sql/mssql/seed.sql

-- 3. Set session node ID
EXEC heer_set_node_id @node_id = 1;

-- 4. Generate IDs
EXEC generate_id;                              -- single HeerId
EXEC generate_ids @count = 10;                 -- bulk HeerId
EXEC generate_ranjid;                          -- single RanjId
EXEC generate_ranjids @count = 10;             -- bulk RanjId

-- 5. With explicit node_id (no session setup needed)
EXEC generate_id @node_id = 1;
EXEC generate_ranjid @node_id = 1;
```

## Follow-up Work (Out of Scope)

- JS/Prisma MSSQL adaptation (`EXEC` instead of `SELECT` for stored procs, `BINARY(16)` handling for RanjId)
- .NET MSSQL adaptation: `SqlHelper` needs to bundle and serve `sql/mssql/` SQL, `ModelBuilderExtensions` needs backend-aware `HasDefaultValueSql` (stored proc syntax differs), `RanjId` needs `BINARY(16)` column type mapping via EF Core
- Custom Python Docker image for containerized dev/CI
- CI pipeline updates for MSSQL test matrix
