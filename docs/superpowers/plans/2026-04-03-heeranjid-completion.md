# HeeRanjID Full Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the HeeRanjID system — all remaining phases from the implementation plan, producing a production-ready crate with RanjId SQL generation, startup validation, comprehensive tests, and clean ergonomics.

**Architecture:** The system uses shared SQL in `sql/` (a git submodule) consumed by a Rust `sqlx` crate via `include_str!`. PostgreSQL functions handle ID generation server-side. The Rust crate provides typed wrappers, validation, and test infrastructure. HeerId (64-bit) generation is complete; RanjId (128-bit UUIDv7) generation is the largest remaining gap.

**Tech Stack:** Rust 2024 edition, sqlx 0.8 (Postgres), PostgreSQL plpgsql, uuid crate, serde, thiserror, tokio (tests)

---

### Task 1: Add boundary and edge-case unit tests for core ID types

**Files:**
- Modify: `src/lib.rs` (tests module, line 17-95)

- [ ] **Step 1: Add HeerId boundary tests**

Add these tests to the `#[cfg(test)] mod tests` block in `src/lib.rs`:

```rust
#[test]
fn heerid_accepts_max_field_values() {
    let id = HeerId::new(HeerId::MAX_TIMESTAMP_MS, HeerId::MAX_NODE_ID, HeerId::MAX_SEQUENCE).unwrap();
    let parts = id.into_parts();
    assert_eq!(parts.timestamp_ms, HeerId::MAX_TIMESTAMP_MS);
    assert_eq!(parts.node_id, HeerId::MAX_NODE_ID);
    assert_eq!(parts.sequence, HeerId::MAX_SEQUENCE);
}

#[test]
fn heerid_rejects_overflow_timestamp() {
    let err = HeerId::new(HeerId::MAX_TIMESTAMP_MS + 1, 0, 0).unwrap_err();
    assert!(matches!(err, Error::TimestampOutOfRange { .. }));
}

#[test]
fn heerid_rejects_overflow_node_id() {
    let err = HeerId::new(0, HeerId::MAX_NODE_ID + 1, 0).unwrap_err();
    assert!(matches!(err, Error::NodeIdOutOfRange { .. }));
}

#[test]
fn heerid_rejects_overflow_sequence() {
    let err = HeerId::new(0, 0, HeerId::MAX_SEQUENCE + 1).unwrap_err();
    assert!(matches!(err, Error::SequenceOutOfRange { .. }));
}

#[test]
fn heerid_zero_round_trips() {
    let id = HeerId::new(0, 0, 0).unwrap();
    assert_eq!(id.as_i64(), 0);
    let parts = id.into_parts();
    assert_eq!(parts.timestamp_ms, 0);
    assert_eq!(parts.node_id, 0);
    assert_eq!(parts.sequence, 0);
}

#[test]
fn heerid_from_str_round_trips() {
    let id = HeerId::new(1000, 5, 42).unwrap();
    let s = id.to_string();
    let parsed: HeerId = s.parse().unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn heerid_from_str_rejects_negative() {
    let err = "-1".parse::<HeerId>().unwrap_err();
    assert_eq!(err, Error::NegativeHeerId);
}

#[test]
fn heerid_from_str_rejects_garbage() {
    let err = "not_a_number".parse::<HeerId>().unwrap_err();
    assert!(matches!(err, Error::InvalidHeerIdString(_)));
}
```

- [ ] **Step 2: Add RanjId boundary tests**

Add these tests immediately after the HeerId tests:

```rust
#[test]
fn ranjid_accepts_max_field_values() {
    let id = RanjId::new(RanjId::MAX_TIMESTAMP_MICROS, RanjId::MAX_NODE_ID, RanjId::MAX_SEQUENCE).unwrap();
    let parts = id.into_parts();
    assert_eq!(parts.timestamp_micros, RanjId::MAX_TIMESTAMP_MICROS);
    assert_eq!(parts.node_id, RanjId::MAX_NODE_ID);
    assert_eq!(parts.sequence, RanjId::MAX_SEQUENCE);
}

#[test]
fn ranjid_rejects_overflow_timestamp() {
    let err = RanjId::new(RanjId::MAX_TIMESTAMP_MICROS + 1, 0, 0).unwrap_err();
    assert!(matches!(err, Error::TimestampOutOfRange { .. }));
}

#[test]
fn ranjid_zero_round_trips() {
    let id = RanjId::new(0, 0, 0).unwrap();
    let parts = id.into_parts();
    assert_eq!(parts.timestamp_micros, 0);
    assert_eq!(parts.node_id, 0);
    assert_eq!(parts.sequence, 0);
}

#[test]
fn ranjid_from_str_round_trips() {
    let id = RanjId::new(1_000_000, 100, 200).unwrap();
    let s = id.to_string();
    let parsed: RanjId = s.parse().unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn ranjid_from_str_rejects_garbage() {
    let err = "not-a-uuid".parse::<RanjId>().unwrap_err();
    assert!(matches!(err, Error::InvalidRanjIdString(_)));
}

#[test]
fn ranjid_preserves_uuid_version_and_variant() {
    let id = RanjId::new(999_999, 42, 7).unwrap();
    let uuid = id.as_uuid();
    assert_eq!(uuid.get_version_num(), 7);
    assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
}

#[test]
fn serde_deserializes_heerid_from_string() {
    let id = HeerId::new(55, 7, 9).unwrap();
    let json = format!("\"{}\"", id.as_i64());
    let parsed: HeerId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn serde_deserializes_heerid_from_integer() {
    let id = HeerId::new(55, 7, 9).unwrap();
    let json = id.as_i64().to_string();
    let parsed: HeerId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib`
