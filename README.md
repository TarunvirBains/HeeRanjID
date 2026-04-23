# HeerRanjId

> Named after Heer and Ranjha, the central figures of a classic South Asian folk tale.

**HeeRanjID lets you start a project on a single Postgres node with a simple 8-byte integer PK, then migrate to distributed writers later without rewriting a single ID or a single line of application code.**

The core type, `HeerId`, is designed specifically for this: it's an 8-byte time-ordered integer whose bit layout *already* carries a `node_id` field even when you only have one writer. Going from one node to many later is a config change (allocate more `node_id` values in `heer_nodes`, bind each new writer's session) — no schema change, no ID-format change, no downtime, no application migration. Existing IDs stay valid forever.

`RanjId` is the natural extension when `HeerId`'s fields stop being enough (more than 511 nodes, more than 8,191 IDs per node per millisecond, or sub-millisecond timestamp precision). Same design philosophy, bigger fields, UUIDv8-compatible 16-byte storage. The `HeerId → RanjId` path is lossless and the reverse is well-defined, so you can grow through both tiers without a forklift migration.

For tables whose natural read pattern is "newest first" (audit logs, activity feeds, event streams), HeeRanjID also ships `HeerIdDesc` and `RanjIdDesc` — reverse-chronologically-sorted siblings whose raw-bit ordering matches a `DESC` scan, so `ORDER BY id` on a descending column is served directly by the PK index with no secondary index and no reverse scan. Conversion between asc and desc is a lossless XOR against a flip mask that preserves the node field (and, for RanjId, UUIDv8 version/variant). See [`docs/migrations/asc-to-desc.md`](./docs/migrations/asc-to-desc.md) for the playbook that converts an existing column under live writes.

The repository centers on a Rust implementation, with PostgreSQL helpers and language bindings for Python, Django, TypeScript, .NET, and C FFI consumers — so the IDs behave identically in every service that shares the database.

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

**Strengths of each option:**

- **BIGSERIAL** — zero ceremony on day one. But: no client-side generation, no multi-writer path, and migrating to a distributed setup later means rewriting both IDs and every system that stores them. A lock-in decision dressed as simplicity.
- **UUIDv7** — RFC-standard, well-supported, works the same way on one node or a hundred. 16 bytes forever, random sub-ms tie-breaking, reverse scan or secondary index for newest-first reads.
- **HeeRanjID** — designed specifically for the single-node greenfield case with a **painless upgrade path to distributed later**. `HeerId` on one node behaves like a sequence — 8 bytes, time-ordered, server-side generated — but the format already carries a node field. Adding writers later means allocating more `node_id` values in the `heer_nodes` table and binding each service's session; existing IDs stay valid, no schema change, no ID-format change, no downtime. Plus: newest-first `ORDER BY id` from the PK index via `HeerIdDesc`, a further lossless upgrade path from `HeerId` to `RanjId` when 8 bytes stops being enough, and identical ID semantics across Rust / Python-Django / TypeScript / .NET / C FFI.

In short: **BIGSERIAL paints you into a corner, UUIDv7 charges you 16 bytes forever for options you may never exercise, HeeRanjID lets you start simple and grow without rewrites.**

**Genuine reasons to pick something else:**

- **Non-Postgres / non-MSSQL stack** — the database-side generators and migration tooling are half the value. MSSQL is planned for v0.3.1; other engines are not on the roadmap.
- **You need opaque / URL-safe / enumeration-resistant IDs** — HeeRanjID IDs embed a timestamp and node. Use `nanoid`, `cuid2`, or `sqids` instead.
- **You can't coordinate node_id allocation at provisioning time** — fully ephemeral / autoscaled workers with no stable identity don't fit the `heer_nodes` model. (A single node_id for a single-writer deployment is the easy case; this caveat only bites at the scale-out edge.)

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
