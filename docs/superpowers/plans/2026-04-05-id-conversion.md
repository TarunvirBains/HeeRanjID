# HeerId ↔ RanjId Conversion + UUIDv8 Precision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Switch RanjId from UUIDv7 to UUIDv8 with self-describing 2-bit precision field, change bit layout to 89-bit timestamp / 15-bit node / 16-bit sequence, add `RanjPrecision` enum, add batch conversion functions between HeerId and RanjId, and update all bindings and tests.

**Architecture:** The Rust `heeranjid` core crate gets a new `RanjPrecision` enum, updated bit layout constants, and a new `convert` module with batch conversion functions. The version nibble changes from v7 to v8. All downstream consumers (FFI, Python, TypeScript, .NET, sqlx, SQL functions) update their version checks and tests. README and docs are updated to reflect the UUIDv8 change.

**Tech Stack:** Rust, PyO3, NAPI-RS, .NET P/Invoke, PostgreSQL, MSSQL

---

## File Structure

### Files to create

| File | Purpose |
|------|---------|
| `heeranjid/src/precision.rs` | `RanjPrecision` enum with `OnceLock` cached value from env |
| `heeranjid/src/convert.rs` | Batch conversion functions, `ConversionError`, `ConversionConflict` |

### Files to modify

| File | Change |
|------|--------|
| `heeranjid/src/ranj.rs` | New bit layout (89ts/2prec/15node/16seq), version 8, precision-aware `new()` and `into_parts()` |
| `heeranjid/src/error.rs` | Version error message v7→v8 |
| `heeranjid/src/lib.rs` | Export new modules, update tests |
| `heeranjid-ffi/src/lib.rs` | Add batch conversion FFI functions |
| `bindings/python/src/lib.rs` | Add conversion methods, precision getter |
| `heeranjid-sqlx/tests/postgres.rs` | Update v7→v8 test |
| `bindings/python/tests/test_ranjid.py` | Update v7→v8 assertions |
| `bindings/typescript/tests/ranjid.test.ts` | Update v7→v8 assertions |
| `bindings/dotnet/tests/HeeRanjID.Tests/RanjIdTests.cs` | Update v7→v8 assertions |
| `bindings/dotnet/src/HeeRanjID/RanjId.cs` | Update doc comment |
| `README.md` | Document UUIDv8 change, precision, updated bit layout |

---

### Task 1: Add RanjPrecision enum

Create the precision type and environment-based configuration.

**Files:**
- Create: `heeranjid/src/precision.rs`
- Modify: `heeranjid/src/lib.rs`

- [ ] **Step 1: Create precision.rs**

Create `heeranjid/src/precision.rs`:

```rust
use std::sync::OnceLock;

/// Timestamp precision for RanjId generation.
///
/// Encoded as 2 bits in the UUID itself, making every RanjId self-describing.
/// For generation, the precision is read from `RANJID_PRECISION` env var at first use.
/// For decoding, the precision is read from the 2-bit field in the UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RanjPrecision {
    Microseconds = 0b00,
    Nanoseconds  = 0b01,
    Picoseconds  = 0b10,
    Femtoseconds = 0b11,
}

impl RanjPrecision {
    /// Multiplier to convert from microseconds to this precision's unit.
    pub fn from_micros_multiplier(self) -> u128 {
        match self {
            Self::Microseconds => 1,
            Self::Nanoseconds  => 1_000,
            Self::Picoseconds  => 1_000_000,
            Self::Femtoseconds => 1_000_000_000,
        }
    }

    /// Divisor to convert from this precision's unit to microseconds.
    pub fn to_micros_divisor(self) -> u128 {
        self.from_micros_multiplier()
    }

    /// Multiplier to convert from milliseconds to this precision's unit.
    pub fn from_millis_multiplier(self) -> u128 {
        self.from_micros_multiplier() * 1_000
    }

    /// Create from the 2-bit field value in a RanjId UUID.
    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0b11 {
            0b00 => Some(Self::Microseconds),
            0b01 => Some(Self::Nanoseconds),
            0b10 => Some(Self::Picoseconds),
            0b11 => Some(Self::Femtoseconds),
            _ => None,
        }
    }

    /// The 2-bit value to encode in the UUID.
    pub fn to_bits(self) -> u8 {
        self as u8
    }

    /// Short label for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Microseconds => "us",
            Self::Nanoseconds  => "ns",
            Self::Picoseconds  => "ps",
            Self::Femtoseconds => "fs",
        }
    }
}

static GENERATION_PRECISION: OnceLock<RanjPrecision> = OnceLock::new();

/// Returns the precision used for generating new RanjIds.
/// Read from `RANJID_PRECISION` env var on first call (default: `ns`).
pub fn generation_precision() -> RanjPrecision {
    *GENERATION_PRECISION.get_or_init(|| {
        match std::env::var("RANJID_PRECISION").as_deref() {
            Ok("us") => RanjPrecision::Microseconds,
            Ok("ns") => RanjPrecision::Nanoseconds,
            Ok("ps") => RanjPrecision::Picoseconds,
            Ok("fs") => RanjPrecision::Femtoseconds,
            _ => RanjPrecision::Nanoseconds, // default
        }
    })
}
```