Expected: All new tests PASS (these test existing working code)

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs
git commit -m "Add boundary and edge-case tests for HeerId and RanjId types"
```

---

### Task 2: Add bootstrap/seed SQL and queries

**Files:**
- Create: `sql/postgres/seed.sql`
- Create: `sql/postgres/queries/fetch_active_node.sql`
- Modify: `sql/postgres/install.sql`

- [ ] **Step 1: Create seed SQL**

Create `sql/postgres/seed.sql`:

```sql
-- Default seed data for single-node deployments.
-- Safe to run multiple times (uses ON CONFLICT).

INSERT INTO heer_nodes (node_id, name, description, is_active)
VALUES (1, 'default', 'Default single-node instance', true)
ON CONFLICT (node_id) DO NOTHING;

INSERT INTO heer_node_state (node_id)
VALUES (1)
ON CONFLICT (node_id) DO NOTHING;

INSERT INTO heer_ranj_node_state (node_id)
VALUES (1)
ON CONFLICT (node_id) DO NOTHING;
```

- [ ] **Step 2: Create fetch_active_node query**

Create `sql/postgres/queries/fetch_active_node.sql`:

```sql
SELECT node_id, name, description, is_active
FROM heer_nodes
WHERE node_id = $1 AND is_active = true
```

- [ ] **Step 3: Update install.sql to include seed**

Update `sql/postgres/install.sql`:

```sql
\i postgres/schema.sql
\i postgres/functions/session.sql
\i postgres/functions/generate_heerid.sql
```

(No change needed — seed is intentionally separate from install so it's opt-in.)

- [ ] **Step 4: Commit**

```bash
cd sql && git add postgres/seed.sql postgres/queries/fetch_active_node.sql && git commit -m "Add seed data and active node query"
cd .. && git add sql && git commit -m "Update sql submodule with seed and active node query"
```

---

### Task 3: Add startup validation helpers in Rust

**Files:**
- Modify: `src/postgres.rs` (add validation functions, lines 30-68)
- Modify: `src/lib.rs` (re-export new items, lines 9-13)

- [ ] **Step 1: Add FETCH_ACTIVE_NODE_SQL constant and validation functions**

Add to `src/postgres.rs` after the existing constants (after line 16):

```rust
pub const SEED_SQL: &str = include_str!("../sql/postgres/seed.sql");
pub const FETCH_ACTIVE_NODE_SQL: &str = include_str!("../sql/postgres/queries/fetch_active_node.sql");
```

Add these functions after `fetch_epoch` (after line 68):

```rust
pub async fn fetch_active_node(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
) -> Result<Option<HeerNode>, sqlx::Error> {
    sqlx::query_as::<_, HeerNode>(FETCH_ACTIVE_NODE_SQL)
        .bind(i32::from(node_id))
        .fetch_optional(executor)
        .await
}

pub async fn validate_startup(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
) -> Result<HeerNode, StartupError> {
    let node = fetch_active_node(executor, node_id)
        .await
        .map_err(StartupError::Database)?;

    match node {
        Some(node) => Ok(node),
        None => Err(StartupError::NodeNotActive(node_id)),
    }
}

pub async fn validate_epoch(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
) -> Result<sqlx::types::time::PrimitiveDateTime, StartupError> {
    let epoch = fetch_epoch(executor)
        .await
        .map_err(StartupError::Database)?;

    match epoch {
        Some(epoch) => Ok(epoch),
        None => Err(StartupError::MissingEpoch),
    }
}

