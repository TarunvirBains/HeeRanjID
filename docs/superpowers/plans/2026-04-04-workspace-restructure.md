# Workspace Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure HeeRanjID from a single-crate repo into a Cargo workspace with the core crate in a subdirectory, ready for binding crates to be added alongside it.

**Architecture:** Move all existing source into `heeranjid/` subdirectory. Root `Cargo.toml` becomes a workspace manifest. Make `sqlx` an optional feature (`postgres`) so binding crates can depend on the core without pulling in database dependencies.

**Tech Stack:** Rust 2024 edition, Cargo workspaces, feature flags

---

### Task 1: Move core crate into subdirectory

**Files:**
- Create: `heeranjid/Cargo.toml`
- Create: `heeranjid/src/` (moved from `src/`)
- Create: `heeranjid/tests/` (moved from `tests/`)
- Delete: root `src/`, root `tests/`

- [ ] **Step 1: Create the heeranjid subdirectory and move source files**

```bash
mkdir -p heeranjid
mv src heeranjid/
mv tests heeranjid/
```

- [ ] **Step 2: Create the crate-level Cargo.toml**

Create `heeranjid/Cargo.toml`:

```toml
[package]
name = "heeranjid"
version = "0.1.0"
edition = "2024"
license = "MIT"

[features]
default = ["postgres"]
postgres = ["dep:sqlx"]

[dependencies]
serde = { version = "1", features = ["derive"] }
sqlx = { version = "0.8", default-features = false, features = ["postgres", "uuid", "runtime-tokio-rustls", "macros", "time"], optional = true }
thiserror = "2"
uuid = { version = "1", features = ["serde"] }

[dev-dependencies]
serde_json = "1"
tokio = { version = "1.48", features = ["macros", "rt-multi-thread"] }
sqlx = { version = "0.8", default-features = false, features = ["postgres", "uuid", "runtime-tokio-rustls", "macros", "time"] }
```

Note: `sqlx` appears in both `[dependencies]` (optional) and `[dev-dependencies]` (required for tests). This ensures tests always have access to sqlx regardless of feature flags.

- [ ] **Step 3: Replace root Cargo.toml with workspace manifest**

Overwrite root `Cargo.toml`:

```toml
[workspace]
members = ["heeranjid"]
resolver = "2"
```

- [ ] **Step 4: Verify the move compiles**

Run: `cargo build --workspace`
Expected: SUCCESS (may fail — we haven't updated paths yet, that's Task 2)

---

### Task 2: Update include_str paths and add feature gates

**Files:**
- Modify: `heeranjid/src/postgres.rs`
- Modify: `heeranjid/src/heer.rs`
- Modify: `heeranjid/src/ranj.rs`
- Modify: `heeranjid/src/error.rs`
- Modify: `heeranjid/src/lib.rs`

- [ ] **Step 1: Update include_str! paths in postgres.rs**

After the move, `postgres.rs` lives at `heeranjid/src/postgres.rs` and the sql submodule is at `sql/` (repo root). Update all `include_str!` paths from `"../sql/..."` to `"../../sql/..."`:

In `heeranjid/src/postgres.rs`, replace every occurrence of `"../sql/` with `"../../sql/`:

```rust
pub const SCHEMA_SQL: &str = include_str!("../../sql/postgres/schema.sql");
pub const SESSION_SQL: &str = include_str!("../../sql/postgres/functions/session.sql");
pub const GENERATE_HEERID_SQL: &str = include_str!("../../sql/postgres/functions/generate_heerid.sql");
pub const GENERATE_RANJID_SQL: &str = include_str!("../../sql/postgres/functions/generate_ranjid.sql");
pub const INSTALL_SQL: &str = concat!(
    include_str!("../../sql/postgres/schema.sql"),
    "\n",
    include_str!("../../sql/postgres/functions/session.sql"),
    "\n",
    include_str!("../../sql/postgres/functions/generate_heerid.sql"),
    "\n",
    include_str!("../../sql/postgres/functions/generate_ranjid.sql"),
);
pub const FETCH_NODE_SQL: &str = include_str!("../../sql/postgres/queries/fetch_node.sql");
pub const FETCH_EPOCH_SQL: &str = include_str!("../../sql/postgres/queries/fetch_epoch.sql");
pub const SEED_SQL: &str = include_str!("../../sql/postgres/seed.sql");
pub const FETCH_ACTIVE_NODE_SQL: &str =
    include_str!("../../sql/postgres/queries/fetch_active_node.sql");
```

- [ ] **Step 2: Gate the postgres module behind the feature flag**

In `heeranjid/src/lib.rs`, change:

```rust
mod error;
mod heer;
mod postgres;
mod ranj;
mod serde_helpers;
```

to:

```rust
mod error;
mod heer;
#[cfg(feature = "postgres")]
mod postgres;
mod ranj;
mod serde_helpers;
```

And change the `pub use postgres::` block to:

```rust
#[cfg(feature = "postgres")]
pub use postgres::{
    FETCH_ACTIVE_NODE_SQL, FETCH_EPOCH_SQL, FETCH_NODE_SQL, GENERATE_HEERID_SQL,
    GENERATE_RANJID_SQL, HeerConfig, HeerNode, INSTALL_SQL, SCHEMA_SQL, SEED_SQL, SESSION_SQL,
    fetch_active_node, fetch_epoch, fetch_node, generate_heerid, generate_heerids, generate_ranjid,
    generate_ranjids, install_schema, seed_default_node, set_ranj_node_id, validate_epoch,
    validate_heer_node_id, validate_startup,
};
```

- [ ] **Step 3: Gate sqlx::Type derive behind the feature flag**

In `heeranjid/src/heer.rs`, change:

```rust
use sqlx::Type;
```

to:

```rust
#[cfg(feature = "postgres")]
use sqlx::Type;
```

And change the derive on `HeerId`:

```rust
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Type, Serialize, Deserialize,
)]
#[sqlx(transparent)]
```

to:

```rust
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "postgres", derive(sqlx::Type))]
#[cfg_attr(feature = "postgres", sqlx(transparent))]
```

Apply the same change in `heeranjid/src/ranj.rs` — change:

```rust
use sqlx::Type;
```

to:

```rust
#[cfg(feature = "postgres")]
use sqlx::Type;
```

And change the derive on `RanjId`:

```rust
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Type, Serialize, Deserialize,
)]
#[sqlx(transparent)]
```

to:

```rust
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "postgres", derive(sqlx::Type))]
#[cfg_attr(feature = "postgres", sqlx(transparent))]
```

- [ ] **Step 4: Gate sqlx-dependent error types**

In `heeranjid/src/error.rs`, change:

```rust
#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("database returned invalid HeerId: {0}")]
    InvalidHeerId(#[source] Error),
    #[error("database returned invalid RanjId: {0}")]
    InvalidRanjId(#[source] Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("node {0} is not registered or not active")]
    NodeNotActive(u16),
    #[error("heer_config epoch is not configured")]
    MissingEpoch,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

to:

```rust
#[cfg(feature = "postgres")]
#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("database returned invalid HeerId: {0}")]
    InvalidHeerId(#[source] Error),
    #[error("database returned invalid RanjId: {0}")]
    InvalidRanjId(#[source] Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(feature = "postgres")]
#[derive(Debug, Error)]
pub enum StartupError {
    #[error("node {0} is not registered or not active")]
    NodeNotActive(u16),
    #[error("heer_config epoch is not configured")]
    MissingEpoch,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

And in `heeranjid/src/lib.rs`, gate the re-exports:

```rust
pub use error::Error;
#[cfg(feature = "postgres")]
pub use error::{GenerateError, StartupError};
```

(Replace the existing `pub use error::{Error, GenerateError, StartupError};` line.)

- [ ] **Step 5: Verify it compiles with and without the feature**

Run: `cargo build -p heeranjid --no-default-features`
Expected: SUCCESS (core types without sqlx)

Run: `cargo build -p heeranjid`
Expected: SUCCESS (full build with postgres feature)

---

### Task 3: Update CI and root config files

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.gitignore`

- [ ] **Step 1: Update CI workflow**

In `.github/workflows/ci.yml`, update the test commands to use workspace-level commands. The key changes:

Replace:
```yaml
      - name: cargo fmt --check
        run: cargo fmt --check

      - name: cargo clippy
        run: cargo clippy -- -D warnings

      - name: cargo deny check
        run: cargo deny check

      - name: cargo test (unit)
        run: cargo test --lib

      - name: cargo test (integration)
        run: cargo test --test postgres
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid

      - name: cargo test (concurrency)
        run: cargo test --test concurrency
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid
```

With:
```yaml
      - name: cargo fmt --check
        run: cargo fmt --all --check

      - name: cargo clippy
        run: cargo clippy --workspace -- -D warnings

      - name: cargo deny check
        run: cargo deny check

      - name: cargo test (unit)
        run: cargo test -p heeranjid --lib

      - name: cargo test (integration)
        run: cargo test -p heeranjid --test postgres
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid

      - name: cargo test (concurrency)
        run: cargo test -p heeranjid --test concurrency
        env:
          DATABASE_URL: postgres://postgres:postgres@postgres:5432/heeranjid
```

- [ ] **Step 2: Move deny.toml into heeranjid/ crate**

`cargo deny` by default looks for `deny.toml` at the workspace root, so it stays where it is. No move needed. Verify:

Run: `cargo deny check`
Expected: SUCCESS

- [ ] **Step 3: Verify the full CI test suite locally**

Run: `cargo fmt --all --check`
Expected: SUCCESS

Run: `cargo clippy --workspace -- -D warnings`
Expected: SUCCESS

Run: `cargo test -p heeranjid --lib`
Expected: All unit tests pass

Run (requires DATABASE_URL): `cargo test -p heeranjid --test postgres`
Expected: All integration tests pass

Run (requires DATABASE_URL): `cargo test -p heeranjid --test concurrency`
Expected: All concurrency tests pass

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: restructure into Cargo workspace with optional postgres feature

Move core crate into heeranjid/ subdirectory. Make sqlx an optional
dependency behind a 'postgres' feature flag so binding crates can
depend on core types without pulling in database dependencies."
```