- [ ] **Step 2: Export from lib.rs**

Add to `heeranjid/src/lib.rs`:

```rust
mod precision;
pub use precision::{RanjPrecision, generation_precision};
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p heeranjid
```

- [ ] **Step 4: Commit**

```bash
git add heeranjid/src/precision.rs heeranjid/src/lib.rs
git commit -m "feat: add RanjPrecision enum with env-based generation config"
```

---

### Task 2: Update RanjId bit layout to UUIDv8 with precision

Change the bit layout from 90ts/16node/16seq/v7 to 89ts/2prec/15node/16seq/v8.

**Files:**
- Modify: `heeranjid/src/ranj.rs`
- Modify: `heeranjid/src/error.rs`
- Modify: `heeranjid/src/lib.rs` (tests)

- [ ] **Step 1: Update constants in ranj.rs**

Change the constants at the top of `heeranjid/src/ranj.rs`:

```rust
pub const RANJ_TIMESTAMP_BITS: u8 = 89;
pub const RANJ_PRECISION_BITS: u8 = 2;
pub const RANJ_NODE_ID_BITS: u8 = 15;
pub const RANJ_SEQUENCE_BITS: u8 = 16;
pub const RANJ_UUID_VERSION: u8 = 0b1000; // UUIDv8
pub const RANJ_UUID_VARIANT: u8 = 0b10;
```