pub async fn seed_default_node<'e, E>(executor: E) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::raw_sql(SEED_SQL).execute(executor).await?;
    Ok(())
}
```

- [ ] **Step 2: Add StartupError to error.rs**

Add to `src/error.rs` after the existing `Error` enum:

```rust
#[derive(Debug, Error)]
pub enum StartupError {
    #[error("node {0} is not registered or not active")]
    NodeNotActive(u16),
    #[error("heer_config epoch is not configured")]
    MissingEpoch,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

- [ ] **Step 3: Update lib.rs exports**

In `src/lib.rs`, update the re-exports:

```rust
pub use error::{Error, StartupError};
pub use postgres::{
    FETCH_ACTIVE_NODE_SQL, FETCH_EPOCH_SQL, FETCH_NODE_SQL, GENERATE_HEERID_SQL, HeerConfig,
    HeerNode, INSTALL_SQL, SCHEMA_SQL, SEED_SQL, SESSION_SQL, fetch_active_node, fetch_epoch,
    fetch_node, install_schema, seed_default_node, validate_epoch, validate_heer_node_id,
    validate_startup,
};
```

- [ ] **Step 4: Run `cargo check` to verify compilation**

Run: `cargo check`
Expected: Compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/postgres.rs src/lib.rs
git commit -m "Add startup validation helpers and seed support"
```

---

### Task 4: Add integration tests for startup validation

**Files:**
- Modify: `tests/postgres.rs`

- [ ] **Step 1: Add startup validation tests**

Add these tests to `tests/postgres.rs`:

```rust
use heeranjid::{
    fetch_epoch, fetch_node, install_schema, seed_default_node, validate_epoch, validate_heer_node_id,
    validate_startup,
};

#[tokio::test]
async fn startup_validates_active_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();
    seed_default_node(&mut conn).await.unwrap();

    let node = validate_startup(&mut conn, 1).await.unwrap();
    assert_eq!(node.node_id, 1);
    assert_eq!(node.name, "default");
}

#[tokio::test]
async fn startup_rejects_inactive_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', false)"#,
    )
    .await
    .unwrap();

    let err = validate_startup(&mut conn, 1).await.unwrap_err();
    assert!(err.to_string().contains("not registered or not active"));
}

#[tokio::test]
async fn startup_rejects_unknown_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    let err = validate_startup(&mut conn, 99).await.unwrap_err();
    assert!(err.to_string().contains("not registered or not active"));
}

#[tokio::test]
async fn startup_rejects_missing_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    let err = validate_epoch(&mut conn).await.unwrap_err();
    assert!(err.to_string().contains("epoch is not configured"));
}

#[tokio::test]
async fn startup_validates_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, TIMESTAMP '2024-01-01 00:00:00')"#,
    )
    .await
    .unwrap();

    let epoch = validate_epoch(&mut conn).await.unwrap();
    assert_eq!(epoch.to_string(), "2024-01-01 0:00:00.0");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All tests PASS (requires DATABASE_URL to be set)

- [ ] **Step 3: Commit**

```bash
git add tests/postgres.rs
git commit -m "Add integration tests for startup validation"
```

---

### Task 5: Implement RanjId SQL generation functions

**Files:**
- Create: `sql/postgres/functions/generate_ranjid.sql`
- Modify: `sql/postgres/install.sql`

- [ ] **Step 1: Create generate_ranjid.sql**

Create `sql/postgres/functions/generate_ranjid.sql`:

```sql
-- RanjId generation functions.
-- Produces UUIDv7-compliant 128-bit identifiers with microsecond precision.
--
-- Bit layout (128 bits total):
--   Bits   0-47 : timestamp_high (48 bits)
--   Bits  48-51 : version 0111 (4 bits)
--   Bits  52-63 : timestamp_mid (12 bits)
--   Bits  64-65 : variant 10 (2 bits)
--   Bits  66-95 : timestamp_low (30 bits)
--   Bits  96-111: node_id (16 bits)
--   Bits 112-127: sequence (16 bits)
--
-- The 96-bit physical timestamp is split across three segments
-- with version and variant bits interleaved per RFC 4122 / UUIDv7.

CREATE OR REPLACE FUNCTION generate_ranjids(
    in_node_id INTEGER,
    requested_count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
)
RETURNS TABLE(id UUID)
LANGUAGE plpgsql
AS $$
DECLARE
    epoch_us NUMERIC(30,0);
    now_us NUMERIC(30,0);
    last_time NUMERIC(30,0);
    last_seq INTEGER;
    current_tick NUMERIC(30,0);
    next_seq INTEGER;
    remaining INTEGER;
    available_this_tick INTEGER;
    emit_count INTEGER;
    last_emitted_time NUMERIC(30,0);
    last_emitted_seq INTEGER;
    rollback_us NUMERIC(30,0);

    ts_high BIGINT;
    ts_mid BIGINT;
    ts_low BIGINT;
    hi BIGINT;  -- upper 64 bits
    lo BIGINT;  -- lower 64 bits
BEGIN
    IF requested_count IS NULL OR requested_count <= 0 THEN
        RAISE EXCEPTION 'requested_count must be greater than zero';
    END IF;

    IF in_node_id IS NULL OR in_node_id < 0 OR in_node_id > 65535 THEN
        RAISE EXCEPTION 'node_id % is out of range for RanjId (0..65535)', in_node_id;
    END IF;

    -- Validate node is registered and active
    IF NOT EXISTS (
        SELECT 1 FROM heer_nodes WHERE node_id = in_node_id AND is_active = true
    ) THEN
        RAISE EXCEPTION 'node_id % is not registered as an active Heer node', in_node_id;
    END IF;

    -- Read epoch as microseconds
    SELECT FLOOR(EXTRACT(EPOCH FROM c.epoch) * 1000000)::NUMERIC(30,0)
    INTO epoch_us
    FROM heer_config AS c
    WHERE c.id = 1;

    IF epoch_us IS NULL THEN
        RAISE EXCEPTION 'heer_config row id=1 must exist before generating IDs';
    END IF;

    now_us := FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000000)::NUMERIC(30,0) - epoch_us;

    -- Ensure state row exists
    INSERT INTO heer_ranj_node_state (node_id)
    VALUES (in_node_id)
    ON CONFLICT (node_id) DO NOTHING;

    -- Lock and read state
    SELECT s.last_id_time, s.last_sequence
    INTO last_time, last_seq
    FROM heer_ranj_node_state AS s
    WHERE s.node_id = in_node_id
    FOR UPDATE;

    -- Clock rollback detection
    rollback_us := last_time - now_us;
    IF rollback_us > 0 THEN
        IF rollback_us < 50000 THEN
            RAISE EXCEPTION 'clock rollback detected for ranj node % (% us)', in_node_id, rollback_us;
        END IF;
        RAISE EXCEPTION 'hard clock rollback detected for ranj node % (% us)', in_node_id, rollback_us;
    END IF;

    current_tick := GREATEST(now_us, last_time);
    next_seq := CASE
        WHEN current_tick = last_time THEN last_seq + 1
        ELSE 0
    END;

    available_this_tick := 65536 - next_seq;
    IF NOT allow_spanning AND requested_count > available_this_tick THEN
        RAISE EXCEPTION
            'requested % IDs but only % remain in microsecond % for ranj node %',
            requested_count,
            available_this_tick,
            current_tick,
            in_node_id;
    END IF;

    remaining := requested_count;

    WHILE remaining > 0 LOOP
        available_this_tick := 65536 - next_seq;
        emit_count := LEAST(remaining, available_this_tick);

        -- Decompose 90-bit timestamp into three segments around version/variant
        -- ts_high: bits 89..42 (48 bits) — goes into UUID bits 0-47
        -- ts_mid:  bits 41..30 (12 bits) — goes into UUID bits 52-63
        -- ts_low:  bits 29..0  (30 bits) — goes into UUID bits 66-95
        ts_high := (current_tick >> 42)::BIGINT & ((1::BIGINT << 48) - 1);
        ts_mid  := (current_tick >> 30)::BIGINT & ((1::BIGINT << 12) - 1);
        ts_low  := current_tick::BIGINT & ((1::BIGINT << 30) - 1);

        -- Upper 64 bits: [ts_high(48) | version(4) | ts_mid(12)]
        hi := (ts_high << 16)
            | (7::BIGINT << 12)
            | ts_mid;

        RETURN QUERY
        SELECT
            -- Compose UUID from two 64-bit halves
            -- lo: [variant(2) | ts_low(30) | node_id(16) | sequence(16)]
            encode(
                set_byte(set_byte(set_byte(set_byte(
                set_byte(set_byte(set_byte(set_byte(
                set_byte(set_byte(set_byte(set_byte(
                set_byte(set_byte(set_byte(set_byte(
                    '\x00000000000000000000000000000000'::bytea,
                    0, ((hi >> 56) & 255)::INTEGER),
                    1, ((hi >> 48) & 255)::INTEGER),
                    2, ((hi >> 40) & 255)::INTEGER),
                    3, ((hi >> 32) & 255)::INTEGER),
                    4, ((hi >> 24) & 255)::INTEGER),
                    5, ((hi >> 16) & 255)::INTEGER),
                    6, ((hi >> 8) & 255)::INTEGER),
                    7, (hi & 255)::INTEGER),
                    -- Lower 64 bits
                    8,  ((2::BIGINT << 6) | ((ts_low >> 24) & 63))::INTEGER),
                    9,  ((ts_low >> 16) & 255)::INTEGER),
                    10, ((ts_low >> 8) & 255)::INTEGER),
                    11, (ts_low & 255)::INTEGER),
                    12, ((in_node_id >> 8) & 255)::INTEGER),
                    13, (in_node_id & 255)::INTEGER),
                    14, ((seq.s >> 8) & 255)::INTEGER),
                    15, (seq.s & 255)::INTEGER),
                'hex'
            )::UUID AS id
        FROM generate_series(next_seq, next_seq + emit_count - 1) AS seq(s);

        last_emitted_time := current_tick;
        last_emitted_seq := next_seq + emit_count - 1;
        remaining := remaining - emit_count;
        current_tick := current_tick + 1;
        next_seq := 0;
    END LOOP;

    UPDATE heer_ranj_node_state
    SET last_id_time = last_emitted_time,
        last_sequence = last_emitted_seq,
        updated_at = CURRENT_TIMESTAMP
    WHERE node_id = in_node_id;
END;
$$;

-- Convenience overloads matching HeerId pattern

CREATE OR REPLACE FUNCTION generate_ranjids(
    requested_count INTEGER,
    allow_spanning BOOLEAN
)
RETURNS TABLE(id UUID)
LANGUAGE sql
AS $$
    SELECT id
    FROM generate_ranjids(current_heer_node_id(), requested_count, allow_spanning);
$$;

CREATE OR REPLACE FUNCTION generate_ranjids(requested_count INTEGER)
RETURNS TABLE(id UUID)
LANGUAGE sql
AS $$
    SELECT id
    FROM generate_ranjids(current_heer_node_id(), requested_count, true);
$$;

CREATE OR REPLACE FUNCTION generate_ranjid(in_node_id INTEGER)
RETURNS UUID
LANGUAGE sql
AS $$
    SELECT id
    FROM generate_ranjids(in_node_id, 1, true);
$$;

CREATE OR REPLACE FUNCTION generate_ranjid()
RETURNS UUID
LANGUAGE sql
AS $$
    SELECT id
    FROM generate_ranjids(current_heer_node_id(), 1, true);
$$;
```

