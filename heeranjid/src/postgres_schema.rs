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

use crate::schema_shared::{self, SharedSchemaError};

pub use crate::schema_shared::{ColumnPair, IdKind};

/// Error returned by the per-table autofill trigger helpers (Task 11 of
/// the v0.3.0 descending-sort rollout).
///
/// The Task-10 install helpers (`install_flip_functions`,
/// `install_desc_generators`, `install_migration_support`,
/// `install_all_desc_support`) deliberately stay on
/// `tokio_postgres::Error` — they don't do any client-side validation.
/// The trigger helpers, by contrast, *must* reject malformed identifiers
/// before interpolating them into SQL, which doesn't fit cleanly into
/// the `tokio_postgres::Error` shape. Hence this local enum.
#[derive(Debug)]
pub enum SchemaError {
    /// Underlying Postgres client error.
    TokioPostgres(tokio_postgres::Error),
    /// Caller passed a table/column name that failed `validate_ident`:
    /// empty, longer than 63 chars, or contains non-[A-Za-z0-9_] bytes
    /// (including the SQL-injection shapes `;`, `"`, `'`, whitespace,
    /// `--`). The offending value is carried for diagnostics only —
    /// callers should not echo it to untrusted clients.
    InvalidIdentifier(String),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::TokioPostgres(e) => write!(f, "tokio-postgres error: {e}"),
            SchemaError::InvalidIdentifier(s) => {
                write!(f, "invalid Postgres identifier: {s:?}")
            }
        }
    }
}

impl std::error::Error for SchemaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SchemaError::TokioPostgres(e) => Some(e),
            SchemaError::InvalidIdentifier(_) => None,
        }
    }
}

impl From<tokio_postgres::Error> for SchemaError {
    fn from(e: tokio_postgres::Error) -> Self {
        SchemaError::TokioPostgres(e)
    }
}

impl From<SharedSchemaError> for SchemaError {
    fn from(e: SharedSchemaError) -> Self {
        match e {
            SharedSchemaError::InvalidIdentifier(s) => SchemaError::InvalidIdentifier(s),
        }
    }
}

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

// --- per-table autofill trigger helpers for v0.3.0 ---
//
// `ColumnPair` and `IdKind` are defined in `schema_shared` and
// re-exported at the top of this module so the public API stays stable
// from the pre-v0.3.1 layout. `validate_ident` is also in
// `schema_shared`; this module delegates to it.

fn validate_ident(s: &str) -> Result<(), SchemaError> {
    schema_shared::validate_ident(s).map_err(Into::into)
}