Update `RanjIdParts` to include precision:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RanjIdParts {
    pub timestamp: u128,
    pub precision: RanjPrecision,
    pub node_id: u16,
    pub sequence: u16,
}
```

Note: field renamed from `timestamp_micros` to `timestamp` since the unit depends on precision.

- [ ] **Step 2: Update MAX constants**

```rust
impl RanjId {
    pub const MAX_TIMESTAMP: u128 = (1u128 << RANJ_TIMESTAMP_BITS) - 1;
    pub const MAX_NODE_ID: u16 = (1u16 << RANJ_NODE_ID_BITS) - 1; // 32767
    pub const MAX_SEQUENCE: u16 = u16::MAX;
```

- [ ] **Step 3: Rewrite `RanjId::new()` with new bit packing**

The new layout splits the 89-bit timestamp across the same positions but with 1 fewer bit in `ts_low` (29 instead of 30). The 2 precision bits sit between variant and ts_low.

```rust
pub fn new(timestamp: u128, precision: RanjPrecision, node_id: u16, sequence: u16) -> Result<Self, Error> {
    if timestamp > Self::MAX_TIMESTAMP {
        return Err(Error::TimestampOutOfRange {
            value: timestamp,
            bits: RANJ_TIMESTAMP_BITS,
        });
    }
    if node_id > Self::MAX_NODE_ID {
        return Err(Error::NodeIdOutOfRange {
            value: node_id as u32,
            bits: RANJ_NODE_ID_BITS,
        });
    }

    // 89-bit timestamp split: ts_high(48) | ts_mid(12) | ts_low(29)
    let ts_high = (timestamp >> 41) & ((1u128 << 48) - 1);
    let ts_mid = (timestamp >> 29) & ((1u128 << 12) - 1);
    let ts_low = timestamp & ((1u128 << 29) - 1);

    let raw = (ts_high << 80)
        | (u128::from(RANJ_UUID_VERSION) << 76)
        | (ts_mid << 64)
        | (u128::from(RANJ_UUID_VARIANT) << 62)
        | (u128::from(precision.to_bits()) << 60)
        | (ts_low << 31)
        | (u128::from(node_id) << 16)
        | u128::from(sequence);

    Ok(Self(Uuid::from_u128(raw)))
}
```

- [ ] **Step 4: Rewrite `into_parts()` with precision decoding**

```rust
pub fn into_parts(self) -> RanjIdParts {
    let raw = self.0.as_u128();
    let ts_high = (raw >> 80) & ((1u128 << 48) - 1);
    let ts_mid = (raw >> 64) & ((1u128 << 12) - 1);
    let precision_bits = ((raw >> 60) & 0b11) as u8;
    let ts_low = (raw >> 31) & ((1u128 << 29) - 1);

    RanjIdParts {
        timestamp: (ts_high << 41) | (ts_mid << 29) | ts_low,
        precision: RanjPrecision::from_bits(precision_bits)
            .expect("2-bit value always maps to valid precision"),
        node_id: ((raw >> 16) & u128::from(Self::MAX_NODE_ID)) as u16,
        sequence: (raw & 0xFFFF) as u16,
    }
}
```

- [ ] **Step 5: Update accessor methods**

```rust
pub fn timestamp(self) -> u128 {
    self.into_parts().timestamp
}

pub fn precision(self) -> RanjPrecision {
    self.into_parts().precision
}

// Keep backward compat helper
pub fn timestamp_micros(self) -> u128 {
    let parts = self.into_parts();
    parts.timestamp / parts.precision.to_micros_divisor()
}
```

- [ ] **Step 6: Update `from_uuid()` to accept v8**

In `error.rs`, change:
```rust
#[error("uuid version must be 8 (UUIDv8)")]
InvalidRanjIdVersion,
```

In `ranj.rs`, `from_uuid()` already checks `version != RANJ_UUID_VERSION` — since we changed the constant to `0b1000`, this now validates v8.

- [ ] **Step 7: Add `use crate::precision::RanjPrecision;` to ranj.rs**

- [ ] **Step 8: Update all tests in lib.rs**

Every test that calls `RanjId::new()` needs a precision parameter. Every assertion on `timestamp_micros` or `MAX_TIMESTAMP_MICROS` needs updating. Every version assertion changes from 7 to 8. Every `MAX_NODE_ID` assertion changes from 65535 to 32767.

Update all tests in `heeranjid/src/lib.rs` to use the new API. Key changes:
- `RanjId::new(ts, node, seq)` → `RanjId::new(ts, RanjPrecision::Microseconds, node, seq)`
- `parts.timestamp_micros` → `parts.timestamp`
- `MAX_TIMESTAMP_MICROS` → `MAX_TIMESTAMP`
- `uuid.get_version_num(), 7` → `uuid.get_version_num(), 8`
- `MAX_NODE_ID` assertions: 65535 → 32767
- Node ID values > 32767 in tests should be reduced

- [ ] **Step 9: Run tests**

```bash
cargo test -p heeranjid --lib
```

Expected: all tests pass with new layout.

- [ ] **Step 10: Commit**

```bash
git add heeranjid/src/ranj.rs heeranjid/src/error.rs heeranjid/src/lib.rs
git commit -m "feat: switch RanjId to UUIDv8 with self-describing precision (89ts/2prec/15node/16seq)"
```

---

### Task 3: Add batch conversion functions

**Files:**
- Create: `heeranjid/src/convert.rs`
- Modify: `heeranjid/src/lib.rs`

- [ ] **Step 1: Create convert.rs with types**

Create `heeranjid/src/convert.rs` with the error types, conflict types, and batch conversion functions as described in the spec. The module should contain:

- `ConversionError` enum (TimestampOverflow, NodeIdOverflow, SequenceOverflow, HeerIdError)
- `ConversionConflict` struct with `ConflictKind` enum
- `HeerId::check_ranjid_convertibility(ids)` — always returns empty vec
- `HeerId::batch_to_ranjids(ids)` — precision-aware, uses `generation_precision()`
- `RanjId::check_heerid_convertibility(ids)` — groups by (ts_ms, node_id), checks overflows
- `RanjId::batch_to_heerids(ids)` — handles timestamp squashing with sequence reassignment

Key implementation detail for `batch_to_heerids`:
1. Convert each RanjId timestamp to milliseconds: `timestamp / precision.from_millis_multiplier()`
2. Check node_id ≤ 511 and timestamp_ms ≤ MAX for each ID
3. Group by `(timestamp_ms, node_id)` using a `HashMap`
4. Sort each group by original RanjId (preserves ordering)
5. Assign sequences 0, 1, 2, ... within each group
6. Fail if any group > 8192 members

- [ ] **Step 2: Add tests for conversion**

Add conversion tests to `heeranjid/src/lib.rs` or as a test module in `convert.rs`:

- `batch_to_ranjids` preserves timestamp, node_id, sequence
- `batch_to_ranjids` returns correct (old, new) tuples
- `batch_to_heerids` with no squashing works
- `batch_to_heerids` with timestamp squashing reassigns sequences
- `batch_to_heerids` preserves ordering within squashed groups
- `batch_to_heerids` fails on node_id > 511
- `batch_to_heerids` fails on timestamp overflow
- `batch_to_heerids` fails on sequence overflow from squashing
- `check_heerid_convertibility` returns empty for valid batch
- `check_heerid_convertibility` detects conflicts

- [ ] **Step 3: Export from lib.rs**

```rust
mod convert;
pub use convert::{ConversionError, ConversionConflict, ConflictKind};
```

- [ ] **Step 4: Run all tests**

```bash
cargo test -p heeranjid
```

- [ ] **Step 5: Commit**

```bash
git add heeranjid/src/convert.rs heeranjid/src/lib.rs
git commit -m "feat: add batch HeerId/RanjId conversion with timestamp squashing"
```

---

### Task 4: Update Python binding

**Files:**
- Modify: `bindings/python/src/lib.rs`
- Modify: `bindings/python/tests/test_ranjid.py`

- [ ] **Step 1: Update RanjId Python class**

In `bindings/python/src/lib.rs`:

- Update `RanjId` getters: `timestamp_micros` → add `timestamp` getter alongside (backward compat)
- Add `precision` getter that returns the precision label string ("us", "ns", etc.)
- Add `batch_to_ranjids` classmethod on `HeerId`
- Add `batch_to_heerids` and `check_heerid_convertibility` classmethods on `RanjId`
- Add `check_ranjid_convertibility` classmethod on `HeerId`

- [ ] **Step 2: Update Python tests**

In `bindings/python/tests/test_ranjid.py`:
- Change `test_ranjid_rejects_non_v7` to `test_ranjid_rejects_non_v8`
- Change `assert u.version == 7` to `assert u.version == 8`

- [ ] **Step 3: Build and test**

```bash
cd bindings/python && make dev
/home/tarunvir/projects/HeeRanjID/.venv/bin/pytest tests/test_ranjid.py tests/test_heerid.py -v
```

- [ ] **Step 4: Commit**

```bash
git add bindings/python/src/lib.rs bindings/python/tests/test_ranjid.py
git commit -m "feat(python): update RanjId for UUIDv8, add conversion methods"
```

---

### Task 5: Update remaining bindings and tests

Update TypeScript, .NET, sqlx, and integration tests for v7→v8.

**Files:**
- Modify: `bindings/typescript/tests/ranjid.test.ts`
- Modify: `bindings/dotnet/src/HeeRanjID/RanjId.cs`
- Modify: `bindings/dotnet/tests/HeeRanjID.Tests/RanjIdTests.cs`
- Modify: `heeranjid-sqlx/tests/postgres.rs`
- Modify: `bindings/python/django/tests/test_postgres_integration.py`
- Modify: `bindings/python/django/tests/test_mssql_integration.py`

- [ ] **Step 1: Update TypeScript tests**

In `bindings/typescript/tests/ranjid.test.ts`:
- Change all UUIDv7 references to UUIDv8
- Update version nibble in test UUID construction from `7` to `8`
- Update "rejects non-UUIDv7" to "rejects non-UUIDv8"

- [ ] **Step 2: Update .NET**

In `bindings/dotnet/src/HeeRanjID/RanjId.cs`: update doc comment from UUIDv7 to UUIDv8.

In `bindings/dotnet/tests/HeeRanjID.Tests/RanjIdTests.cs`:
- Update test UUIDs to use version nibble 8
- Update "version 4, not 7" comments to "not 8"

- [ ] **Step 3: Update sqlx tests**

In `heeranjid-sqlx/tests/postgres.rs`:
- Rename `ranjid_sql_generates_valid_uuidv7` to `ranjid_sql_generates_valid_uuidv8`
- Note: the SQL functions still generate with v7 version nibble until they're updated separately. This test may need to be temporarily adjusted or skipped.

- [ ] **Step 4: Update integration tests**

In `bindings/python/django/tests/test_postgres_integration.py`:
- Change `assert u.version == 7` to `assert u.version == 8` in `test_ranjid_is_valid_uuidv7`
- Rename test to `test_ranjid_is_valid_uuidv8`
- Note: same as sqlx — SQL functions still generate v7 until updated. May need to skip or adjust.

In `bindings/python/django/tests/test_mssql_integration.py`: same changes.

- [ ] **Step 5: Run all Rust tests**

```bash
cargo test --workspace --exclude heeranjid-python --exclude heeranjid-node
```

- [ ] **Step 6: Commit**

```bash
git add bindings/typescript/tests/ranjid.test.ts \
        bindings/dotnet/src/HeeRanjID/RanjId.cs \
        bindings/dotnet/tests/HeeRanjID.Tests/RanjIdTests.cs \
        heeranjid-sqlx/tests/postgres.rs \
        bindings/python/django/tests/test_postgres_integration.py \
        bindings/python/django/tests/test_mssql_integration.py
git commit -m "refactor: update all bindings and tests for UUIDv8"
```

---

### Task 6: Update README and docs

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README**

Key changes to `README.md`:
- RanjId description: "128-bit UUIDv7" → "128-bit UUIDv8 with self-describing precision"
- Add precision section explaining `RANJID_PRECISION` env var and the 4 levels (us/ns/ps/fs)
- Update bit layout diagram to show new layout (89ts/2prec/15node/16seq)
- Update max values: node_id max from 65535 to 32767
- Mention that precision is encoded in the UUID itself — no external config needed to decode
- Note: default generation precision is nanoseconds

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README for UUIDv8 with self-describing precision"
```

---

### Task 7: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run lint checks**

```bash
bash scripts/check.sh
```

Expected: all checks pass.

- [ ] **Step 2: Run Rust unit tests**

```bash
cargo test -p heeranjid --lib
```

Expected: all tests pass.

- [ ] **Step 3: Run Python tests**

```bash
cd bindings/python && make dev
/home/tarunvir/projects/HeeRanjID/.venv/bin/pytest bindings/python/tests/ -v
/home/tarunvir/projects/HeeRanjID/.venv/bin/pytest bindings/python/django/tests/test_django_fields.py bindings/python/django/tests/test_managers.py -v
```

Expected: all tests pass.

- [ ] **Step 4: Run Postgres integration tests**

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/heeranjid \
  /home/tarunvir/projects/HeeRanjID/.venv/bin/pytest \
  bindings/python/django/tests/test_postgres_integration.py -v
```

Note: these will fail on `test_ranjid_is_valid_uuidv8` because the SQL functions still generate v7. This is expected — the SQL functions will be updated in a separate task (the `heer_configure()` meta-function spec).

- [ ] **Step 5: Push and check CI**

```bash
git push -u origin feat/id-conversion
```

Create PR and monitor CI.