- [ ] **Step 2: Update install.sql**

Update `sql/postgres/install.sql`:

```sql
\i postgres/schema.sql
\i postgres/functions/session.sql
\i postgres/functions/generate_heerid.sql
\i postgres/functions/generate_ranjid.sql
```

- [ ] **Step 3: Commit the SQL submodule changes**

```bash
cd sql && git add postgres/functions/generate_ranjid.sql postgres/install.sql && git commit -m "Add RanjId generation functions"
cd .. && git add sql && git commit -m "Update sql submodule with RanjId generation"
```

---

### Task 6: Wire RanjId SQL into the Rust crate

**Files:**
- Modify: `src/postgres.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add RanjId SQL constants and query helpers**

In `src/postgres.rs`, add after the existing SQL constants (after line 16, near existing constants):

```rust
pub const GENERATE_RANJID_SQL: &str = include_str!("../sql/postgres/functions/generate_ranjid.sql");
```

Update the `INSTALL_SQL` constant to include the new file:

```rust
pub const INSTALL_SQL: &str = concat!(
    include_str!("../sql/postgres/schema.sql"),
    "\n",
    include_str!("../sql/postgres/functions/session.sql"),
    "\n",
    include_str!("../sql/postgres/functions/generate_heerid.sql"),
    "\n",
    include_str!("../sql/postgres/functions/generate_ranjid.sql"),
);
```

Add query helper functions after the existing `seed_default_node` function:

```rust
pub async fn generate_heerid(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
) -> Result<crate::HeerId, sqlx::Error> {
    let raw: i64 = sqlx::query_scalar("SELECT generate_id($1)")
        .bind(i32::from(node_id))
        .fetch_one(executor)
        .await?;
    Ok(crate::HeerId::from_i64(raw).expect("database returned negative HeerId"))
}

pub async fn generate_ranjid(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
) -> Result<crate::RanjId, sqlx::Error> {
    let uuid: uuid::Uuid = sqlx::query_scalar("SELECT generate_ranjid($1)")
        .bind(i32::from(node_id))
        .fetch_one(executor)
        .await?;
    Ok(crate::RanjId::from_uuid(uuid).expect("database returned invalid RanjId UUID"))
}

pub async fn generate_heerids(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
    count: i32,
) -> Result<Vec<crate::HeerId>, sqlx::Error> {
    let rows: Vec<i64> = sqlx::query_scalar("SELECT id FROM generate_ids($1, $2)")
        .bind(i32::from(node_id))
        .bind(count)
        .fetch_all(executor)
        .await?;
    Ok(rows
        .into_iter()
        .map(|raw| crate::HeerId::from_i64(raw).expect("database returned negative HeerId"))
        .collect())
}

