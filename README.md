# HeerRanjId

> **Pronunciation:** *"Heer-Ranj-Id"* — named after Heer and Ranjha, the star-crossed lovers of the classic Punjabi folk tale (think Romeo and Juliet, set in Punjab). `HeerId` takes its name from Heer; `RanjId` from Ranjha.

HeerRanjId is a cross-language ID system built around two related formats:

- `HeerId`: a compact, time-ordered 64-bit identifier for internal storage
- `RanjId`: a UUIDv8-compatible 128-bit identifier for APIs and cross-system interoperability

The repository centers on a Rust implementation, with PostgreSQL helpers and
language bindings for Python, Django, TypeScript, .NET, and C FFI consumers.

## Why HeeRanjID

Most teams end up choosing between:

- integers that are compact but local to one database
- UUIDs that are portable but larger and less index-friendly
- Snowflake-style IDs that work well internally but are often tied to one stack

HeeRanjID separates those concerns cleanly:

- store `HeerId` where compactness and index locality matter
- expose `RanjId` where UUID compatibility matters
- keep one encoding model across multiple languages and database integrations

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
