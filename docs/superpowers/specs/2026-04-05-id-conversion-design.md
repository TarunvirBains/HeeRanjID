# HeerId ↔ RanjId Conversion Design

## Goal

Add conversion functions to the Rust `heeranjid` core crate that allow converting between HeerId (64-bit) and RanjId (128-bit UUIDv7). These are used by all language bindings for schema migrations when a system needs to change its ID type.

## Problem

A system that started with HeerId (64-bit, millisecond precision) may need to upgrade to RanjId (128-bit UUIDv7, microsecond precision) as requirements grow. The reverse may also be needed. Currently there's no way to convert existing IDs — you'd have to regenerate them, breaking all references.

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

## Conversion: HeerId → RanjId

Always succeeds. Every HeerId value fits in a RanjId.

**Mapping:**
- `timestamp_micros = timestamp_ms * 1000` — milliseconds to microseconds. The sub-millisecond digits are zero. This preserves the original ordering and value. Losing microsecond precision going forward is an acceptable tradeoff.
- `node_id` — direct copy, zero-extended (9-bit → 16-bit)
- `sequence` — direct copy, zero-extended (13-bit → 16-bit)

**Implementation:** Decode HeerId into parts, apply mapping, call `RanjId::new(timestamp_micros, node_id, sequence)`.

```rust
impl HeerId {
    pub fn to_ranjid(&self) -> RanjId {
        let parts = self.into_parts();
        RanjId::new(
            u128::from(parts.timestamp_ms) * 1000,
            parts.node_id,
            parts.sequence,
        ).expect("HeerId always fits in RanjId")
    }
}
```

## Conversion: RanjId → HeerId

Can fail. A RanjId may not fit in a HeerId if:
- `node_id > 511` (exceeds 9-bit max)
- `sequence > 8191` (exceeds 13-bit max)
- `timestamp_micros / 1000 > 2^41 - 1` (exceeds 41-bit millisecond max)

**Mapping:**
- `timestamp_ms = timestamp_micros / 1000` — microseconds to milliseconds, truncating sub-ms precision
- `node_id` — must fit in 9 bits, error if > 511
- `sequence` — must fit in 13 bits, error if > 8191

```rust
impl RanjId {
    pub fn to_heerid(&self) -> Result<HeerId, ConversionError> {
        let parts = self.into_parts();
        let timestamp_ms = parts.timestamp_micros / 1000;
        if timestamp_ms > u128::from(HeerId::MAX_TIMESTAMP_MS) {
            return Err(ConversionError::TimestampOverflow { ... });
        }
        if parts.node_id > HeerId::MAX_NODE_ID {
            return Err(ConversionError::NodeIdOverflow { ... });
        }
        if parts.sequence > HeerId::MAX_SEQUENCE {
            return Err(ConversionError::SequenceOverflow { ... });
        }
        HeerId::new(timestamp_ms as u64, parts.node_id, parts.sequence)
            .map_err(|e| ConversionError::HeerIdError(e))
    }
}
```

## Batch Convertibility Check

An associated function on RanjId that checks a batch of IDs for convertibility without actually converting them. Returns the list of IDs that would fail conversion.

```rust
impl RanjId {
    pub fn check_heerid_convertibility(ids: &[RanjId]) -> Vec<RanjId> {
        ids.iter()
            .filter(|id| id.to_heerid().is_err())
            .copied()
            .collect()
    }
}
```

This is used by framework-level migration tools (Django, EF Core, etc.) to pre-flight check before running a schema conversion. If the returned vec is non-empty, the migration should abort and report which IDs can't be converted.

## Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("timestamp {value} us exceeds HeerId max ({max} ms)")]
    TimestampOverflow { value: u128, max: u64 },

    #[error("node_id {value} exceeds HeerId max ({max})")]
    NodeIdOverflow { value: u16, max: u16 },

    #[error("sequence {value} exceeds HeerId max ({max})")]
    SequenceOverflow { value: u16, max: u16 },

    #[error("HeerId construction failed: {0}")]
    HeerIdError(#[from] Error),
}
```

## FFI Exposure

The conversion functions must be exposed through `heeranjid-ffi` for .NET and other C ABI consumers:

```c
// HeerId → RanjId (always succeeds)
int heer_id_to_ranj_id(int64_t heer_id, uint8_t* ranj_id_out);  // writes 16 bytes

// RanjId → HeerId (can fail)
int ranj_id_to_heer_id(const uint8_t* ranj_id, int64_t* heer_id_out);  // returns 0 on success, -1 on error

// Batch check (returns count of unconvertible IDs)
int ranj_id_check_heer_convertibility(const uint8_t* ranj_ids, int count, uint8_t* failures_out, int* failure_count_out);
```

## Python Binding

Exposed through PyO3 in the `heeranjid` Python package:

```python
hid = HeerId(12345)
rid = hid.to_ranjid()       # always succeeds

rid = RanjId.from_str("...")
hid = rid.to_heerid()       # raises ValueError if overflow

# Batch check
unconvertible = RanjId.check_heerid_convertibility([rid1, rid2, rid3])
```

## Testing

**Rust unit tests:**
- HeerId → RanjId preserves timestamp (ms * 1000), node_id, sequence
- RanjId → HeerId with valid values succeeds
- RanjId → HeerId with node_id > 511 fails with NodeIdOverflow
- RanjId → HeerId with sequence > 8191 fails with SequenceOverflow
- RanjId → HeerId with timestamp overflow fails
- Roundtrip: HeerId → RanjId → HeerId equals original
- Batch check returns empty vec for all-convertible IDs
- Batch check returns correct IDs for mixed batch

**FFI tests:**
- C ABI roundtrip
- Error code on overflow

**Python tests:**
- `to_ranjid()` method on HeerId
- `to_heerid()` method on RanjId
- `check_heerid_convertibility` class method on RanjId
- ValueError on overflow

## What's NOT in scope

- SQL migration operations (Django, EF Core) — separate spec per framework
- `HeeRanjIdPKMixin` model mixin — separate Django spec
- Database-level conversion (ALTER TABLE, FK cascade) — framework spec
- Conversion of IDs stored in JSON or string columns — application-level concern