pub async fn generate_ranjids(
    executor: impl Executor<'_, Database = sqlx::Postgres>,
    node_id: u16,
    count: i32,
) -> Result<Vec<crate::RanjId>, sqlx::Error> {
    let rows: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM generate_ranjids($1, $2)")
        .bind(i32::from(node_id))
        .bind(count)
        .fetch_all(executor)
        .await?;
    Ok(rows
        .into_iter()
        .map(|uuid| crate::RanjId::from_uuid(uuid).expect("database returned invalid RanjId UUID"))
        .collect())
}
```

- [ ] **Step 2: Update lib.rs exports**

Update the postgres re-exports in `src/lib.rs`:

```rust
pub use postgres::{
    FETCH_ACTIVE_NODE_SQL, FETCH_EPOCH_SQL, FETCH_NODE_SQL, GENERATE_HEERID_SQL,
    GENERATE_RANJID_SQL, HeerConfig, HeerNode, INSTALL_SQL, SCHEMA_SQL, SEED_SQL, SESSION_SQL,
    fetch_active_node, fetch_epoch, fetch_node, generate_heerid, generate_heerids,
    generate_ranjid, generate_ranjids, install_schema, seed_default_node, validate_epoch,
    validate_heer_node_id, validate_startup,
};
```

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`
Expected: Compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src/postgres.rs src/lib.rs
git commit -m "Wire RanjId SQL generation into Rust crate"
```

---

### Task 7: Add RanjId integration tests

**Files:**
- Modify: `tests/postgres.rs`

- [ ] **Step 1: Add RanjId generation and validation tests**

Add these tests to `tests/postgres.rs`:

```rust
use heeranjid::RanjId;

#[tokio::test]
async fn ranjid_sql_generates_valid_uuidv7() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    let uuid: uuid::Uuid = sqlx::query_scalar("SELECT generate_ranjid($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap();

    // Verify it's a valid RanjId (version 7, RFC 4122 variant)
    let ranj = RanjId::from_uuid(uuid).unwrap();
    let parts = ranj.into_parts();
    assert!(parts.timestamp_micros > 0);
    assert_eq!(parts.node_id, 1);
}

#[tokio::test]
async fn ranjid_sql_generates_monotonic_batch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    let batch: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM generate_ranjids(10)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 10);

    // All must be valid RanjIds
    let ranj_ids: Vec<RanjId> = batch
        .iter()
        .map(|u| RanjId::from_uuid(*u).unwrap())
        .collect();

    // Must be strictly increasing (UUIDv7 bytes sort correctly)
    assert!(ranj_ids.windows(2).all(|pair| pair[0] < pair[1]));

    // All should have node_id = 1
    for r in &ranj_ids {
        assert_eq!(r.node_id(), 1);
    }
}

#[tokio::test]
async fn ranjid_sql_rejects_clock_rollback() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Set state far in the future to simulate rollback
    conn.execute(
        r#"INSERT INTO heer_ranj_node_state (node_id, last_id_time, last_sequence) VALUES (1, 999999999999999, 0)"#,
    )
    .await
    .unwrap();

    let error = sqlx::query_scalar::<_, uuid::Uuid>("SELECT generate_ranjid($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("clock rollback"));
}

#[tokio::test]
async fn ranjid_rust_helper_generates_valid_id() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    let ranj = heeranjid::generate_ranjid(&mut conn, 1).await.unwrap();
    assert_eq!(ranj.node_id(), 1);
    assert!(ranj.timestamp_micros() > 0);

    let batch = heeranjid::generate_ranjids(&mut conn, 1, 5).await.unwrap();
    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/postgres.rs
git commit -m "Add integration tests for RanjId SQL generation"
```

---

### Task 8: Add comprehensive HeerId SQL tests

**Files:**
- Modify: `tests/postgres.rs`

- [ ] **Step 1: Add HeerId edge case tests**

Add these tests to `tests/postgres.rs`:

```rust
#[tokio::test]
async fn heerid_sql_non_spanning_rejects_overflow() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    // Request more IDs than fit in one millisecond, with spanning disabled
    let err = sqlx::query_scalar::<_, i64>("SELECT id FROM generate_ids($1, $2, $3)")
        .bind(1_i32)
        .bind(8193_i32) // exceeds 8192 max per ms
        .bind(false)
        .fetch_all(&mut conn)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("requested"));
}

#[tokio::test]
async fn heerid_sql_spanning_handles_overflow() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Set state so only a few sequences remain in this tick
    conn.execute(
        r#"INSERT INTO heer_node_state (node_id, last_id_time, last_sequence)
           SELECT 1,
                  FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
                  - FLOOR(EXTRACT(EPOCH FROM (SELECT epoch FROM heer_config WHERE id = 1)) * 1000)::BIGINT,
                  8190"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    // Request 5 IDs which should span across millisecond boundary
    let batch: Vec<i64> = sqlx::query_scalar("SELECT id FROM generate_ids(5)")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
}

#[tokio::test]
async fn heerid_sql_rejects_missing_epoch() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();

    // No epoch inserted — should fail
    let err = sqlx::query_scalar::<_, i64>("SELECT generate_id($1)")
        .bind(1_i32)
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("heer_config"));
}

