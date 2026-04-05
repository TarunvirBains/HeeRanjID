# Oracle Database Backend Design

## Goal

Add Oracle Database as a third backend for HeeRanjID ID generation, implementing full functional parity with the existing Postgres (`sql/postgres/`) and MSSQL (`sql/mssql/`) backends.

---

## Research Findings

### 1. Oracle PL/SQL Stored Procedure Syntax

Oracle uses `CREATE OR REPLACE PROCEDURE` and `CREATE OR REPLACE FUNCTION` with either `IS` or `AS` as the body delimiter. Parameters use `IN`, `OUT`, or `IN OUT` mode annotations:

```sql
CREATE OR REPLACE PROCEDURE heer_set_node_id(
    p_node_id IN INTEGER
)
AS
BEGIN
    -- body
END heer_set_node_id;
```

Functions that return values use `RETURN`:

```sql
CREATE OR REPLACE FUNCTION current_heer_node_id
RETURN INTEGER
AS
    v_val VARCHAR2(40);
BEGIN
    v_val := SYS_CONTEXT('HEER_CTX', 'node_id');
    IF v_val IS NULL THEN
        RAISE_APPLICATION_ERROR(-20001, 'heer.node_id is not set for this session');
    END IF;
    RETURN TO_NUMBER(v_val);
END current_heer_node_id;
```

Exception handling uses `RAISE_APPLICATION_ERROR(-20000 to -20999, 'message')` rather than Postgres `RAISE EXCEPTION` or MSSQL `THROW`.

Procedures and functions support `CREATE OR REPLACE` which is idempotent on re-run — matching Postgres's `CREATE OR REPLACE FUNCTION` and MSSQL's `CREATE OR ALTER PROCEDURE`. DDL statements (including `CREATE OR REPLACE PROCEDURE`) cannot be executed directly inside a PL/SQL procedure body; they must be executed via `EXECUTE IMMEDIATE` for dynamic procedure regeneration (relevant to `heer_configure`).

### 2. High-Precision Timestamps — Oracle `SYSTIMESTAMP`

Oracle's `SYSTIMESTAMP` returns `TIMESTAMP WITH TIME ZONE` with sub-second fractional digits. It is the equivalent of Postgres `clock_timestamp()`: both reflect wall-clock time at the point of the call (not transaction start time). The number of fractional second digits defaults to 6 (microseconds) and can be specified up to 9. On Linux/Unix platforms, resolution is typically microseconds; Windows is typically milliseconds only.

To compute elapsed microseconds since a Unix epoch:

```sql
-- Milliseconds since 1970-01-01:
SELECT (SYSTIMESTAMP - TIMESTAMP '1970-01-01 00:00:00 UTC')
       DAY(9) TO SECOND(6)
-- Decompose the interval:
SELECT EXTRACT(DAY    FROM diff) * 86400000
     + EXTRACT(HOUR   FROM diff) * 3600000
     + EXTRACT(MINUTE FROM diff) * 60000
     + EXTRACT(SECOND FROM diff) * 1000  -- gives milliseconds as FLOAT
```

