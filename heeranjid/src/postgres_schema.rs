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