#[tokio::test]
async fn heerid_sql_rejects_missing_session_node() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    // Don't set session node — call generate_id() without node arg
    let err = sqlx::query_scalar::<_, i64>("SELECT generate_id()")
        .fetch_one(&mut conn)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("node_id"));
}

#[tokio::test]
async fn heerid_rust_helper_generates_valid_id() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    let heer = heeranjid::generate_heerid(&mut conn, 1).await.unwrap();
    assert_eq!(heer.node_id(), 1);
    assert!(heer.timestamp_ms() > 0);

    let batch = heeranjid::generate_heerids(&mut conn, 1, 5).await.unwrap();
    assert_eq!(batch.len(), 5);
    assert!(batch.windows(2).all(|pair| pair[0] < pair[1]));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/postgres.rs
git commit -m "Add comprehensive HeerId SQL edge case tests"
```

---

### Task 9: Add From/TryFrom conversions and convenience methods

**Files:**
- Modify: `src/heer.rs`
- Modify: `src/ranj.rs`

- [ ] **Step 1: Add conversions to HeerId**

Add to `src/heer.rs` after the `FromStr` impl (after line 114):

```rust
impl From<HeerId> for i64 {
    fn from(id: HeerId) -> Self {
        id.0
    }
}

impl TryFrom<i64> for HeerId {
    type Error = Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::from_i64(value)
    }
}
```

- [ ] **Step 2: Add conversions to RanjId**

Add to `src/ranj.rs` after the `FromStr` impl (after line 120):

```rust
impl From<RanjId> for Uuid {
    fn from(id: RanjId) -> Self {
        id.0
    }
}

impl TryFrom<Uuid> for RanjId {
    type Error = Error;

    fn try_from(uuid: Uuid) -> Result<Self, Self::Error> {
        Self::from_uuid(uuid)
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/heer.rs src/ranj.rs
git commit -m "Add From and TryFrom conversions for HeerId and RanjId"
```

---

### Task 10: Add SQL ordering validation tests

**Files:**
- Modify: `tests/postgres.rs`

- [ ] **Step 1: Add SQL ORDER BY validation tests**

Add these tests to `tests/postgres.rs`:

```rust
#[tokio::test]
async fn heerid_sql_order_by_matches_generation_order() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    // Insert IDs into a temp table and verify ORDER BY matches insertion order
    conn.execute(
        r#"CREATE TEMP TABLE test_ids (pos SERIAL, hid BIGINT NOT NULL)"#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"INSERT INTO test_ids (hid) SELECT id FROM generate_ids(20)"#,
    )
    .await
    .unwrap();

    let ordered: Vec<(i32, i64)> =
        sqlx::query_as("SELECT pos, hid FROM test_ids ORDER BY hid ASC")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    // Position order should match value order
    for (i, (pos, _)) in ordered.iter().enumerate() {
        assert_eq!(*pos as usize, i + 1);
    }
}

#[tokio::test]
async fn ranjid_sql_order_by_matches_generation_order() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    conn.execute(
        r#"CREATE TEMP TABLE test_rids (pos SERIAL, rid UUID NOT NULL)"#,
    )
    .await
    .unwrap();

    conn.execute(
        r#"INSERT INTO test_rids (rid) SELECT id FROM generate_ranjids(20)"#,
    )
    .await
    .unwrap();

    let ordered: Vec<(i32, uuid::Uuid)> =
        sqlx::query_as("SELECT pos, rid FROM test_rids ORDER BY rid ASC")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    for (i, (pos, _)) in ordered.iter().enumerate() {
        assert_eq!(*pos as usize, i + 1);
    }
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/postgres.rs
git commit -m "Add SQL ORDER BY validation tests for HeerId and RanjId"
```

---

### Task 11: Add schema install idempotency test

**Files:**
- Modify: `tests/postgres.rs`

- [ ] **Step 1: Add idempotency test**

Add this test to `tests/postgres.rs`:

```rust
#[tokio::test]
async fn schema_install_is_idempotent() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    // Install twice — second call must not fail
    install_schema(&mut conn).await.unwrap();
    install_schema(&mut conn).await.unwrap();

    // Verify tables exist by inserting data
    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();

