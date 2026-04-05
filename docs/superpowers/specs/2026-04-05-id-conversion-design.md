# HeerId ↔ RanjId Conversion Design

## Goal

Add batch conversion functions to the Rust `heeranjid` core crate that allow converting between HeerId (64-bit) and RanjId (128-bit UUIDv7). These are used by all language bindings for schema migrations when a system needs to change its ID type.

## Problem

A system that started with HeerId (64-bit, millisecond precision) may need to upgrade to RanjId (128-bit UUIDv7, microsecond precision) as requirements grow. The reverse may also be needed. Currently there's no way to convert existing IDs — you'd have to regenerate them, breaking all references.

### Timestamp Squashing Problem (RanjId → HeerId)

RanjId has microsecond precision. HeerId has millisecond precision. When converting RanjId → HeerId, `timestamp_ms = timestamp_micros / 1000` truncates sub-millisecond digits. Two RanjIds with timestamps `1000500` and `1000999` both become `timestamp_ms = 1000`.

If those two RanjIds share the same `node_id`, they would produce identical HeerIds unless the sequence is adjusted. The conversion must:

1. Group by `(timestamp_ms, node_id)` after truncation
2. Reassign sequences within each group to avoid collisions
3. Fail if any group exceeds the 13-bit sequence limit (8191)

This makes RanjId → HeerId conversion inherently a **batch operation**. Single-value conversion is not safe for migration use — it cannot detect timestamp squashing collisions.

## Bit Layouts

```
HeerId (i64, 63 usable bits):
  [41-bit timestamp_ms][9-bit node_id][13-bit sequence]
  Max timestamp: 2^41 - 1 = 2,199,023,255,551 ms (~69 years)
  Max node_id: 511
  Max sequence: 8,191

RanjId (u128 as UUIDv7):
  [48-bit ts_high][4-bit version=0111][12-bit ts_mid][2-bit variant=10][30-bit ts_low][16-bit node_id][16-bit sequence]
  Timestamp is 90 bits of microseconds, split across ts_high(48) | ts_mid(12) | ts_low(30)
  Max node_id: 65,535
  Max sequence: 65,535
```

## Conversion: HeerId → RanjId (Batch)

Always succeeds. Every HeerId value fits in a RanjId. No timestamp squashing occurs because the conversion expands precision (ms → us).

**Mapping per ID:**
- `timestamp_micros = timestamp_ms * 1000` — milliseconds to microseconds. Sub-millisecond digits are zero. Preserves original ordering and value.
- `node_id` — direct copy, zero-extended (9-bit → 16-bit)
- `sequence` — direct copy, zero-extended (13-bit → 16-bit)

Returns old/new tuples so callers can generate UPDATE statements.

```rust
impl HeerId {
    /// Pre-flight check — always returns empty (HeerId always fits in RanjId).
    /// Provided for API symmetry with RanjId::check_heerid_convertibility.
    pub fn check_ranjid_convertibility(ids: &[HeerId]) -> Vec<ConversionConflict> {
        Vec::new()
    }

    pub fn batch_to_ranjids(ids: &[HeerId]) -> Vec<(HeerId, RanjId)> {
        ids.iter()
            .map(|hid| {
                let parts = hid.into_parts();
                let rid = RanjId::new(
                    u128::from(parts.timestamp_ms) * 1000,
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

1. For each RanjId, compute the candidate HeerId parts:
   - `timestamp_ms = timestamp_micros / 1000`
   - `node_id` — unchanged (must be ≤ 511)
   - `sequence` — initially from the RanjId (must be ≤ 8191 before reassignment)

2. Check per-ID hard failures (cannot be fixed by reassignment):
   - `node_id > 511` → `NodeIdOverflow`
   - `timestamp_ms > 2^41 - 1` → `TimestampOverflow`

3. Group by `(timestamp_ms, node_id)`

4. Within each group, sort by original RanjId (preserves ordering), then assign sequences `0, 1, 2, ...`

5. If any group has more than 8192 members → `SequenceOverflow`

6. Return `Vec<(RanjId, HeerId)>` mapping old → new

```rust
impl RanjId {
    pub fn batch_to_heerids(ids: &[RanjId]) -> Result<Vec<(RanjId, HeerId)>, ConversionError> {
        // 1. Check hard failures
        // 2. Group by (timestamp_ms, node_id)
        // 3. Assign sequences within each group
        // 4. Build (old, new) pairs
        // ...
    }
}
```

## Pre-flight Convertibility Check

An associated function that analyzes a batch without converting. Returns a list of conflicts — each describing why a group of IDs can't convert.

```rust
#[derive(Debug)]
pub struct ConversionConflict {
    pub kind: ConflictKind,
    pub ranj_ids: Vec<RanjId>,
}

