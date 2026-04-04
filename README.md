# HeerId Specification v1.0

## 1. Overview

**HeerId** and **RanjId** are time-ordered identifiers sinpired by Snowflake-style IDs and designed for framework use. Collectively known as the **HeerRanjId** suite, they provide:

- **HeerId (64-bit):** Default primary key for standard entities (Postgres `BIGINT`).
- **RanjId (128-bit):** High-precision key for event streams and logs (Postgres `UUID`).
- **Deterministic Sortability:** Database-native ordering for both variants.
- **High write throughput:** 8196 and 65536 rows per node - most likely never your bottleneck
- **Distributed System Compatibility:** Zero migration path from single-node to multi-node systems*
- **Cross-Stack Compatibility:** Seamless use in **Rust (Axum)**, **Python (Django)**, **JS (Prisma)** and **C# (.NET)**.


---

## 2. Design Goals

HeerId is designed to:

- feel simple in single-node applications
- scale without schema changes
- avoid centralized ID services
- remain database-native
- be portable across database backends
- support long system lifetimes

---

## 3. Core Principles

- Single-node by default  
- Distributed-ready by design  
- Fail fast on misconfiguration  
- Database enforces correctness  
- Environment defines node identity  
- No runtime coordination required  
- Stable format over time  
- Database-native ergonomics (works in raw SQL)

---

## 4. ID Format

HeerId is a signed 64-bit integer using **63 usable bits**.

| 41-bit timestamp | 9-bit node_id | 13-bit sequence |

### RanjId (128-bit / UUID)
RanjId uses a 128-bit block structured for UUIDv7 RFC 4122 compliance. While it features a physical 96-16-16 split, 6 bits are reserved for the UUID version and variant, resulting in a **6-90-16-16** effective payload.

| Bit Range | Length | Content | Note |
| :--- | :--- | :--- | :--- |
| 0 - 47 | 48 bits | Timestamp (High) | Part 1 of 96-bit $\mu s$ timestamp |
| 48 - 51 | 4 bits | **Version (0111)** | UUIDv7 Marker |
| 52 - 63 | 12 bits | Timestamp (Mid) | Part 2 of 96-bit $\mu s$ timestamp |
| 64 - 65 | 2 bits | **Variant (10)** | RFC 4122 Marker |
| 66 - 95 | 30 bits | Timestamp (Low) | Part 3 of 96-bit $\mu s$ timestamp |
| 96 - 111 | 16 bits | **Node ID** | Supports 65,536 Nodes |
| 112 - 127 | 16 bits | **Sequence** | Supports 65,536 IDs/μs |

> [!CAUTION]
> **Frontend Safety:** Because 2^63-1 exceeds JavaScript's `Number.MAX_SAFE_INTEGER` (2^53-1), all HeerIds **MUST** be serialized as **Strings** in JSON responses to prevent truncation.



### Bit Layout

| 41-bit timestamp | 9-bit node_id | 13-bit sequence |

---

## 5. Field Definitions

### Timestamp (41 bits)
Milliseconds since custom epoch (~69.7 years range).

### Node ID (9 bits)
- Up to 512 nodes.
- **Static:** Assigned to long-lived infrastructure.
- **Dynamic:** Ephemeral nodes should "lease" and "release" IDs from `heer_nodes` to allow recycling.

### Sequence (13 bits)
- Up to 8192 IDs per ms per node.


### Epoch

The epoch is not defined by the HeerId crate.

Each implementation chooses its own epoch via the `heer_config` table.

---

## 6. Heer Tables

```sql
CREATE TABLE heer_nodes (
    node_id       INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    description   TEXT,
    is_active     BOOLEAN DEFAULT true, -- Supports node recycling
    created_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_accessed TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE heer_config (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    epoch       TIMESTAMP NOT NULL,
    updated_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE heer_node_state (
    node_id         INTEGER PRIMARY KEY
                    REFERENCES heer_nodes(node_id) ON DELETE CASCADE,
    last_id_time    BIGINT NOT NULL DEFAULT 0,
    last_sequence   SMALLINT NOT NULL DEFAULT 0,
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
COMMENT ON TABLE heer_node_state IS
'Internal state for HeerId generator (one row per node). Do not modify manually.';
-- New RanjId state (128-bit)
CREATE TABLE heer_ranj_node_state (
    node_id         INTEGER PRIMARY KEY REFERENCES heer_nodes(node_id),
    last_id_time    NUMERIC(30,0) NOT NULL DEFAULT 0, -- Supports 96-bit microsecond integers
    last_sequence   INTEGER NOT NULL DEFAULT 0,       -- Supports 16-bit sequence
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```


