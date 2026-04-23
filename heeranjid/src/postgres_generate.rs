//! ID generation helpers for the `tokio-postgres` stack.
//!
//! # What
//!
//! Thin async wrappers over the installed HeeRanjID SQL generation
//! functions — [`generate_heerid`], [`generate_ranjid`], and their
//! batch counterparts [`generate_heerids`] / [`generate_ranjids`].
//! Each helper takes a [`tokio_postgres::GenericClient`], a `node_id`,
//! and (for batch variants) a row count; it runs the appropriate
//! `SELECT generate_id(...)` / `SELECT id FROM generate_ids(...)` query
//! and deserialises the result into the matching [`crate::HeerId`] or
//! [`crate::RanjId`] type.
//!
//! All helpers return [`GenerateError`], which wraps the underlying
//! [`tokio_postgres::Error`] for transport failures and carries a
//! [`crate::Error`] source for the (unlikely but possible) case where
//! the database returns a bit pattern that fails HeerId / RanjId
//! validation.
//!
//! # Why here
//!
//! The `postgres_schema` module covers database-wide bootstrap
//! (installing DDL, seeding the default node). This module covers the
//! per-request generation path. Both are gated on the `postgres`
//! feature and require `tokio-postgres` at runtime.
//!
//! The sibling [`heeranjid-sqlx`](https://docs.rs/heeranjid-sqlx) crate
//! offers the same four helpers over `sqlx::Executor`; this module is
//! the `tokio-postgres` parity surface so applications on the
//! `postgres` feature don't have to hand-write `client.query_one(
//! "SELECT generate_id($1)", &[&node_id_i32])` calls. Public
//! signatures, naming, and error-handling behaviour match the sqlx
//! counterparts exactly (only the executor bound and error type
//! differ).

use tokio_postgres::GenericClient;

use crate::{HeerId, RanjId};

fn map_pg_error(err: tokio_postgres::Error) -> GenerateError {
    if let Some(db_err) = err.as_db_error() {
        let message = db_err.message().to_string();
        match db_err.code().code() {
            "50021" => return GenerateError::LogicalDrift { message },
            "50020" => return GenerateError::ClockRollback { message },
            "50022" => return GenerateError::HardClockRollback { message },
            _ => {}
        }
    }
    GenerateError::Database(err)
}

