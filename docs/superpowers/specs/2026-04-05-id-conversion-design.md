# HeerId ↔ RanjId Conversion + RanjId v8 Precision Design

## Goal

1. Switch RanjId from UUIDv7 to UUIDv8 (custom format, honestly labeled)
2. Make RanjId timestamp precision configurable (microseconds, nanoseconds, picoseconds, femtoseconds)
3. Add batch conversion functions between HeerId and RanjId

## RanjId → UUIDv8 with Self-Describing Precision

RanjId uses a custom bit layout that doesn't conform to UUIDv7's requirement of a 48-bit millisecond Unix timestamp in the high bits. The version nibble changes from `0111` (v7) to `1000` (v8), which is RFC 9562's designated catch-all for custom UUID formats.

Additionally, 2 bits are repurposed (1 from timestamp, 1 from node_id) to encode the timestamp precision directly in the UUID. This makes every RanjId self-describing — no external configuration needed to interpret it.

**What changes from current layout:**
- Version nibble: `0111` (v7) → `1000` (v8)
- Timestamp: 90 bits → 89 bits (1 bit given to precision field)
- Node ID: 16 bits → 15 bits (1 bit given to precision field)
- New 2-bit precision field: `00`=μs, `01`=ns, `10`=ps, `11`=fs
- `RANJ_UUID_VERSION` constant: `0b0111` → `0b1000`

**What stays the same:**
- 128-bit value stored as UUID
- Variant bits `10` (RFC 4122)
- Sort order preserved within same precision
- Postgres `uuid`, MSSQL `UNIQUEIDENTIFIER`, Python `uuid.UUID`, Rust `uuid::Uuid` — all accept v8 without issue
- 16-bit sequence unchanged

This is not a breaking change — nothing is deployed.

## Configurable Timestamp Precision

The 90-bit timestamp field can represent different precisions:

| Precision | Bits | 89-bit range | Use case |
|-----------|------|-------------|----------|
| Microseconds (μs) | `00` | ~19.6 trillion years | Web apps, databases (default) |
| Nanoseconds (ns) | `01` | ~19.6 billion years | High-frequency systems |
| Picoseconds (ps) | `10` | ~19.6 million years | Instrumentation, telecom |
| Femtoseconds (fs) | `11` | ~19,620 years | Particle physics, laser experiments |

The precision is encoded in the UUID itself (2-bit field), making every RanjId self-describing. No external configuration needed to interpret an ID's timestamp.

For ID **generation**, the precision is set once at process startup via environment variable `RANJID_PRECISION`, cached as a static. Default: `us` (microseconds).

```
RANJID_PRECISION=us   # microseconds (default)
RANJID_PRECISION=ns   # nanoseconds
RANJID_PRECISION=ps   # picoseconds
RANJID_PRECISION=fs   # femtoseconds
```

**In Rust:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RanjPrecision {
    Microseconds = 0b00,
    Nanoseconds  = 0b01,
    Picoseconds  = 0b10,
    Femtoseconds = 0b11,
}

impl RanjPrecision {
    /// Multiplier to convert from microseconds to this precision's unit
    pub fn from_micros_multiplier(&self) -> u128 {
        match self {
            Self::Microseconds => 1,
            Self::Nanoseconds => 1_000,
            Self::Picoseconds => 1_000_000,
            Self::Femtoseconds => 1_000_000_000,
        }
    }
}
```

The precision for **generation** is read from `RANJID_PRECISION` at first use and cached in a `OnceLock<RanjPrecision>`. The precision for **decoding** is read from the 2-bit field in the UUID itself — so `into_parts()` always returns the correct unit regardless of what the current process is configured to generate.

**Cross-precision sort order:** Within the same precision, sort order is correct (higher timestamp = higher UUID). Across precisions, IDs may not sort chronologically since the precision bits sit between variant and timestamp-low bits. A deployment should use one precision consistently per table.

**SQL functions:** Postgres `clock_timestamp()` provides microsecond precision. MSSQL `SYSUTCDATETIME()` provides 100-nanosecond precision. For picosecond/femtosecond generation, IDs must be generated in application code (Rust), not SQL. SQL-based generation supports microseconds and nanoseconds only.

## Problem: HeerId ↔ RanjId Conversion

A system that started with HeerId (64-bit, millisecond precision) may need to upgrade to RanjId (128-bit, configurable precision). The reverse may also be needed.

### Timestamp Squashing (RanjId → HeerId)

RanjId has finer precision than HeerId's milliseconds. When converting RanjId → HeerId, multiple RanjIds with different timestamps may map to the same millisecond. If they share a `node_id`, sequences must be reassigned to avoid collisions. This makes RanjId → HeerId conversion inherently a **batch operation**.

## Bit Layouts

```
HeerId (i64, 63 usable bits):
  [41-bit timestamp_ms][9-bit node_id][13-bit sequence]
  Max timestamp: 2^41 - 1 = 2,199,023,255,551 ms (~69 years)
  Max node_id: 511
  Max sequence: 8,191