For microsecond-precision arithmetic (required for RanjId's `NUMERIC(30,0)` tick counter), the INTERVAL DAY TO SECOND can be decomposed similarly with a multiplier of 1,000,000. The result must be cast to `NUMBER` for large-integer arithmetic.

`SYSDATE` and `CURRENT_TIMESTAMP` are the equivalent of Postgres `now()` (transaction start time) and should **not** be used for ID generation. `SYSTIMESTAMP` is correct.

### 3. UUID / RAW(16) for RanjId Storage

Oracle does not have a native `UUID` type at the column level in pre-23ai versions. The standard Oracle pattern for storing 16-byte binary IDs is `RAW(16)`. This is byte-exact and preserves insertion-order for big-endian UUIDs, matching HeeRanjID's requirement for chronological sort order.

Oracle 23ai introduced a native `UUID` column type and `UUID()` generation function, but targeting `RAW(16)` is the more portable choice and works identically on Oracle 12c, 19c, 21c, and 23ai.

Oracle provides conversion functions:
- `RAW_TO_UUID(raw_val)` — converts `RAW(16)` to the 36-character `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` string format (Oracle 23ai+)
- `UUID_TO_RAW(uuid_str)` — the reverse (Oracle 23ai+)

For pre-23ai compatibility the conversion can be done via `RAWTOHEX` and `UTL_RAW.CAST_TO_RAW`.

**Important**: MSSQL stores RanjId as `BINARY(16)` to avoid `UNIQUEIDENTIFIER`'s mixed-endian byte swapping. Oracle's `RAW(16)` is straight big-endian, so it preserves sort order correctly without any workaround.

### 4. Row Locking — Oracle `SELECT FOR UPDATE`

Oracle natively supports `SELECT ... FOR UPDATE`, which is syntactically identical to Postgres. It acquires an exclusive row lock held until the end of the transaction:

```sql
SELECT last_id_time, last_sequence
  INTO v_last_time, v_last_seq
  FROM heer_node_state
 WHERE node_id = p_node_id
   FOR UPDATE;
```

Oracle's `FOR UPDATE` is simpler to write than MSSQL's `WITH (UPDLOCK, ROWLOCK, HOLDLOCK)` table hint. Unlike MSSQL, no extra hint is needed to prevent lock escalation — Oracle defaults to row-level locking. The `FOR UPDATE NOWAIT` or `FOR UPDATE WAIT n` variants are available if needed.

### 5. Session Variables — Oracle Application Context

Oracle has no equivalent of Postgres `set_config()`/`current_setting()` or MSSQL `sp_set_session_context()`/`SESSION_CONTEXT()` that works without schema setup. The Oracle mechanism is **Application Context**:

1. **Create a named context** (one-time DDL, requires `CREATE ANY CONTEXT` privilege or DBA):
   ```sql
   CREATE OR REPLACE CONTEXT heer_ctx USING heer_session_pkg;
   ```

2. **Create the designated trusted package** — only code inside this package may call `DBMS_SESSION.SET_CONTEXT` for this namespace:
   ```sql
   CREATE OR REPLACE PACKAGE heer_session_pkg AS
       PROCEDURE set_node_id(p_node_id IN INTEGER);
       FUNCTION  current_node_id RETURN INTEGER;
       -- ... ranj equivalents
   END heer_session_pkg;
   ```

3. **Set values** (only callable from within the trusted package):
   ```sql
   DBMS_SESSION.SET_CONTEXT('HEER_CTX', 'node_id', TO_CHAR(p_node_id));
   ```

4. **Read values** (callable from anywhere):
   ```sql
   SYS_CONTEXT('HEER_CTX', 'node_id')  -- returns VARCHAR2, NULL if not set
   ```

Values persist for the duration of the session (connection). In connection pools, the context must be cleared or reset on connection handback — the `DBMS_SESSION.CLEAR_CONTEXT` procedure can clear all attributes of a namespace.

**Key constraint**: `CREATE CONTEXT` requires `CREATE ANY CONTEXT` system privilege or must be granted by a DBA. This is a one-time schema setup step, analogous to how Postgres requires `SET search_path` and MSSQL requires `CREATE SCHEMA`. It should live in `sql/oracle/schema.sql`.

### 6. Large Integer Arithmetic — Oracle `NUMBER`

Oracle's `NUMBER(p, s)` is equivalent to Postgres `NUMERIC(p, s)` — `NUMERIC` is simply an alias. There is no precision cap equivalent to MSSQL's `NUMERIC(38,0)` upper limit: Oracle `NUMBER` supports up to 38 significant digits, identical in effective range.

For HeeRanjID's RanjId tick counter (up to 90 bits, ~28 decimal digits), `NUMBER(30, 0)` is a direct match with no changes required. Arithmetic operations (`/`, `MOD`, `POWER`, `FLOOR`) all work correctly on `NUMBER`.

Oracle note: `POWER(2, n)` returns `NUMBER`, not `BIGINT`, so large-power arithmetic is natural without needing explicit casts (unlike MSSQL's `POWER(CAST(2 AS NUMERIC(38,0)), n)` workaround).

### 7. Dynamic SQL — Oracle `EXECUTE IMMEDIATE`

Oracle's `EXECUTE IMMEDIATE` is the native dynamic SQL mechanism, equivalent to Postgres `EXECUTE format(...)` and MSSQL `sp_executesql`:

```sql
EXECUTE IMMEDIATE 'CREATE OR REPLACE FUNCTION generate_ids ...' ;
```

Key differences from the other backends:
- `EXECUTE IMMEDIATE` can execute DDL statements (including `CREATE OR REPLACE PROCEDURE/FUNCTION`). This is the mechanism `heer_configure` uses to bake the epoch constant into the generation functions.
- DDL inside `EXECUTE IMMEDIATE` **commits any open transaction implicitly**. `heer_configure` should not be called inside a transaction; it is an admin-only operation. This matches the Postgres behaviour where `heer_configure()` uses `EXECUTE format(...)` which also issues a DDL statement.
- String quoting inside dynamic SQL: Oracle uses `q'[...]'` or `q'$...$'` quoting (the "q-quote" operator) to avoid escaping single quotes inside the dynamic SQL string, analogous to Postgres dollar-quoting (`$$...$$ ` or `$fmt$...$fmt$`).

---

## Key Differences from Postgres / MSSQL

| Feature | Postgres | MSSQL | Oracle |
|---|---|---|---|
| **Procedure/function DDL** | `CREATE OR REPLACE FUNCTION ... LANGUAGE plpgsql` | `CREATE OR ALTER PROCEDURE ... AS BEGIN ... END` | `CREATE OR REPLACE PROCEDURE ... AS BEGIN ... END` |
| **Wall-clock timestamp** | `clock_timestamp()` | `SYSUTCDATETIME()` | `SYS_EXTRACT_UTC(SYSTIMESTAMP)` |
| **Milliseconds since epoch** | `FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT` | `DATEDIFF_BIG(MILLISECOND, '1970-01-01', SYSUTCDATETIME())` | INTERVAL arithmetic: decompose DAY TO SECOND interval, multiply components |
| **Large integer type** | `NUMERIC(30,0)` | `NUMERIC(38,0)` | `NUMBER(30,0)` (identical to Postgres) |
| **UUID/binary 16-byte type** | `UUID` | `BINARY(16)` | `RAW(16)` |
| **Row lock** | `SELECT ... FOR UPDATE` | `SELECT ... WITH (UPDLOCK, ROWLOCK, HOLDLOCK)` | `SELECT ... FOR UPDATE` (identical to Postgres) |
| **Session variable set** | `set_config('heer.node_id', val, false)` | `EXEC sp_set_session_context N'heer_node_id', @val` | `DBMS_SESSION.SET_CONTEXT('HEER_CTX', 'node_id', val)` (requires trusted package) |
| **Session variable read** | `current_setting('heer.node_id', true)` | `SESSION_CONTEXT(N'heer_node_id')` | `SYS_CONTEXT('HEER_CTX', 'node_id')` |
| **Session variable scope** | Per-session, no setup required | Per-session, no setup required | Per-session, requires one-time `CREATE CONTEXT` DDL |
| **Connection pool reset** | Not needed (session ends) | `sp_set_session_context` with `@read_only=0` | Must call `DBMS_SESSION.CLEAR_CONTEXT('HEER_CTX')` on connection return |
| **Dynamic SQL** | `EXECUTE format($fmt$...$fmt$, args)` | `EXEC sp_executesql @sql, @params, @val` | `EXECUTE IMMEDIATE 'sql text'` |
| **Dynamic DDL in procedure** | `EXECUTE format(...)` directly | `EXEC sp_executesql` | `EXECUTE IMMEDIATE` (issues implicit commit) |
| **Idempotent table creation** | `CREATE TABLE IF NOT EXISTS` | `IF NOT EXISTS (SELECT ...) CREATE TABLE` | `CREATE TABLE IF NOT EXISTS` (Oracle 23ai+) or `EXECUTE IMMEDIATE` with exception handler for ORA-00955 |
| **Generate series** | `generate_series(n, m)` | `WHILE` loop | `WHILE` loop (no generate_series) |
| **Raise exception** | `RAISE EXCEPTION 'msg'` | `THROW 50001, 'msg', 1` | `RAISE_APPLICATION_ERROR(-20001, 'msg')` |
| **Error code range** | SQLSTATE codes | 50000–50999 user range | -20000 to -20999 user range |
| **Upsert / conflict** | `INSERT ... ON CONFLICT DO NOTHING` | `IF NOT EXISTS ... INSERT` | `MERGE ... WHEN NOT MATCHED THEN INSERT` or `INSERT ... WHERE NOT EXISTS` |
| **String quoting in dynamic SQL** | Dollar-quoting: `$fmt$...$fmt$` | N-string literals: `N'...'` | Q-quote operator: `q'[...]'` or `q'$...$'` |
| **Procedure output** | `RETURNS TABLE(...)` set-returning function | Result set from temp table + `SELECT` | `OUT SYS_REFCURSOR` parameter or pipelined table function |
| **Returning multiple rows** | Set-returning function: `RETURN QUERY SELECT ...` | Insert into `#temp` then `SELECT` | `OPEN ref_cursor FOR SELECT ...` (cursor) or pipelined function |

---

## Proposed Implementation

### Directory Structure

Following the established pattern from `sql/postgres/` and `sql/mssql/`:

```
sql/oracle/
├── schema.sql          -- table DDL + CREATE CONTEXT
├── seed.sql            -- default node insert (MERGE-based for idempotency)
├── install.sql         -- run all files in order
├── procedures/         -- named "procedures/" like mssql/, not "functions/"
│   ├── session.sql     -- set/get node_id via Application Context
│   ├── generate_heerid.sql
│   ├── generate_ranjid.sql
│   └── configure.sql   -- heer_configure equivalent
└── queries/
    ├── fetch_node.sql
    ├── fetch_active_node.sql
    └── fetch_epoch.sql
```

### Table Definitions

Equivalent DDL with Oracle type mappings:

```sql
-- schema.sql

-- Application context namespace for session node IDs
-- Requires CREATE ANY CONTEXT privilege
CREATE OR REPLACE CONTEXT heer_ctx USING heer_session_pkg;

CREATE TABLE heer_nodes (
    node_id       INTEGER PRIMARY KEY,
    name          VARCHAR2(255)  NOT NULL,
    description   VARCHAR2(4000),
    is_active     NUMBER(1,0)    DEFAULT 1 NOT NULL,
    created_at    TIMESTAMP      DEFAULT SYSTIMESTAMP NOT NULL,
    last_accessed TIMESTAMP      DEFAULT SYSTIMESTAMP NOT NULL
);

CREATE TABLE heer_config (
    id                  INTEGER        PRIMARY KEY CHECK (id = 1),
    epoch               TIMESTAMP      NOT NULL,
    precision           VARCHAR2(2)    DEFAULT 'ns' NOT NULL,
    ranj_epoch_offset   NUMBER(30,0)   DEFAULT 0 NOT NULL,
    updated_at          TIMESTAMP      DEFAULT SYSTIMESTAMP NOT NULL
);

CREATE TABLE heer_node_state (
    node_id         INTEGER        PRIMARY KEY
                    REFERENCES heer_nodes(node_id) ON DELETE CASCADE,
    last_id_time    NUMBER(19,0)   DEFAULT 0 NOT NULL,   -- equivalent of BIGINT
    last_sequence   NUMBER(5,0)    DEFAULT 0 NOT NULL,   -- equivalent of SMALLINT
    updated_at      TIMESTAMP      DEFAULT SYSTIMESTAMP NOT NULL
);

CREATE TABLE heer_ranj_node_state (
    node_id         INTEGER        PRIMARY KEY
                    REFERENCES heer_nodes(node_id) ON DELETE CASCADE,
    last_id_time    NUMBER(30,0)   DEFAULT 0 NOT NULL,
    last_sequence   INTEGER        DEFAULT 0 NOT NULL,
    updated_at      TIMESTAMP      DEFAULT SYSTIMESTAMP NOT NULL
);
```

Type mapping summary:

| Postgres / MSSQL Type | Oracle Type | Notes |
|---|---|---|
| `BOOLEAN` / `BIT` | `NUMBER(1,0)` | 1 = true, 0 = false |
| `TEXT` / `NVARCHAR(255)` | `VARCHAR2(255)` | Oracle VARCHAR2 is UTF-8 natively |
| `TEXT` / `NVARCHAR(MAX)` | `VARCHAR2(4000)` or `CLOB` | 4000 fits descriptions; use CLOB for unbounded |
| `TIMESTAMP` / `DATETIME2` | `TIMESTAMP` | Oracle TIMESTAMP has up to 9 fractional digits |
| `UUID` / `BINARY(16)` | `RAW(16)` | Big-endian byte-exact; no mixed-endian issues |
| `NUMERIC(30,0)` / `NUMERIC(38,0)` | `NUMBER(30,0)` | Direct alias; Oracle supports up to 38 significant digits |
| `BIGINT` | `NUMBER(19,0)` | Oracle has no native BIGINT; NUMBER(19,0) covers 2^63 |
| `SMALLINT` | `NUMBER(5,0)` | Covers 0..32767 |
| `INTEGER` / `INT` | `INTEGER` | Oracle INTEGER is an alias for NUMBER(38,0); use NUMBER(10,0) for clarity if desired |
| `ON CONFLICT DO NOTHING` / `IF NOT EXISTS INSERT` | `MERGE ... WHEN NOT MATCHED THEN INSERT` | Oracle 9i+; or INSERT with `WHERE NOT EXISTS` sub-select |

### Session Management Approach

Oracle requires a context definition and a designated trusted package. These live together in `procedures/session.sql`:

```sql
-- procedures/session.sql

CREATE OR REPLACE PACKAGE heer_session_pkg AS
    PROCEDURE set_node_id(p_node_id IN INTEGER);
    PROCEDURE set_ranj_node_id(p_node_id IN INTEGER);
    FUNCTION  current_node_id   RETURN INTEGER;
    FUNCTION  current_ranj_node_id RETURN INTEGER;
END heer_session_pkg;
/

CREATE OR REPLACE PACKAGE BODY heer_session_pkg AS

    PROCEDURE set_node_id(p_node_id IN INTEGER) AS
        v_valid INTEGER;
    BEGIN
        IF p_node_id IS NULL THEN
            RAISE_APPLICATION_ERROR(-20001, 'node_id cannot be null');
        END IF;
        IF p_node_id < 0 OR p_node_id > 511 THEN
            RAISE_APPLICATION_ERROR(-20002,
                'node_id ' || p_node_id || ' is out of range for HeerId');
        END IF;
        SELECT node_id INTO v_valid
          FROM heer_nodes
         WHERE node_id = p_node_id AND is_active = 1;
        -- Raises NO_DATA_FOUND if not found; let it propagate or catch:
        DBMS_SESSION.SET_CONTEXT('HEER_CTX', 'node_id', TO_CHAR(p_node_id));
    EXCEPTION
        WHEN NO_DATA_FOUND THEN
            RAISE_APPLICATION_ERROR(-20003,
                'node_id ' || p_node_id || ' is not registered as an active Heer node');
    END set_node_id;

    FUNCTION current_node_id RETURN INTEGER AS
        v_val VARCHAR2(40);
    BEGIN
        v_val := SYS_CONTEXT('HEER_CTX', 'node_id');
        IF v_val IS NULL THEN
            RAISE_APPLICATION_ERROR(-20004, 'heer.node_id is not set for this session');
        END IF;
        -- Re-validate: call set_node_id to confirm node is still active
        set_node_id(TO_NUMBER(v_val));
        RETURN TO_NUMBER(v_val);
    END current_node_id;

    -- ... set_ranj_node_id and current_ranj_node_id follow same pattern

END heer_session_pkg;
/
```

The `CREATE CONTEXT` statement in `schema.sql` names `heer_session_pkg` as the trusted package. Only calls to `DBMS_SESSION.SET_CONTEXT` originating from within `heer_session_pkg` will succeed.

### ID Generation Stored Procedures

#### Returning Multiple Rows

Oracle procedures cannot use `RETURNS TABLE` like Postgres or implicit result sets like MSSQL. The two viable approaches are:

**Option A: `OUT SYS_REFCURSOR` (recommended)**

```sql
CREATE OR REPLACE PROCEDURE generate_ids(
    p_node_id       IN  INTEGER  DEFAULT NULL,
    p_count         IN  INTEGER,
    p_allow_spanning IN INTEGER  DEFAULT 1,
    p_result        OUT SYS_REFCURSOR
)
AS
    ...
BEGIN
    ...
    OPEN p_result FOR
        SELECT id FROM heer_id_results_tmp
         WHERE session_id = SYS_CONTEXT('USERENV', 'SESSIONID');
END;
```

**Option B: Pipelined Table Function (cleaner for callers)**

```sql
CREATE OR REPLACE TYPE heer_id_tab AS TABLE OF NUMBER(19,0);

CREATE OR REPLACE FUNCTION generate_ids(
    p_node_id        IN INTEGER DEFAULT NULL,
    p_count          IN INTEGER DEFAULT 1,
    p_allow_spanning IN INTEGER DEFAULT 1
) RETURN heer_id_tab PIPELINED
AS
    v_id NUMBER(19,0);
BEGIN
    ...
    PIPE ROW(v_id);
    ...
END;
```

A pipelined function is called as `SELECT * FROM TABLE(generate_ids(1, 10))`, which is the most ergonomic API. The `SYS_REFCURSOR` approach requires callers to handle cursor binding, which is less ergonomic for raw SQL usage but works well with JDBC/cx_Oracle drivers.

**Recommendation**: Implement pipelined table functions for `generate_ids` and `generate_ranjids`. Use `OUT SYS_REFCURSOR` as a fallback if pipelining has concurrency complications under the `FOR UPDATE` lock model.

#### HeerId Generation — Key Translation

Postgres uses `<<` bit-shift operators; MSSQL uses `* POWER(CAST(2 AS BIGINT), n)`. Oracle `NUMBER` does not support bitwise operators. All bit manipulation must use arithmetic:

```sql
-- Postgres:  (current_tick::BIGINT << 22) | (in_node_id::BIGINT << 13) | sequence
-- MSSQL:     @tick * POWER(CAST(2 AS BIGINT), 22) | @node * POWER(CAST(2 AS BIGINT), 13) | @seq
-- Oracle:
v_id := (v_current_tick * POWER(2, 22))
      + (p_node_id * POWER(2, 13))
      + v_sequence;
-- Addition is safe here because the bit fields are non-overlapping
```

For the 128-bit RanjId, the big-endian `RAW(16)` must be assembled from two 64-bit halves. Oracle's `UTL_RAW.CONCAT` and `UTL_RAW.CAST_FROM_NUMBER` can be used, or the value can be assembled via `TO_RAW`/hex string manipulation similar to the Postgres `to_hex` approach.

#### Epoch / Time Arithmetic

Oracle does not have `DATEDIFF_BIG`. Elapsed milliseconds/microseconds must be computed via INTERVAL decomposition:

```sql
DECLARE
    v_diff     INTERVAL DAY(9) TO SECOND(6);
    v_now_ms   NUMBER(19,0);
BEGIN
    v_diff   := SYS_EXTRACT_UTC(SYSTIMESTAMP)
              - TIMESTAMP '1970-01-01 00:00:00';
    v_now_ms := (EXTRACT(DAY    FROM v_diff) * 86400000)
              + (EXTRACT(HOUR   FROM v_diff) * 3600000)
              + (EXTRACT(MINUTE FROM v_diff) * 60000)
              + FLOOR(EXTRACT(SECOND FROM v_diff) * 1000);
    -- Subtract epoch_ms (read from heer_config) for elapsed tick
    v_elapsed_ms := v_now_ms - v_epoch_ms;
END;
```

For RanjId microseconds, replace `* 1000` with `* 1000000` in the final line.

### `heer_configure` Equivalent

The `heer_configure` procedure bakes the epoch constant into the generation functions using dynamic DDL. In Oracle, `EXECUTE IMMEDIATE` issues DDL inside a PL/SQL block. The important caveat is that DDL triggers an implicit `COMMIT`. Since `heer_configure` is a privileged admin-only operation (not called during normal ID generation), this implicit commit is acceptable.

The `REVOKE EXECUTE` equivalent in Oracle is:

```sql
REVOKE EXECUTE ON heer_configure FROM PUBLIC;
-- or restrict by granting only to specific roles:
GRANT EXECUTE ON heer_configure TO heer_admin_role;
```

String quoting inside dynamic SQL uses Oracle's q-quote operator instead of Postgres dollar-quoting:

```sql
-- Postgres:   EXECUTE format($fmt$ CREATE OR REPLACE FUNCTION ... $fmt$, epoch_ms);
-- Oracle:
EXECUTE IMMEDIATE q'[
    CREATE OR REPLACE FUNCTION generate_ids(...]
    -- use ]' to close, or q'$...$' if ] appears in the body
```

---

## Challenges

### 1. No Native Bitwise Operators

Oracle `NUMBER` types do not support `<<`, `>>`, `|`, or `&`. All bit manipulation in the HeerId and RanjId assembly algorithms must be rewritten using arithmetic (`*`, `+`, `MOD`, `FLOOR`, `POWER`). This is the same constraint as MSSQL (which also has no bit-shift on `BIGINT`) but adds complexity for the 128-bit RanjId assembly where MSSQL uses `|` on `BIGINT`.

For `NUMBER` in Oracle, since bit fields are non-overlapping, `+` is equivalent to `|`. For the variant/version bit masking in RanjId that sets specific bits (e.g., the `0x8000000000000000` variant marker), equivalent arithmetic is:
```sql
-- Set bit 63: add 2^63 (but NUMBER can hold this, unlike signed BIGINT)
v_lo := POWER(2, 63)
      + (v_precision_bits * POWER(2, 60))
      + (v_ts_low * POWER(2, 31))
      + (p_node_id * POWER(2, 16))
      + v_sequence;
```

The 128-bit RAW assembly then concatenates two 64-bit halves via `UTL_RAW.CONCAT(hi_raw, lo_raw)`.

### 2. Application Context Setup Requires DBA Privileges

`CREATE CONTEXT` requires `CREATE ANY CONTEXT` (or `CREATE SESSION CONTEXT` in 12c+). This is a schema-level DDL that must be run by a DBA or a user with elevated privileges. Unlike Postgres `set_config` (no setup needed) or MSSQL `sp_set_session_context` (built-in, no setup needed), Oracle users must explicitly configure the context namespace before the session management procedures will work.

This is a deployment friction point: DBAs must run `schema.sql` under a privileged account. The `install.sql` file should include clear comments about the required privilege.

### 3. Connection Pool Session Isolation

In connection pools (common with Java/cx_Oracle/ADO.NET), connections are reused across logical user sessions. Oracle does not automatically clear Application Context values when a connection is returned to the pool. Callers must explicitly call `DBMS_SESSION.CLEAR_CONTEXT('HEER_CTX')` — or call `DBMS_SESSION.MODIFY_PACKAGE_STATE(DBMS_SESSION.REINITIALIZE)` — before returning the connection to the pool.

This is more operational burden than Postgres (where sessions terminate on disconnect) or MSSQL (where `SESSION_CONTEXT` resets with the session). This should be prominently documented.

### 4. Returning Multiple Result Rows

Oracle does not support set-returning functions (Postgres) or implicit multi-row result sets from procedures (MSSQL). The pipelined table function approach adds a required schema object (`CREATE TYPE heer_id_tab AS TABLE OF NUMBER(19,0)`) and a separate type for `RAW(16)` results. These types must be created as schema-level objects before the functions can be compiled.

### 5. `EXECUTE IMMEDIATE` DDL Commits Open Transactions

The `heer_configure` procedure issues DDL via `EXECUTE IMMEDIATE`, which triggers an implicit `COMMIT` in Oracle. If `heer_configure` is called inside a transaction (e.g., inside a migration tool's wrapping transaction), it will commit more than intended. The procedure's documentation must warn callers not to invoke it inside an active transaction.

### 6. Idempotent Table Creation Pre-23ai

`CREATE TABLE IF NOT EXISTS` syntax was introduced in Oracle 23ai. For Oracle 12c/19c/21c compatibility, idempotent table creation requires either:
- Wrapping in an `EXECUTE IMMEDIATE` block with an exception handler catching `ORA-00955` (name already used by an existing object)
- Using `DBMS_METADATA` checks first

The `schema.sql` file should target the widest supported version (Oracle 19c, the current long-term release as of 2026) and use the exception-handler pattern.

---

## What's NOT in Scope

- Python/Django Oracle backend adapter — Django's `cx_Oracle`/`oracledb` backend support, `connection.vendor == 'oracle'` detection, pipelined cursor result handling. Future work.
- .NET Oracle provider integration — `Oracle.ManagedDataAccess` NuGet package, `OracleCommand` for ref cursor output parameters, EF Core Oracle provider (Devart or Oracle's official). Future work.
- Rust sqlx Oracle support — sqlx does not support Oracle as of 2026. No Rust integration path available.
- TypeScript/Prisma Oracle support — Prisma does not support Oracle. Future work dependent on upstream.
- Oracle-specific CI pipeline — Docker image (`container-registry.oracle.com/database/free:latest`) and CI test matrix. Separate work item.
- Oracle 23ai-specific features — native `UUID` column type, `IF NOT EXISTS` DDL. The implementation should target Oracle 19c (LTS) for maximum compatibility.
