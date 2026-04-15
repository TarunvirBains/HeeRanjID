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