RanjId (u128 as UUIDv8):
  [48-bit ts_high][4-bit version=1000][12-bit ts_mid][2-bit variant=10][2-bit precision][29-bit ts_low][15-bit node_id][16-bit sequence]
  Timestamp: ts_high(48) | ts_mid(12) | ts_low(29) = 89 bits in self-described precision
  Precision: 00=μs, 01=ns, 10=ps, 11=fs (encoded in UUID, self-describing)
  Max node_id: 32,767
  Max sequence: 65,535
```

## Conversion: HeerId → RanjId (Batch)

Always succeeds. Every HeerId value fits in a RanjId.

**Mapping per ID (precision-aware):**
- `timestamp = timestamp_ms * precision.divisor() / 1000` — converts ms to the target precision. For microseconds: `* 1000`. For nanoseconds: `* 1_000_000`. For femtoseconds: `* 1_000_000_000_000`.
- `node_id` — direct copy, zero-extended (9-bit → 15-bit, always fits)
- `sequence` — direct copy, zero-extended (13-bit → 16-bit)

```rust
impl HeerId {
    pub fn check_ranjid_convertibility(ids: &[HeerId]) -> Vec<ConversionConflict> {
        Vec::new()  // always empty — HeerId always fits in RanjId
    }

    pub fn batch_to_ranjids(ids: &[HeerId]) -> Vec<(HeerId, RanjId)> {
        let precision = RanjPrecision::current();
        let factor = precision.from_micros_multiplier() * 1000; // ms → target precision
        ids.iter()
            .map(|hid| {
                let parts = hid.into_parts();
                let rid = RanjId::new(
                    u128::from(parts.timestamp_ms) * u128::from(factor),
                    parts.node_id,
                    parts.sequence,
                ).expect("HeerId always fits in RanjId");
                (*hid, rid)
            })
            .collect()
    }
}
```

## Conversion: RanjId → HeerId (Batch)

Can fail. Requires batch-level analysis to handle timestamp squashing.

**Algorithm:**

1. For each RanjId, compute candidate HeerId parts:
   - `timestamp_ms = timestamp / (precision.divisor() / 1000)` — target precision → ms
   - `node_id` — unchanged (must be ≤ 511)

2. Check per-ID hard failures:
   - `node_id > 511` → `NodeIdOverflow` (RanjId allows 15-bit / max 32,767; HeerId allows 9-bit / max 511)
   - `timestamp_ms > 2^41 - 1` → `TimestampOverflow`

3. Group by `(timestamp_ms, node_id)`

4. Within each group, sort by original RanjId (preserves ordering), assign sequences `0, 1, 2, ...`

5. If any group has more than 8192 members → `SequenceOverflow`

6. Return `Vec<(RanjId, HeerId)>` mapping old → new

```rust
impl RanjId {
    pub fn check_heerid_convertibility(ids: &[RanjId]) -> Vec<ConversionConflict> {
        // Group by (timestamp_ms, node_id)
        // Check node_id overflow, timestamp overflow, group sizes > 8192
    }

    pub fn batch_to_heerids(ids: &[RanjId]) -> Result<Vec<(RanjId, HeerId)>, ConversionError> {
        // 1. Check hard failures
        // 2. Group by (timestamp_ms, node_id)
        // 3. Assign sequences within each group
        // 4. Build (old, new) pairs
    }
}
```

Both directions follow the same pattern: `check_*_convertibility` → `batch_to_*`. Framework migration tools always call check first, then convert.

## Pre-flight Convertibility Check

```rust
#[derive(Debug)]
pub struct ConversionConflict {
    pub kind: ConflictKind,
    pub ranj_ids: Vec<RanjId>,
}