/// Installs a `BEFORE INSERT OR UPDATE` trigger that keeps descending
/// sibling columns in sync with their ascending sources.
///
/// The generated function is named `zzz_<table>_autofill_desc`. The
/// `zzz_` prefix is load-bearing — Postgres fires `BEFORE` triggers in
/// alphabetical order, and forcing this one to run last means any
/// user-defined trigger that wants to adjust `NEW.<src>` gets to do so
/// before the descending sibling is computed from it (spec §5.1).
///
/// For each `ColumnPair { src, dst }`:
/// * On `INSERT`: if `NEW.<dst> IS NULL`, fill it from `flip(NEW.<src>)`.
/// * On `UPDATE`: if `NEW.<src>` changed, recompute `NEW.<dst>`; else if
///   `NEW.<dst> IS NULL`, fill it from `flip(NEW.<src>)`.
///
/// Idempotent — re-running replaces both the function and the trigger.
///
/// # Errors
///
/// Returns [`SchemaError::InvalidIdentifier`] if `table` or any
/// `ColumnPair` field fails identifier validation (strict
/// `^[A-Za-z_][A-Za-z0-9_]*$`, max 63 chars);
/// [`SchemaError::TokioPostgres`] if the DDL `batch_execute` fails.
///
/// # Panics
///
/// Panics if `pairs` is empty — at least one pair is required.
pub async fn install_autofill_trigger_for_table<C>(
    client: &C,
    table: &str,
    pairs: &[ColumnPair<'_>],
    kind: IdKind,
) -> Result<(), SchemaError>
where
    C: GenericClient + ?Sized,
{
    assert!(!pairs.is_empty(), "at least one ColumnPair required");
    validate_ident(table)?;
    for p in pairs {
        validate_ident(p.src)?;
        validate_ident(p.dst)?;
    }

    let flip_fn = kind.flip_fn();
    let fn_name = format!("zzz_{}_autofill_desc", table);
    let trig_name = &fn_name;

    let mut insert_body = String::new();
    let mut update_body = String::new();
    for p in pairs {
        use std::fmt::Write as _;
        writeln!(
            insert_body,
            "        IF NEW.{dst} IS NULL THEN NEW.{dst} := {flip}(NEW.{src}); END IF;",
            dst = p.dst,
            flip = flip_fn,
            src = p.src,
        )
        .expect("write! to String cannot fail");
        write!(
            update_body,
            "        IF NEW.{src} IS DISTINCT FROM OLD.{src} THEN\n\
             \x20           NEW.{dst} := {flip}(NEW.{src});\n\
             \x20       ELSIF NEW.{dst} IS NULL THEN\n\
             \x20           NEW.{dst} := {flip}(NEW.{src});\n\
             \x20       END IF;\n",
            src = p.src,
            dst = p.dst,
            flip = flip_fn,
        )
        .expect("write! to String cannot fail");
    }

    let sql = format!(
        r#"
CREATE OR REPLACE FUNCTION {fn_name}() RETURNS trigger AS $body$
BEGIN
    IF TG_OP = 'INSERT' THEN
{insert_body}    ELSIF TG_OP = 'UPDATE' THEN
{update_body}    END IF;
    RETURN NEW;
END;
$body$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS {trig_name} ON {table};
CREATE TRIGGER {trig_name}
    BEFORE INSERT OR UPDATE ON {table}
    FOR EACH ROW EXECUTE FUNCTION {fn_name}();
"#,
        fn_name = fn_name,
        trig_name = trig_name,
        insert_body = insert_body,
        update_body = update_body,
        table = table,
    );

    client.batch_execute(&sql).await?;
    Ok(())
}

/// Removes the autofill trigger and underlying function installed by
/// [`install_autofill_trigger_for_table`] for `table`.
///
/// Safe to call when the trigger is not present (uses `IF EXISTS`).
///
/// # Errors
///
/// Returns [`SchemaError::InvalidIdentifier`] if `table` fails
/// identifier validation (strict `^[A-Za-z_][A-Za-z0-9_]*$`, max 63
/// chars); [`SchemaError::TokioPostgres`] on underlying DDL failure.
pub async fn drop_autofill_trigger_for_table<C>(client: &C, table: &str) -> Result<(), SchemaError>
where
    C: GenericClient + ?Sized,
{
    validate_ident(table)?;
    let fn_name = format!("zzz_{}_autofill_desc", table);
    let sql = format!(
        "DROP TRIGGER IF EXISTS {name} ON {tbl};\n\
         DROP FUNCTION IF EXISTS {name}() CASCADE;\n",
        name = fn_name,
        tbl = table,
    );
    client.batch_execute(&sql).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ident_rejects_sql_injection_attempts() {
        // Classic statement-terminator injection.
        assert!(validate_ident("tbl; DROP TABLE users").is_err());
        // Quote-based injection.
        assert!(validate_ident("\"quoted\"").is_err());
        assert!(validate_ident("it's").is_err());
        // Comment-based injection.
        assert!(validate_ident("tbl--").is_err());
        // Whitespace.
        assert!(validate_ident("two words").is_err());
        assert!(validate_ident("tab\tname").is_err());
        assert!(validate_ident("nl\nname").is_err());
        // Empty.
        assert!(validate_ident("").is_err());
        // Over 63 chars.
        assert!(validate_ident(&"x".repeat(64)).is_err());
        // Starts with a digit.
        assert!(validate_ident("1tbl").is_err());
        // Stray punctuation that isn't ; " ' but would still be wrong.
        assert!(validate_ident("tbl-name").is_err());
        assert!(validate_ident("tbl.name").is_err());
    }

    #[test]
    fn validate_ident_accepts_valid_identifiers() {
        assert!(validate_ident("tbl").is_ok());
        assert!(validate_ident("_internal_thing").is_ok());
        assert!(validate_ident("events_v2").is_ok());
        assert!(validate_ident("A").is_ok());
        assert!(validate_ident("_").is_ok());
        assert!(validate_ident("id_desc").is_ok());
        // Exactly 63 chars is OK (NAMEDATALEN - 1).
        assert!(validate_ident(&"a".repeat(63)).is_ok());
    }

    #[test]
    fn id_kind_flip_fn_matches_sql_names() {
        assert_eq!(IdKind::Heer.flip_fn(), "heerid_to_desc");
        assert_eq!(IdKind::Ranj.flip_fn(), "ranjid_to_desc");
    }
}
