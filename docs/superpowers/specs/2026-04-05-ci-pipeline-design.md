# CI Pipeline Design

## Goal

A multi-language CI pipeline where the Rust job compiles all binding artifacts once, then fans out to language-specific test jobs. Linux-only. No cross-platform builds.

## Problem

The current CI workflow only covers Rust (lint, unit tests, Postgres integration). The Python, TypeScript, and .NET bindings have no CI — their tests only run locally. Each binding needs a compiled Rust artifact to function, and compiling Rust in every language job wastes time.

## Architecture

```
rust (lint + test + build all artifacts)
  ├── python (install wheel, run core + Django tests)
  ├── typescript (install .node, run type tests)
  └── dotnet (link FFI, dotnet test)
```

One workflow file. The Rust job produces all artifacts. Language jobs depend on the Rust job, download their artifact, and run tests. No language job compiles Rust.

## Rust Job

**Image:** `rust:1.94-slim-bookworm` (upgrade from Bullseye for Python 3.11+ and Node 18+)

**Additional installs:**
- `apt-get install python3 python3-pip python3-venv nodejs npm` — language runtimes for building binding artifacts
- `pip install maturin` — builds Python wheel (PyO3)
- `npm install -g @napi-rs/cli` — builds TypeScript native module (NAPI-RS)
- Existing: git, curl, rustfmt, clippy, cargo-deny

**Lint steps (unchanged):**
- `cargo fmt --all --check`
- `cargo clippy --workspace --exclude heeranjid-python --exclude heeranjid-node -- -D warnings`
- `cargo deny check`

**Test steps (unchanged):**
- `cargo test -p heeranjid --lib` — 24 unit tests
- `cargo test -p heeranjid-sqlx --test postgres` — Postgres integration
- `cargo test -p heeranjid-sqlx --test concurrency` — Postgres concurrency

**Build artifact steps (new):**
- `cd bindings/python && make build` — runs `maturin build --release`, produces `.whl` in `target/wheels/`
- `cd bindings/typescript && npm run build` — runs `napi build --platform --release`, produces `.node` file
- `cargo build -p heeranjid-ffi --release` — produces `target/release/libheeranjid_ffi.so` + `heeranjid.h` (via cbindgen)

**Upload artifacts:**
- `python-wheel` — the `.whl` file from `target/wheels/`
- `typescript-native` — the `.node` file from `bindings/typescript/`
- `ffi-linux-x64` — `libheeranjid_ffi.so` + `heeranjid.h`

**Services:** Postgres (same as current)

## Python Job

**Needs:** `rust`

**Image:** `python:3.11-slim-bookworm`

**Steps:**
1. Checkout (with submodules — needed for SQL files in tests)
2. Download `python-wheel` artifact
3. `pip install *.whl` — installs `heeranjid` from the wheel
4. `pip install -e bindings/python/django/` — installs `heeranjid-django` in dev mode
5. `pip install pytest django psycopg2-binary` — test dependencies
6. `pytest bindings/python/tests/` — core type tests + SQL constants (31 tests)
7. `pytest bindings/python/django/tests/test_django_fields.py` — Django field unit tests (21 tests)
8. `pytest bindings/python/django/tests/test_postgres_integration.py` — Postgres integration (6 tests)

**Services:** Postgres (same config as Rust job)

**Not in scope this session:**
- MSSQL integration tests (needs ODBC driver + MSSQL service container)

## TypeScript Job

**Needs:** `rust`

**Image:** `node:18-bookworm-slim`

**Steps:**
1. Checkout (with submodules)
2. Download `typescript-native` artifact, place `.node` in `bindings/typescript/`
3. `cd bindings/typescript && npm install` — install dev dependencies (vitest, typescript)
4. `npm test` — runs `vitest run` (HeerId/RanjId type tests, Prisma extension shape tests)

**No database needed** — current tests are unit-only.

**Not in scope this session:**
- Prisma integration tests against real Postgres/MSSQL
- Splitting Prisma into separate framework package

## .NET Job

**Needs:** `rust`

**Image:** `mcr.microsoft.com/dotnet/sdk:8.0`

**Steps:**
1. Checkout (with submodules)
2. Download `ffi-linux-x64` artifact
3. Place `libheeranjid_ffi.so` where the .NET runtime can find it (e.g., alongside test assembly or in `LD_LIBRARY_PATH`)
4. `dotnet test bindings/dotnet/tests/HeeRanjID.Tests/` — HeerId/RanjId type tests, SqlHelper tests