#[derive(Debug)]
pub enum ConflictKind {
    NodeIdOverflow { node_id: u16 },
    TimestampOverflow { timestamp_ms: u64 },
    SequenceOverflow { timestamp_ms: u64, node_id: u16, count: usize, max: usize },
}
```

## Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("timestamp {value} exceeds HeerId max ({max} ms)")]
    TimestampOverflow { value: u128, max: u64 },

    #[error("node_id {value} exceeds HeerId max ({max})")]
    NodeIdOverflow { value: u16, max: u16 },

    #[error("{count} IDs share (timestamp_ms={timestamp_ms}, node_id={node_id}) after squashing, exceeding sequence max {max}")]
    SequenceOverflow { timestamp_ms: u64, node_id: u16, count: usize, max: usize },

    #[error("HeerId construction failed: {0}")]
    HeerIdError(#[from] Error),
}
```

## FFI Exposure

```c
// Set precision (call once at startup)
void ranj_set_precision(int precision);  // 0=us, 1=ns, 2=ps, 3=fs

// Batch HeerId → RanjId
int heer_id_batch_to_ranj_ids(
    const int64_t* heer_ids, int count,
    int64_t* heer_ids_out, uint8_t* ranj_ids_out);

// Batch RanjId → HeerId
int ranj_id_batch_to_heer_ids(
    const uint8_t* ranj_ids, int count,
    uint8_t* ranj_ids_out, int64_t* heer_ids_out);

// Pre-flight check
int ranj_id_check_heer_convertibility(
    const uint8_t* ranj_ids, int count,
    int* conflict_count_out);
```

## Python Binding

```python
# Pre-flight check (always empty — HeerId always fits in RanjId)
conflicts = HeerId.check_ranjid_convertibility([hid1, hid2, hid3])

# Batch HeerId → RanjId (always succeeds)
pairs = HeerId.batch_to_ranjids([hid1, hid2, hid3])
# Returns: [(hid1, rid1), (hid2, rid2), (hid3, rid3)]

# Pre-flight check (may return conflicts)
conflicts = RanjId.check_heerid_convertibility([rid1, rid2, rid3])
# Returns: [ConversionConflict(kind=..., ranj_ids=[...])]

# Batch RanjId → HeerId (raises ValueError if any group overflows)
pairs = RanjId.batch_to_heerids([rid1, rid2, rid3])
# Returns: [(rid1, hid1), (rid2, hid2), (rid3, hid3)]
```

Both directions follow the same pattern: `check_*_convertibility` → `batch_to_*`. Framework migration tools always call check first, then convert.

## Testing

**Rust unit tests:**
- UUIDv8: version nibble is `1000`, variant is `10`
- UUIDv8: `from_uuid` rejects v7, accepts v8
- Precision: default is microseconds
- Precision: `RANJID_PRECISION=ns` produces nanosecond timestamps
- Precision: `RANJID_PRECISION=fs` produces femtosecond timestamps
- Precision: same 90-bit value means different times at different precisions
- `HeerId::batch_to_ranjids` preserves timestamp, node_id, sequence (precision-aware)
- `HeerId::batch_to_ranjids` returns correct (old, new) tuples
- `RanjId::batch_to_heerids` with no squashing produces correct mappings
- `RanjId::batch_to_heerids` with timestamp squashing reassigns sequences correctly
- `RanjId::batch_to_heerids` preserves ordering within squashed groups
- `RanjId::batch_to_heerids` with node_id > 511 fails with NodeIdOverflow
- `RanjId::batch_to_heerids` with timestamp overflow fails
- `RanjId::batch_to_heerids` with too many IDs in one squashed group fails with SequenceOverflow
- Roundtrip: `batch_to_ranjids` → `batch_to_heerids` preserves ordering
- `check_heerid_convertibility` returns empty for all-valid batch
- `check_heerid_convertibility` returns correct conflicts for mixed batch
- `check_heerid_convertibility` detects sequence overflow from timestamp squashing

**FFI tests:**
- Batch roundtrip through C ABI
- Error code on overflow
- Precision setting

**Python tests:**
- `HeerId.check_ranjid_convertibility` class method returns empty list
- `HeerId.batch_to_ranjids` class method returns correct tuples
- `RanjId.check_heerid_convertibility` class method detects conflicts
- `RanjId.batch_to_heerids` class method returns correct tuples
- `RanjId.batch_to_heerids` raises ValueError on overflow

## What's NOT in scope

- SQL migration operations (Django, EF Core) — separate spec per framework
- `HeeRanjIdPKMixin` model mixin — separate Django spec
- Database-level conversion (ALTER TABLE, FK cascade) — framework spec
- Conversion of IDs stored in JSON or string columns — application-level concern
- SQL generation functions for picosecond/femtosecond precision (documented limitation — use application-level generation)