### Default Values

node_id = 1  
name = "default"

---

## 7. Node Identity

Defined via environment:

NODE_ID=1

Must:

- exist  
- be valid  
- be unique per writer  
- exist in `heer_nodes`  

---

## 8. Startup Validation

On boot:

- read NODE_ID
- validate
- ensure it exists in `heer_nodes`
- print node info
- fail if invalid

---

## 9. Session-Based Node Configuration

Set node for session:

```sql
set_heer_node_id(node_id INTEGER);
```

Read node:

```sql
current_heer_node_id() RETURNS INTEGER;
```

---

## 10. ID Generation

```sql
generate_id() RETURNS BIGINT;
generate_id(node_id INTEGER) RETURNS BIGINT;
```

---

## 11. Bulk ID Allocation

```sql
generate_ids(count INTEGER) RETURNS TABLE(id BIGINT);

generate_ids(
    count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
) RETURNS TABLE(id BIGINT);

generate_ids(
    node_id INTEGER,
    count INTEGER,
    allow_spanning BOOLEAN DEFAULT true
) RETURNS TABLE(id BIGINT);
```

### Behavior

- Returns exactly count IDs  
- Strictly increasing within batch  
- Fully concurrency-safe  
- Uses read-once, compute, write-once state update  

---

## 12. Column Defaults

```sql
id BIGINT PRIMARY KEY DEFAULT generate_id();
```

---

## 13. Guarantees

### Provided
- **HeerId:** Millisecond precision; K-sortable by node.
- **RanjId:** Microsecond precision; Strictly sortable by Time -> Node -> Sequence.
- **Collision Resistance:** Deterministic uniqueness across 65,536 nodes for RanjId.
- **Storage:** RanjId is fully compliant with `UUID` parsers in **Django** and **.NET**.

### Not Provided
- **Anonymity:** Both IDs leak the creation time and Node ID by design.
- **strict global ordering**
- **exact timestamp equality**

---

## 14. Scaling

framework nodes add 2 --name="region-a"  

NODE_ID=2  

No schema changes required.

---

## 15. Clock and Sequence Edge Case Handling

### Clock Rollback
- **Minor Drift (< 50ms):** Throw an error; the application layer (e.g., the HeerId Crate) should handle the retry.
- **Major Drift (> 50ms):** Fail fast with a hard error.
- **Constraint:** Do NOT use `sleep()` or `stall` commands inside the database to prevent connection pool exhaustion.

### Sequence Overflow
- Advance timestamp (+1ms).
- Reset sequence to 0.

### Bulk Generation
- Must perform exactly ONE update to `heer_node_state` for the entire batch.
- may span multiple milliseconds  
- lock held only for read + write  

### Clock Skew Between Nodes

- expected  
- slight ordering differences possible  

Use created_at for strict ordering.

---

## 16. Failure Modes

| Issue | Cause | Mitigation |
|------|------|-----------|
| ID collision | duplicate node_id | enforce registry |
| startup failure | invalid NODE_ID | fail fast |
| ordering drift | clock skew | expected |
| generator stall | overflow / rollback | brief blocking |
| clock rollback error | severe drift | error + NTP |
| session missing | node not set | runtime error |

---

## 17. Best Practices

- Always store `created_at` alongside the ID for auditing.
- treat node_id as infrastructure  
- Use `generate_ids()` for all bulk insert operations to minimize DB contention.
- - **Serialization:** Always serialize both HeerId and RanjId as **Strings** in JSON for frontend consumers (**HTMX**, SPAs) to prevent precision loss.
- **Infrastructure:** Pinned `NODE_ID` environment variables are preferred for core services.
- **NTP:** Use NTP in **slew mode** to ensure the clock is adjusted gradually without jumps.
- **Storage:** Use `BIGINT` for HeerId and native `UUID` for RanjId in PostgreSQL for optimal indexing.
- **Node Management:** Use the `heer_nodes` registry to prevent `NODE_ID` collisions.
- set session node per connection  
- avoid strict ordering reliance  

---

## 18. HeerId Crate

Provides:

- HeerId type  
- encoding/decoding  
- SQL generation  
- backend modules  

Supports:

- postgres  
- mssql  
- future extensions  

---

## 19. Extensibility

Backends can be added:

- oracle  
- cockroach  
- sqlite  

---

## 20. Philosophy

HeerId is a database-native ID system that scales without introducing complexity early.

---

## 21. Long-Term

- ~69 year lifespan  
- future migration → UUIDv7, Heer128, or composite key (multiple epoches).     

---

## 22. Summary

- simple defaults  
- raw SQL compatibility  
- high throughput  
- safe concurrency  
- scalable architecture  

| Feature | HeerId | RanjId |
| :--- | :--- | :--- |
| **Bit Width** | 64-bit | 128-bit |
| **Postgres Type** | `BIGINT` | `UUID` |
| **Precision** | Millisecond ($ms$) | Microsecond ($\mu s$) |
| **Timestamp Bits** | 41 bits | 96 bits (90 effective) |
| **Node ID Bits** | 9 bits (512) | 16 bits (65,536) |
| **Sequence Bits** | 13 bits (8,192/ms) | 16 bits (65,536/μs) |
| **Max Lifespan** | ~69 Years | ~2.5 Trillion Years |

---

## Rust Crate Usage

### Installation

```toml
[dependencies]
heeranjid = { path = "." }
```

### Quick Start

```rust
use heeranjid::{
    install_schema, seed_default_node, validate_startup, validate_epoch,
    generate_heerid, generate_ranjid, generate_heerids, generate_ranjids,
    HeerId, RanjId,
};
use sqlx::PgConnection;

// 1. Install schema (idempotent)
install_schema(&mut conn).await?;

// 2. Seed default node (idempotent, inserts node_id=1)
seed_default_node(&mut conn).await?;

// 3. Validate on startup
let node = validate_startup(&mut conn, 1).await?;
let epoch = validate_epoch(&mut conn).await?;
println!("Node {} ({}) ready, epoch: {}", node.node_id, node.name, epoch);

// 4. Generate IDs
let heer: HeerId = generate_heerid(&mut conn, 1).await?;
let ranj: RanjId = generate_ranjid(&mut conn, 1).await?;

// 5. Generate batches
let heer_batch: Vec<HeerId> = generate_heerids(&mut conn, 1, 100).await?;
let ranj_batch: Vec<RanjId> = generate_ranjids(&mut conn, 1, 100).await?;
```

### Session-Based Generation

For connection-pooled applications, set the node once per connection:

```sql
-- HeerId sessions (node_id 0-511)
SELECT set_heer_node_id(1);
SELECT generate_id();
SELECT id FROM generate_ids(10);

-- RanjId sessions (node_id 0-65535)
SELECT set_heer_ranj_node_id(1);
SELECT generate_ranjid();
SELECT id FROM generate_ranjids(10);
```

### Column Defaults

```sql
-- HeerId as primary key
CREATE TABLE users (
    id BIGINT PRIMARY KEY DEFAULT generate_id(),
    name TEXT NOT NULL
);

-- RanjId as primary key
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT generate_ranjid(),
    payload JSONB NOT NULL
);
```

### ID Inspection

```rust
// HeerId parts
let parts = heer.into_parts();
println!("time={}ms node={} seq={}", parts.timestamp_ms, parts.node_id, parts.sequence);

// RanjId parts
let parts = ranj.into_parts();
println!("time={}us node={} seq={}", parts.timestamp_micros, parts.node_id, parts.sequence);
```

### JSON Serialization

Both types serialize as strings to prevent JavaScript precision loss:

```json
{ "heer_id": "1234567890123456", "ranj_id": "0192d4e0-7b3a-7f00-8001-000100000001" }
```

Deserialization accepts both strings and integers for HeerId.

### Postgres Bootstrap

```bash
# Start a local Postgres instance
./scripts/postgres.sh up

# Set DATABASE_URL and run tests
export DATABASE_URL=$(./scripts/postgres.sh url)
cargo test

# Lint and check
./scripts/check.sh
```

### Extended Epochs (Big Bang)

RanjId supports epochs beyond PostgreSQL's TIMESTAMP range via `ranj_epoch_offset`:

```sql
INSERT INTO heer_config (id, epoch, ranj_epoch_offset)
VALUES (
    1,
    TIMESTAMP '1970-01-01 00:00:00',
    FLOOR(13.787e9 * 365.25 * 86400 * 1e6)::NUMERIC(30,0)
);
```

This encodes microseconds since the Big Bang (~4.35 x 10^23), well within the 90-bit timestamp range (~1.24 x 10^27).