/// Error returned by the [`postgres_generate`](self) helpers.
///
/// Shape mirrors `heeranjid_sqlx::GenerateError` so callers can swap
/// between the two client stacks without restructuring their error
/// handling. Transport and SQL-level failures surface through
/// [`GenerateError::Database`]; the `Invalid*` variants fire only when
/// the database hands back a value that doesn't parse as a valid
/// HeerId / RanjId (a corruption or schema-mismatch signal, not a
/// normal operational error).
#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    /// The database returned an `i64` that failed
    /// [`HeerId::from_i64`] validation (e.g. a negative raw value).
    #[error("database returned invalid HeerId: {0}")]
    InvalidHeerId(#[source] crate::Error),
    /// The database returned a `uuid` that failed
    /// [`RanjId::from_uuid`] validation (wrong version or variant).
    #[error("database returned invalid RanjId: {0}")]
    InvalidRanjId(#[source] crate::Error),
    /// Logical future drift detected (batch-induced, < 2ms / < 2000µs).
    #[error("logical future drift (batch-induced): {message}")]
    LogicalDrift { message: String },
    /// Soft clock rollback detected (2-50ms / 2000-50000µs).
    #[error("clock rollback: {message}")]
    ClockRollback { message: String },
    /// Hard clock rollback detected (>= 50ms / >= 50000µs).
    #[error("hard clock rollback: {message}")]
    HardClockRollback { message: String },
    /// Underlying `tokio-postgres` transport or SQL error.
    #[error("database error: {0}")]
    Database(#[from] tokio_postgres::Error),
}

/// Generates a single [`HeerId`] for `node_id` by calling the installed
/// `generate_id(INTEGER)` SQL function.
///
/// The SQL function allocates a timestamp/sequence pair for the given
/// node and returns a `BIGINT`; this helper parses that integer through
/// [`HeerId::from_i64`] and returns the validated type.
///
/// # Errors
///
/// * [`GenerateError::Database`] on transport or SQL-level failures
///   (node not registered, sequence exhausted, etc.).
/// * [`GenerateError::InvalidHeerId`] if the returned integer fails
///   validation — indicates schema / client version drift.
pub async fn generate_heerid<C>(client: &C, node_id: u16) -> Result<HeerId, GenerateError>
where
    C: GenericClient + ?Sized,
{
    let row = client
        .query_one("SELECT generate_id($1)", &[&i32::from(node_id)])
        .await
        .map_err(map_pg_error)?;
    let raw: i64 = row.get(0);
    HeerId::from_i64(raw).map_err(GenerateError::InvalidHeerId)
}

/// Generates a single [`RanjId`] for `node_id` by calling the installed
/// `generate_ranjid(INTEGER)` SQL function.
///
/// The SQL function returns a `UUID` encoding the timestamp,
/// precision, node, and sequence fields; this helper validates the
/// version/variant bits through [`RanjId::from_uuid`] and returns the
/// typed value.
///
/// # Errors
///
/// * [`GenerateError::Database`] on transport or SQL-level failures.
/// * [`GenerateError::InvalidRanjId`] if the returned UUID is not a
///   valid RanjId (wrong version or variant bits) — indicates schema /
///   client version drift.
pub async fn generate_ranjid<C>(client: &C, node_id: u16) -> Result<RanjId, GenerateError>
where
    C: GenericClient + ?Sized,
{
    let row = client
        .query_one("SELECT generate_ranjid($1)", &[&i32::from(node_id)])
        .await
        .map_err(map_pg_error)?;
    let uuid: uuid::Uuid = row.get(0);
    RanjId::from_uuid(uuid).map_err(GenerateError::InvalidRanjId)
}

/// Generates `count` [`HeerId`]s for `node_id` in a single round-trip
/// by calling the installed `generate_ids(INTEGER, INTEGER)` SQL
/// function.
///
/// Returns the IDs in the order the SQL function emits them
/// (time-ordered, ascending). Each row is validated through
/// [`HeerId::from_i64`]; any invalid row short-circuits the result.
///
/// # Errors
///
/// * [`GenerateError::Database`] on transport or SQL-level failures.
/// * [`GenerateError::InvalidHeerId`] if any returned integer fails
///   validation.
pub async fn generate_heerids<C>(
    client: &C,
    node_id: u16,
    count: i32,
) -> Result<Vec<HeerId>, GenerateError>
where
    C: GenericClient + ?Sized,
{
    // Explicit `::integer` casts disambiguate against the
    // `generate_ids(integer, boolean)` overload — without them tokio-
    // postgres sends parameters as unknown-typed placeholders and
    // Postgres can't choose between the 2-arg (integer, boolean) form
    // and the 3-arg (integer, integer, boolean DEFAULT true) form.
    let rows = client
        .query(
            "SELECT id FROM generate_ids($1::integer, $2::integer)",
            &[&i32::from(node_id), &count],
        )
        .await
        .map_err(map_pg_error)?;
    rows.into_iter()
        .map(|row| {
            let raw: i64 = row.get(0);
            HeerId::from_i64(raw).map_err(GenerateError::InvalidHeerId)
        })
        .collect()
}

/// Generates `count` [`RanjId`]s for `node_id` in a single round-trip
/// by calling the installed `generate_ranjids(INTEGER, INTEGER)` SQL
/// function.
///
/// Returns the IDs in the order the SQL function emits them
/// (time-ordered, ascending). Each row is validated through
/// [`RanjId::from_uuid`]; any invalid row short-circuits the result.
///
/// # Errors
///
/// * [`GenerateError::Database`] on transport or SQL-level failures.
/// * [`GenerateError::InvalidRanjId`] if any returned UUID fails
///   validation.
pub async fn generate_ranjids<C>(
    client: &C,
    node_id: u16,
    count: i32,
) -> Result<Vec<RanjId>, GenerateError>
where
    C: GenericClient + ?Sized,
{
    // Explicit `::integer` casts disambiguate against the
    // `generate_ranjids(integer, boolean)` overload — see the comment
    // on `generate_heerids` above for details.
    let rows = client
        .query(
            "SELECT id FROM generate_ranjids($1::integer, $2::integer)",
            &[&i32::from(node_id), &count],
        )
        .await
        .map_err(map_pg_error)?;
    rows.into_iter()
        .map(|row| {
            let uuid: uuid::Uuid = row.get(0);
            RanjId::from_uuid(uuid).map_err(GenerateError::InvalidRanjId)
        })
        .collect()
}
