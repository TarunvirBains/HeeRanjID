# HeerRanjId

> Named after Heer and Ranjha, the central figures of a classic South Asian folk tale.

HeerRanjId is a cross-language Snowflake-style ID system built around two related formats:

- `HeerId`: a compact, time-ordered 64-bit integer identifier
- `RanjId`: a UUIDv8-compatible 128-bit identifier with sub-millisecond precision and higher node/sequence capacity

The repository centers on a Rust implementation, with PostgreSQL helpers and
language bindings for Python, Django, TypeScript, .NET, and C FFI consumers.

## Why HeeRanjID

Most teams end up choosing between:

- integers that are compact but collide in distributed systems
- UUIDs that are portable but random — they fragment indexes and carry no timing information
- Snowflake-style IDs that solve ordering but are often tied to one stack or language

HeeRanjID provides a Snowflake-style system that works consistently across languages and databases, with a clear upgrade path built in:

- start with `HeerId` — compact bigint, time-ordered, up to 511 nodes and 8,191 IDs per node per millisecond
- migrate to `RanjId` when you need more headroom — up to 32,767 nodes, 65,535 IDs per node per timestamp unit, sub-millisecond precision, and UUID-compatible storage
- the migration is lossless: `HeerId` converts to `RanjId` without data loss, and the UUID column type is compatible with existing tooling

For tables whose natural read pattern is "newest first" (audit logs, activity feeds, event streams), HeeRanjID also ships `HeerIdDesc` and `RanjIdDesc` — reverse-chronologically-sorted siblings whose raw-bit ordering matches a `DESC` scan, so `ORDER BY id` on a descending column is served directly by a B-tree index without a reverse scan. Conversion between asc and desc is a lossless XOR against a flip mask that preserves the node field (and, for RanjId, UUIDv8 version/variant). See [`docs/migrations/asc-to-desc.md`](./docs/migrations/asc-to-desc.md) for the playbook that converts an existing column under live writes.

## When to use HeeRanjID (and when not to)

Three common alternatives cover most projects: **database sequences** (BIGSERIAL / IDENTITY), **UUIDv7**, and **HeeRanjID**. Each wins on different axes:

| Need / property | BIGSERIAL | UUIDv7 | HeeRanjID |
|---|---|---|---|
| Setup cost | None — built into Postgres | None — built into PG 18+ / every driver | Seed `heer_nodes`, bind session node_id |
| Storage | 8 bytes | 16 bytes | 8 bytes (`HeerId`) or 16 bytes (`RanjId`) |
| Multi-writer / distributed generation | Needs coordination (shared sequence, range-leasing) | Yes, every client generates independently | Yes, via coordinated `node_id` allocation |
| Client-side generation (no DB round-trip) | No — each `INSERT` must call the sequence | Yes | Yes |
| Time-ordered | Yes (insertion-ordered, but not wall-clock) | Yes (wall-clock ms) | Yes (wall-clock ms / sub-ms) |
| Reverse-chronological `ORDER BY id` without a secondary index | No — needs `DESC` index or reverse scan | No — same | **Yes, via `HeerIdDesc` / `RanjIdDesc`** |
| Enumeration-resistant | No (trivially predictable) | Partial (random low bits) | No (embeds timing) |
| Cross-DB portability of existing IDs | No — sequence state is database-local | Yes | Yes |
| Clock-rollback protection | N/A (monotonic by construction) | Implementation-defined | Yes — `heer_node_state` tracks last-issued per node |
| Upgrade path (8 → 16 bytes without rewrite) | No | N/A (already 16) | Yes — `HeerId ↔ RanjId` via `From` / `TryFrom` |
| Cross-language bit-for-bit parity | Yes (it's just an int) | Mostly (tie-breaking varies by driver) | Yes — Rust + Python/Django + TS + .NET + C FFI |

**When each is the right default:**

- **BIGSERIAL** — single-writer Postgres apps with no multi-region or sharding plans. Simple, cheap, boring. The round-trip cost per INSERT only matters at high write rates.
- **UUIDv7** — multi-writer, client-side generation, greenfield, no strong sort-order requirements. The RFC-standard choice.
- **HeeRanjID** — any one of: (a) you want newest-first `ORDER BY id` cheap, (b) you want 8-byte IDs *now* with an escape hatch to 16 bytes later, (c) you need identical ID semantics across multiple language stacks sharing a DB, (d) you want server-side generators with clock-rollback guards.

**Don't reach for HeeRanjID if:**

- You're greenfield and single-writer — BIGSERIAL is fine and requires zero ceremony.
- You're greenfield and multi-writer with no strong sort requirement — UUIDv7 wins on inertia and ecosystem support.
- You don't use PostgreSQL (MSSQL is planned for v0.3.1). The database-side generators are where half the value lives.
- You want opaque IDs for URL safety or enumeration-resistance. HeeRanjID's IDs carry timing information; use `nanoid`, `cuid2`, or `sqids` instead.
- Your nodes can't be assigned stable, coordinated node_ids at provisioning time (`heer_nodes` table is required).

### Caveats worth reading before you adopt

- `HeerId → RanjId` is always lossless. `RanjId → HeerId` preserves `timestamp_ms` and `node_id` but **reassigns `sequence`** within each `(timestamp_ms, node_id)` group (`HeerId`'s 13 sequence bits can't hold `RanjId`'s 16). Single-value `TryFrom<RanjId> for HeerId` always returns `sequence = 0` — see the rustdoc on that impl.
- `HeerIdDesc`'s raw-bit ordering matches reverse-chronological only when all values share the same direction. String boundaries (JSON APIs, untyped columns) can silently reinterpret direction; the type system prevents this inside Rust.
- The asc↔desc migration is a multi-hour operation on large tables — worth it if you're going to run it once, probably not worth it to experiment.

## Repository Layout

- [`heeranjid/`](./heeranjid): core Rust types and conversions
- [`heeranjid-sqlx/`](./heeranjid-sqlx): PostgreSQL and SQLx integration
- [`heeranjid-ffi/`](./heeranjid-ffi): C FFI shared library
- [`bindings/python/`](./bindings/python): Python bindings
- [`bindings/python/django/`](./bindings/python/django): Django fields and managers
- [`bindings/typescript/`](./bindings/typescript): Node / TypeScript bindings
- [`bindings/dotnet/`](./bindings/dotnet): .NET bindings
- [`docs/`](./docs): format, design, and publishing documentation
- [`sql/`](./sql): SQL assets used by the database-backed integrations

## Clone And Build

This repository uses git submodules for SQL assets and helper scripts. Clone it
with submodules, or initialize them after cloning:

```bash
git clone --recurse-submodules https://github.com/TarunvirBains/HeeRanjID.git
```

or:

```bash
git submodule update --init --recursive
```

Without the `sql/` submodule, the SQLx crate and Python wheel build will fail.

## Current Status

- Core Rust crate: ready for normal development use
- SQLx/PostgreSQL integration: builds and tests locally from a complete checkout
- Python wheel: builds locally with `maturin`
- TypeScript native module: builds locally
- FFI shared library: builds locally

Package-publishing polish is still in progress, mainly around metadata quality,
package-specific READMEs, and release documentation.

## Minimal Rust Example

```rust
use heeranjid::{HeerId, RanjId, RanjPrecision};

let heer = HeerId::new(1_000, 7, 42)?;
let ranj = RanjId::new(1_000_000, RanjPrecision::Microseconds, 7, 42)?;

assert_eq!(heer.node_id(), 7);
assert_eq!(ranj.node_id(), 7);
# Ok::<(), heeranjid::Error>(())
```

## Minimal Python Example

```python
from heeranjid import HeerId, RanjId

hid = HeerId(42)
rid = RanjId.from_str("00000000-0000-8000-8007-a120006400c8")
```

## Documentation

Start here:

- [`docs/overview.md`](./docs/overview.md)
- [`docs/id-formats/heerid.md`](./docs/id-formats/heerid.md)
- [`docs/id-formats/ranjid.md`](./docs/id-formats/ranjid.md)
- [`docs/id-formats/conversion.md`](./docs/id-formats/conversion.md)
- [`docs/generation/database-generation.md`](./docs/generation/database-generation.md)
- [`docs/PUBLISHING.md`](./docs/PUBLISHING.md)

## Release Notes

The main remaining release tasks are:

- finalizing `crates.io` metadata for all Rust crates
- improving package pages on PyPI
- tightening public-facing docs and examples

That work is tracked directly in this repository rather than a separate release
branch.