**No database needed** — current tests are unit-only.

**Not in scope this session:**
- EF Core integration tests against real Postgres/MSSQL
- Splitting EF Core into separate framework package

## Test Parity: Postgres ↔ MSSQL

The MSSQL integration test suite (33 tests) is a superset of the Postgres suite (6 tests). Before CI goes live, bring Postgres to parity. Both suites should have identical test coverage:

**Tests to add to Postgres (matching MSSQL):**
- `test_bulk_ids_are_unique` — 100 IDs, verify uniqueness
- `test_bulk_ids_monotonically_increasing` — 50 IDs, verify ordering
- `test_ids_across_calls_are_unique` — 5 single generate calls
- `test_different_nodes_produce_different_ids` — node 1 vs node 2
- `test_node_id_roundtrips_through_decode` — verify node_id survives encode/decode
- `TestHeerIdErrors` — invalid node, zero/negative count, session unset, allow_spanning overflow
- `TestRanjIdErrors` — invalid node, zero/negative count, spanning overflow
- `TestDjangoFieldsPostgres` — from_db_value, prep roundtrip, db_type (mirror of TestDjangoFieldsMssql)
- `TestConcurrencyPostgres` — 4 threads x 50 IDs for both HeerId and RanjId

After parity, both test files should have the same class/test structure, differing only in SQL dialect (EXEC vs SELECT, pyodbc vs psycopg2).

## Django ORM Integration Tests

**Blocked on separate spec:** The Django ORM integration tests require a `HeeranjidManager` design (bulk_create support, field enforcement). This is being designed in a separate spec. Once that spec is implemented, the Django ORM tests will be added to this pipeline.

The Python job's Postgres service is already configured to support these tests when they're ready.

## Artifacts

| Artifact | Produced by | Contents | Consumed by |
|----------|-------------|----------|-------------|
| `python-wheel` | `maturin build --release` | `heeranjid-0.1.0-*.whl` | Python job |
| `typescript-native` | `napi build --platform --release` | `heeranjid.linux-x64-gnu.node` | TypeScript job |
| `ffi-linux-x64` | `cargo build -p heeranjid-ffi --release` | `libheeranjid_ffi.so` + `heeranjid.h` | .NET job |

The SQL submodule is checked out in every job (`submodules: recursive`), so SQL files are not passed as artifacts.

## Image Upgrade: Bullseye → Bookworm

The Rust job image changes from `rust:1.94-slim-bullseye` to `rust:1.94-slim-bookworm`. Bookworm (Debian 12) provides:
- Python 3.11 (Bullseye has 3.9)
- Node 18 (Bullseye has 12)
- No other differences that affect this pipeline

The `requires-python` in `pyproject.toml` stays `>=3.10` — the CI just happens to use 3.11 from Bookworm.

## Future: Custom Docker Image (Approach B)

When the install steps become a bottleneck, build a custom CI image:

```dockerfile
FROM rust:1.94-slim-bookworm
RUN apt-get update && apt-get install -y \
    python3 python3-pip python3-venv nodejs npm git curl \
    && pip install maturin \
    && npm install -g @napi-rs/cli \
    && rustup component add rustfmt clippy \
    && <install cargo-deny>
```

Publish to GHCR (`ghcr.io/tarunvirbains/heeranjid-ci:latest`). The CI workflow switches `image:` to point at it, saving ~60-90s of install time per run. The language test jobs already use lightweight stock images and don't need a custom image.

## What's In Scope

- Upgrade Rust image from Bullseye to Bookworm
- Install Python, Node, maturin, napi-rs in Rust job
- Build and upload all three binding artifacts from Rust job
- Bring Postgres integration tests to parity with MSSQL (same test structure, same coverage)
- Python job: install wheel, run core tests + Django field tests + Postgres integration
- TypeScript job: install native module, run vitest
- .NET job: link FFI, run dotnet test
- Document custom Docker image migration path

## What's NOT In Scope

- MSSQL integration tests in CI (needs ODBC driver + MSSQL service container)
- Postgres integration tests for TypeScript or .NET
- Django ORM integration tests (blocked on HeeranjidManager design — separate spec)
- Framework ORM tests for Prisma, EF Core
- Cross-platform builds (macOS, Windows)
- Publishing to PyPI / npm / NuGet
- Custom Docker CI image (documented for future)
- Splitting Prisma or EF Core into separate framework packages