    let node = fetch_node(&mut conn, 1).await.unwrap().unwrap();
    assert_eq!(node.name, "default");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/postgres.rs
git commit -m "Add schema install idempotency test"
```

---

### Task 12: Add column default integration test

**Files:**
- Modify: `tests/postgres.rs`

- [ ] **Step 1: Add column default test**

Add this test to `tests/postgres.rs`:

```rust
#[tokio::test]
async fn heerid_works_as_column_default() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    // Create a table that uses generate_id() as a column default
    conn.execute(
        r#"CREATE TABLE test_entities (
            id BIGINT PRIMARY KEY DEFAULT generate_id(),
            label TEXT NOT NULL
        )"#,
    )
    .await
    .unwrap();

    conn.execute(r#"INSERT INTO test_entities (label) VALUES ('alpha')"#)
        .await
        .unwrap();
    conn.execute(r#"INSERT INTO test_entities (label) VALUES ('bravo')"#)
        .await
        .unwrap();

    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, label FROM test_entities ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    assert_eq!(rows.len(), 2);
    assert!(rows[0].0 > 0);
    assert!(rows[0].0 < rows[1].0);
}

#[tokio::test]
async fn ranjid_works_as_column_default() {
    let mut conn = match connect_test_db().await {
        Some(conn) => conn,
        None => return,
    };

    let schema = test_schema_name();
    conn.execute(format!(r#"CREATE SCHEMA "{schema}""#).as_str())
        .await
        .unwrap();
    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
        .await
        .unwrap();

    install_schema(&mut conn).await.unwrap();

    conn.execute(
        r#"INSERT INTO heer_nodes (node_id, name, is_active) VALUES (1, 'default', true)"#,
    )
    .await
    .unwrap();
    conn.execute(
        r#"INSERT INTO heer_config (id, epoch) VALUES (1, CURRENT_TIMESTAMP - INTERVAL '1 day')"#,
    )
    .await
    .unwrap();

    sqlx::query("SELECT set_heer_node_id($1)")
        .bind(1_i32)
        .execute(&mut conn)
        .await
        .unwrap();

    conn.execute(
        r#"CREATE TABLE test_events (
            id UUID PRIMARY KEY DEFAULT generate_ranjid(),
            label TEXT NOT NULL
        )"#,
    )
    .await
    .unwrap();

    conn.execute(r#"INSERT INTO test_events (label) VALUES ('alpha')"#)
        .await
        .unwrap();
    conn.execute(r#"INSERT INTO test_events (label) VALUES ('bravo')"#)
        .await
        .unwrap();

    let rows: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, label FROM test_events ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .unwrap();

    assert_eq!(rows.len(), 2);
    // Both must be valid RanjIds
    RanjId::from_uuid(rows[0].0).unwrap();
    RanjId::from_uuid(rows[1].0).unwrap();
    assert!(rows[0].0 < rows[1].0);
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/postgres.rs
git commit -m "Add column default integration tests for HeerId and RanjId"
```

---

### Task 13: Update SQL README

**Files:**
- Modify: `sql/README.md`

- [ ] **Step 1: Add RanjId generation API documentation**

Add a new section to `sql/README.md` after the existing Generation API section (after the bulk behavior section). Add this content after the existing `generate_ids` documentation:

```markdown
### RanjId Generation API

```sql
generate_ranjid() RETURNS UUID;
generate_ranjid(node_id INTEGER) RETURNS UUID;
```

For bulk allocation:

```sql
generate_ranjids(count INTEGER) RETURNS TABLE(id UUID);

generate_ranjids(
    count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
) RETURNS TABLE(id UUID);

generate_ranjids(
    node_id INTEGER,
    count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
) RETURNS TABLE(id UUID);
```

### RanjId Bulk Behavior

- returns exactly `count` UUIDs
- strictly increasing within a batch (UUIDv7 byte ordering)
- fully concurrency-safe
- uses a read-once, compute, write-once state update
- performs exactly one update to `heer_ranj_node_state` for the full batch
- may span multiple microseconds when needed
- clock rollback detection threshold is 50,000 microseconds (50ms)
```

Also add to the Column Defaults section:

```markdown
For RanjId:

```sql
id UUID PRIMARY KEY DEFAULT generate_ranjid();
```
```

Also add to the File Structure section a note about the new file:

```markdown
## File Structure

```
postgres/
├── schema.sql                      -- table definitions
├── seed.sql                        -- default single-node seed data
├── install.sql                     -- psql install entrypoint
├── functions/
│   ├── session.sql                 -- set/get session node
│   ├── generate_heerid.sql         -- HeerId generation
│   └── generate_ranjid.sql         -- RanjId generation
└── queries/
    ├── fetch_node.sql              -- node lookup
    ├── fetch_epoch.sql             -- epoch lookup
    └── fetch_active_node.sql       -- active node lookup
```
```

- [ ] **Step 2: Commit**

```bash
cd sql && git add README.md && git commit -m "Update README with RanjId generation API and file structure"
cd .. && git add sql && git commit -m "Update sql submodule README"
```

---

### Task 14: Final verification pass

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All unit tests PASS

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test postgres`
Expected: All integration tests PASS (requires DATABASE_URL)

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 4: Verify clean compile**

Run: `cargo build`
Expected: Clean build with no warnings
