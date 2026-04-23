//! Schema installation and seed helpers for the `tokio-postgres` stack.
//!
//! # What
//!
//! Exposes the HeeRanjID DDL and seed SQL as `pub const` blobs and offers
//! convenience async helpers that run them through a
//! [`tokio_postgres::GenericClient`]. Intended for test harnesses and
//! application bootstrap paths that want to install HeeRanjID's schema
//! without depending on the `heeranjid-sqlx` crate.
//!
//! # Why here
//!
//! The `postgres_codec` module covers per-row type coercion; this module
//! covers database-wide bootstrap. Both are gated on the `postgres`
//! feature and require `tokio-postgres` at runtime.

use tokio_postgres::GenericClient;

/// Core `heer` schema DDL — tables, domains, and base types.
pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

/// Session-local node-id helpers.
pub const SESSION_SQL: &str = include_str!("../sql/functions/session.sql");

/// `generate_id()` / HeerId generation function.
pub const GENERATE_HEERID_SQL: &str = include_str!("../sql/functions/generate_heerid.sql");

/// `generate_ranj_id()` / RanjId generation function.
pub const GENERATE_RANJID_SQL: &str = include_str!("../sql/functions/generate_ranjid.sql");

/// Complete install blob — schema + all function definitions, in
/// dependency order. Equivalent to executing `SCHEMA_SQL`,
/// `SESSION_SQL`, `GENERATE_HEERID_SQL`, and `GENERATE_RANJID_SQL` in
/// sequence.
pub const INSTALL_SQL: &str = concat!(
    include_str!("../sql/schema.sql"),
    "\n",
    include_str!("../sql/functions/session.sql"),
    "\n",
    include_str!("../sql/functions/generate_heerid.sql"),
    "\n",
    include_str!("../sql/functions/generate_ranjid.sql"),
);

/// Seed SQL — inserts the default node row (node_id = 1).
pub const SEED_SQL: &str = include_str!("../sql/seed.sql");

// --- flip/generator/backfill install helpers for v0.3.0 ---

/// Flip primitives: `heerid_flip_mask`, `heerid_to_desc`/`heerid_to_asc`,
/// `ranjid_to_desc`/`ranjid_to_asc`. (§5.1)
pub const DESC_FLIP_SQL: &str = include_str!("../sql/functions/desc_flip.sql");

/// Single-row generators + desc generators: `heerid_next`, `ranjid_next`,
/// `heerid_next_desc`, `ranjid_next_desc`. (§5.1)
pub const DESC_GENERATORS_SQL: &str = include_str!("../sql/functions/desc_generators.sql");

/// Migration-support procedure: `heeranjid_bulk_backfill`. (§5.1)
pub const BULK_BACKFILL_SQL: &str = include_str!("../sql/functions/bulk_backfill.sql");

/// Install the HeeRanjID schema + functions on the target database.
///
/// Runs [`INSTALL_SQL`] via `client.batch_execute`. Idempotent in the
/// sense that all DDL uses `CREATE OR REPLACE` / `CREATE ... IF NOT
/// EXISTS`, so re-running against an already-installed database is a
/// no-op.
pub async fn install_schema<C>(client: &C) -> Result<(), tokio_postgres::Error>
where
    C: GenericClient + ?Sized,
{
    client.batch_execute(INSTALL_SQL).await
}

/// Seed the default node row (node_id = 1).
///
/// Runs [`SEED_SQL`] via `client.batch_execute`. Intended for test
/// setups and single-node development installs; production deployments
/// typically seed node_id at provisioning time rather than calling this.
pub async fn seed_default_node<C>(client: &C) -> Result<(), tokio_postgres::Error>
where
    C: GenericClient + ?Sized,
{
    client.batch_execute(SEED_SQL).await
}

// --- flip/generator/backfill install helpers for v0.3.0 ---

/// Installs the asc↔desc flip functions. Idempotent.
pub async fn install_flip_functions<C>(client: &C) -> Result<(), tokio_postgres::Error>
where
    C: GenericClient + ?Sized,
{
    client.batch_execute(DESC_FLIP_SQL).await
}

/// Installs `heerid_next` / `ranjid_next` single-row wrappers plus the
/// `*_next_desc` generators. Requires the base `generate_ids` /
/// `generate_ranj_ids` functions to already be present (v0.2.x schema).
pub async fn install_desc_generators<C>(client: &C) -> Result<(), tokio_postgres::Error>
where
    C: GenericClient + ?Sized,
{
    client.batch_execute(DESC_GENERATORS_SQL).await
}

/// Installs the `heeranjid_bulk_backfill` procedure. Does not install
/// per-table triggers — those go through `install_autofill_trigger_for_table`.
pub async fn install_migration_support<C>(client: &C) -> Result<(), tokio_postgres::Error>
where
    C: GenericClient + ?Sized,
{
    client.batch_execute(BULK_BACKFILL_SQL).await
}

/// Convenience: runs [`install_flip_functions`], [`install_desc_generators`],
/// and [`install_migration_support`] in order. Idempotent.
pub async fn install_all_desc_support<C>(client: &C) -> Result<(), tokio_postgres::Error>
where
    C: GenericClient + ?Sized,
{
    install_flip_functions(client).await?;
    install_desc_generators(client).await?;
    install_migration_support(client).await?;
    Ok(())
}