#[derive(Debug)]
pub enum ConflictKind {
    /// node_id exceeds HeerId's 9-bit max
    NodeIdOverflow { node_id: u16 },
    /// timestamp exceeds HeerId's 41-bit max
    TimestampOverflow { timestamp_ms: u64 },
    /// Too many IDs in one (timestamp_ms, node_id) group after squashing
    SequenceOverflow { timestamp_ms: u64, node_id: u16, count: usize, max: usize },
}

impl RanjId {
    pub fn check_heerid_convertibility(ids: &[RanjId]) -> Vec<ConversionConflict> {
        // Group by (timestamp_ms, node_id)
        // Check node_id overflow
        // Check timestamp overflow
        // Check group sizes > 8192
        // Return conflicts
    }
}
```

If the returned vec is empty, `batch_to_heerids` is guaranteed to succeed.

## Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("timestamp {value} us exceeds HeerId max ({max} ms)")]
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

Exposed through `heeranjid-ffi` for .NET and other C ABI consumers:

```c
// Batch HeerId → RanjId (always succeeds)
// Writes pairs to output buffer: [heer_id_0, ranj_id_0, heer_id_1, ranj_id_1, ...]
int heer_id_batch_to_ranj_ids(
    const int64_t* heer_ids, int count,
    int64_t* heer_ids_out, uint8_t* ranj_ids_out);

// Batch RanjId → HeerId (can fail)
// Returns 0 on success, -1 on error (call heer_last_error for details)
int ranj_id_batch_to_heer_ids(
    const uint8_t* ranj_ids, int count,
    uint8_t* ranj_ids_out, int64_t* heer_ids_out);

// Pre-flight check (returns count of conflicts)
int ranj_id_check_heer_convertibility(
    const uint8_t* ranj_ids, int count,
    int* conflict_count_out);
```

## Python Binding

Exposed through PyO3 in the `heeranjid` Python package:

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
- `HeerId::batch_to_ranjids` preserves timestamp (ms * 1000), node_id, sequence for each ID
- `HeerId::batch_to_ranjids` returns correct (old, new) tuples
- `RanjId::batch_to_heerids` with no squashing produces correct mappings
- `RanjId::batch_to_heerids` with timestamp squashing reassigns sequences correctly
- `RanjId::batch_to_heerids` preserves ordering within squashed groups
- `RanjId::batch_to_heerids` with node_id > 511 fails with NodeIdOverflow
- `RanjId::batch_to_heerids` with timestamp overflow fails
- `RanjId::batch_to_heerids` with too many IDs in one squashed group fails with SequenceOverflow
- Roundtrip: `batch_to_ranjids` → `batch_to_heerids` preserves ordering (sequences may differ due to reassignment)
- `check_heerid_convertibility` returns empty for all-valid batch
- `check_heerid_convertibility` returns correct conflicts for mixed batch
- `check_heerid_convertibility` detects sequence overflow from timestamp squashing

**FFI tests:**
- Batch roundtrip through C ABI
- Error code on overflow

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
