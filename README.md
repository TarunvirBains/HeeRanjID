# HeeRanjId Specification v2.0

## 1. Overview

**HeerId** and **RanjId** are time-ordered, deterministic identifiers designed to avoid the randomness of UUID while providing database-native sortability and distributed uniqueness. Collectively known as the **HeeRanjId** suite, they provide:

- **HeerId (64-bit):** A Snowflake-like ID modified for immediate implementation in non-distributed cases. Default primary key for standard entities (Postgres `BIGINT`). No coordination service needed — works out of the box with a single node and scales to 512 nodes without schema changes.
- **RanjId (128-bit, UUIDv8):** High-precision key with self-describing timestamp precision — from microseconds for web apps to femtoseconds for particle physics and scientific instrumentation. Stored as a standard `UUID` in any database.
- **Configurable Precision:** RanjId supports microsecond, nanosecond, picosecond, and femtosecond timestamp precision, encoded directly in each ID. No external configuration needed to interpret an ID's timestamp.
- **Deterministic Sortability:** Database-native ordering for both variants. No random bits — every bit is meaningful.
- **High write throughput:** 8,192 IDs/ms/node (HeerId) and 65,536 IDs/tick/node (RanjId) — most likely never your bottleneck.
- **Physics use cases:** RanjId's femtosecond precision and 89-bit timestamp support timestamping events in particle physics experiments, laser instrumentation, and high-energy physics — anywhere sub-microsecond precision matters.
- **Distributed System Compatibility:** Zero migration path from single-node to multi-node systems.
- **Cross-Stack Compatibility:** Seamless use in **Rust**, **Python (Django)**, **TypeScript (Prisma)**, and **C# (.NET)**.


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

### RanjId (128-bit / UUIDv8)
RanjId uses a 128-bit block structured as UUIDv8 (RFC 9562) — the designated format for custom UUID layouts. The 2-bit precision field makes every RanjId self-describing: you can determine the timestamp's unit (μs, ns, ps, fs) by inspecting the ID itself.

| Bit Range | Length | Content | Note |
| :--- | :--- | :--- | :--- |
| 0 - 47 | 48 bits | Timestamp (High) | Part 1 of 89-bit timestamp |
| 48 - 51 | 4 bits | **Version (1000)** | UUIDv8 Marker |
| 52 - 63 | 12 bits | Timestamp (Mid) | Part 2 of 89-bit timestamp |
| 64 - 65 | 2 bits | **Variant (10)** | RFC 4122 Marker |
| 66 - 67 | 2 bits | **Precision** | `00`=μs, `01`=ns, `10`=ps, `11`=fs |
| 68 - 96 | 29 bits | Timestamp (Low) | Part 3 of 89-bit timestamp |
| 97 - 111 | 15 bits | **Node ID** | Supports 32,768 Nodes |
| 112 - 127 | 16 bits | **Sequence** | Supports 65,536 IDs/tick |

### Precision Levels

| Setting | Unit | 89-bit Range | Use Case |
| :--- | :--- | :--- | :--- |
| `us` | Microseconds (10⁻⁶ s) | ~19.6 trillion years | Web apps, databases (default for SQL generation) |
| `ns` | Nanoseconds (10⁻⁹ s) | ~19.6 billion years | High-frequency trading, real-time systems |
| `ps` | Picoseconds (10⁻¹² s) | ~19.6 million years | Telecom, instrumentation |
| `fs` | Femtoseconds (10⁻¹⁵ s) | ~19,620 years | Particle physics, laser experiments |

Set via environment variable: `RANJID_PRECISION=ns` (default: `ns` for application-level generation).

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

The default epoch is `2026-01-01T00:00:00Z`. Each deployment can override this via the `heer_config` table. The epoch determines the zero-point for all timestamps — HeerId's 41-bit millisecond counter and RanjId's 89-bit precision-dependent counter both measure time since this epoch.

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
- **RanjId:** Configurable precision (μs/ns/ps/fs); Strictly sortable by Time → Node → Sequence within the same precision.
- **Collision Resistance:** Deterministic uniqueness across 32,768 nodes for RanjId, 512 nodes for HeerId.
- **Storage:** RanjId is fully compliant with `UUID` parsers in **Django**, **.NET**, **Postgres**, and **MSSQL**. UUIDv8 is accepted everywhere UUIDs are stored.
- **Self-Describing:** Each RanjId encodes its own precision — no external configuration needed to interpret timestamps.

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

- HeerId: ~69 year lifespan from epoch
- RanjId: ~19,620 years (femtoseconds) to ~19.6 trillion years (microseconds)
- Built-in batch conversion: `HeerId::batch_to_ranjids()` for upgrading from 64-bit to 128-bit IDs
- Reverse conversion with automatic timestamp squashing detection

---

## 22. Summary

- simple defaults  
- raw SQL compatibility  
- high throughput  
- safe concurrency  
- scalable architecture  

| Feature | HeerId | RanjId |
| :--- | :--- | :--- |
| **Bit Width** | 64-bit | 128-bit (UUIDv8) |
| **Postgres Type** | `BIGINT` | `UUID` |
| **Precision** | Millisecond | Configurable: μs / ns / ps / fs |
| **Timestamp Bits** | 41 bits | 89 bits |
| **Precision Bits** | — | 2 bits (self-describing) |
| **Node ID Bits** | 9 bits (512) | 15 bits (32,768) |
| **Sequence Bits** | 13 bits (8,192/ms) | 16 bits (65,536/tick) |
| **Max Lifespan** | ~69 years | ~19.6T years (μs) to ~19,620 years (fs) |
| **Default Epoch** | `2026-01-01` | `2026-01-01` |

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

-- RanjId sessions (node_id 0-32767)
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
println!("time={}{} node={} seq={}", parts.timestamp, parts.precision.label(), parts.node_id, parts.sequence);
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

RanjId supports epochs beyond PostgreSQL's TIMESTAMP range via `ranj_epoch_offset`. Combined with femtosecond precision, this enables timestamping events relative to cosmological timescales:

```sql
INSERT INTO heer_config (id, epoch, ranj_epoch_offset)
VALUES (
    1,
    TIMESTAMP '1970-01-01 00:00:00',
    FLOOR(13.787e9 * 365.25 * 86400 * 1e6)::NUMERIC(30,0)
);
```

With the default epoch of `2026-01-01`, the 89-bit timestamp provides ~19,620 years of femtosecond-precision range — sufficient for any modern scientific application.

