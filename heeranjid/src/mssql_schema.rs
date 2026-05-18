//! MSSQL schema helpers for the v0.3.0 descending-sort variants.
//!
//! These helpers generate T-SQL strings; they do not execute them.
//! Callers run the returned SQL through their own MSSQL client —
//! `pyodbc` from Python, `tiberius` from Rust, or a deploy script via
//! `sqlcmd`. The module has no runtime MSSQL dependency and emits
//! only static T-SQL strings; it is always compiled and carries no
//! optional feature flag.
//!
//! Mirrors the surface of `postgres_schema` for the asc↔desc
//! migration workflow. See `docs/migrations/asc-to-desc-mssql.md` for
//! the operator playbook.

use crate::schema_shared::{ColumnPair, IdKind, SharedSchemaError, validate_ident};

pub use crate::schema_shared::{ColumnPair as MssqlColumnPair, IdKind as MssqlIdKind};

/// Static T-SQL for the flip primitives (asc↔desc scalar functions).
pub const DESC_FLIP_TSQL: &str = include_str!("../sql/mssql/procedures/desc_flip.sql");

/// Static T-SQL for the `heerid_next_desc` / `ranjid_next_desc`
/// generator stored procedures.
pub const DESC_GENERATORS_TSQL: &str = include_str!("../sql/mssql/procedures/desc_generators.sql");

/// Static T-SQL for the `heeranjid_bulk_backfill` migration procedure.
pub const BULK_BACKFILL_TSQL: &str = include_str!("../sql/mssql/procedures/bulk_backfill.sql");

/// Convenience: all three SQL blobs concatenated with `GO` separators
/// so a single `sqlcmd` / `sp_executesql` invocation installs the full
/// v0.3.1 MSSQL desc surface.
pub const INSTALL_ALL_DESC_TSQL: &str = concat!(
    include_str!("../sql/mssql/procedures/desc_flip.sql"),
    "\n",
    include_str!("../sql/mssql/procedures/desc_generators.sql"),
    "\n",
    include_str!("../sql/mssql/procedures/bulk_backfill.sql"),
);

/// Generates T-SQL to install a per-table autofill trigger that keeps
/// descending sibling columns in sync with their ascending sources.
///
/// Returns a multi-statement T-SQL script (`GO`-separated batches) that
/// the caller should execute via their MSSQL client. The emitted
/// script:
///
/// 1. `CREATE OR ALTER TRIGGER zzz_<table>_autofill_desc` — one
///    trigger, fires `AFTER INSERT, UPDATE`.
/// 2. `EXEC sp_settriggerorder @order = 'Last' @stmttype = 'INSERT'`
/// 3. `EXEC sp_settriggerorder @order = 'Last' @stmttype = 'UPDATE'`
///
/// # Semantics
///
/// For each [`ColumnPair`] `{ src, dst }`, the trigger recomputes
/// `dst = flip(src)` (or NULL if src is NULL) whenever the row's src
/// appears in `inserted`. The recompute is idempotent — applying it to
/// an unchanged row produces the same dst — so the trigger is safe to
/// re-enter during replication, bulk inserts, etc.
///
/// # Assumptions
///
/// The trigger matches `t.src IN (SELECT src FROM inserted)` rather
/// than joining on a PK. That is correct when `src` is effectively
/// unique within the table (the typical case — src is the primary key
/// or part of a unique index in an asc-to-desc migration). If `src` is
/// non-unique, all rows sharing a given src value will have their dst
/// recomputed on each touch; the result remains correct but with more
/// writes than minimal. Callers with non-unique src columns should
/// write a hand-tuned trigger keyed on the PK.
///
/// # Errors
///
/// Returns [`SharedSchemaError::InvalidIdentifier`] if `table` or any
/// [`ColumnPair`] field fails identifier validation (strict
/// `^[A-Za-z_][A-Za-z0-9_]*$`, max 63 chars). The returned `Err` carries
/// the offending value for diagnostics; callers should not echo it
/// back to untrusted clients.
///
/// # Panics
///
/// Panics if `pairs` is empty — at least one pair is required.
pub fn install_autofill_trigger_for_table_mssql(
    table: &str,
    pairs: &[ColumnPair<'_>],
    kind: IdKind,
) -> Result<String, SharedSchemaError> {
    assert!(!pairs.is_empty(), "at least one ColumnPair required");
    validate_ident(table)?;
    for p in pairs {
        validate_ident(p.src)?;
        validate_ident(p.dst)?;
    }

    let flip_fn = kind.flip_fn();
    let trig_name = format!("zzz_{}_autofill_desc", table);

    // Emit one UPDATE per (src, dst) pair inside a single trigger
    // body. Each update is idempotent — applying flip(src) twice is a
    // no-op.
    let mut body = String::new();
    for p in pairs {
        use std::fmt::Write as _;
        writeln!(
            body,
            "    UPDATE t\n    \
             SET t.{dst} = CASE WHEN t.{src} IS NULL THEN NULL ELSE dbo.{flip_fn}(t.{src}) END\n    \
             FROM {table} AS t\n    \
             WHERE t.{src} IN (SELECT {src} FROM inserted WHERE {src} IS NOT NULL);",
            dst = p.dst,
            src = p.src,
            flip_fn = flip_fn,
            table = table,
        )
        .expect("write! into String cannot fail");
    }

    let script = format!(
        "CREATE OR ALTER TRIGGER {trig_name}\n\
         ON {table}\n\
         AFTER INSERT, UPDATE\n\
         AS\n\
         BEGIN\n    \
         SET NOCOUNT ON;\n\
         {body}\
         END;\n\
         GO\n\
         EXEC sp_settriggerorder @triggername = N'{trig_name}', @order = 'Last', @stmttype = 'INSERT';\n\
         GO\n\
         EXEC sp_settriggerorder @triggername = N'{trig_name}', @order = 'Last', @stmttype = 'UPDATE';\n\
         GO\n",
        trig_name = trig_name,
        table = table,
        body = body,
    );
    Ok(script)
}

/// Generates T-SQL to drop the autofill trigger installed by
/// [`install_autofill_trigger_for_table_mssql`].
///
/// The emitted script is idempotent — no-op if the trigger is absent.
///
/// # Errors
///
/// Returns [`SharedSchemaError::InvalidIdentifier`] if `table` fails
/// identifier validation.
pub fn drop_autofill_trigger_for_table_mssql(table: &str) -> Result<String, SharedSchemaError> {
    validate_ident(table)?;
    let trig_name = format!("zzz_{}_autofill_desc", table);
    Ok(format!(
        "IF OBJECT_ID(N'{trig_name}', N'TR') IS NOT NULL DROP TRIGGER {trig_name};\nGO\n",
        trig_name = trig_name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_script_contains_required_statements() {
        let script = install_autofill_trigger_for_table_mssql(
            "events",
            &[ColumnPair {
                src: "id",
                dst: "id_desc",
            }],
            IdKind::Heer,
        )
        .unwrap();
        assert!(script.contains("CREATE OR ALTER TRIGGER zzz_events_autofill_desc"));
        assert!(script.contains("AFTER INSERT, UPDATE"));
        assert!(script.contains("sp_settriggerorder"));
        assert!(script.contains("@stmttype = 'INSERT'"));
        assert!(script.contains("@stmttype = 'UPDATE'"));
        assert!(script.contains("dbo.heerid_to_desc"));
    }

    #[test]
    fn install_script_uses_ranjid_flip_for_ranj_kind() {
        let script = install_autofill_trigger_for_table_mssql(
            "audit_log",
            &[ColumnPair {
                src: "id",
                dst: "id_desc",
            }],
            IdKind::Ranj,
        )
        .unwrap();
        assert!(script.contains("dbo.ranjid_to_desc"));
        assert!(!script.contains("dbo.heerid_to_desc"));
    }

    #[test]
    fn multi_pair_emits_branch_per_pair() {
        let script = install_autofill_trigger_for_table_mssql(
            "nodes",
            &[
                ColumnPair {
                    src: "id",
                    dst: "id_desc",
                },
                ColumnPair {
                    src: "parent_id",
                    dst: "parent_id_desc",
                },
            ],
            IdKind::Heer,
        )
        .unwrap();
        assert_eq!(script.matches("UPDATE t").count(), 2);
        assert!(script.contains("id_desc"));
        assert!(script.contains("parent_id_desc"));
    }

    #[test]
    fn rejects_invalid_table_identifier() {
        let err = install_autofill_trigger_for_table_mssql(
            "tbl; DROP TABLE users",
            &[ColumnPair {
                src: "id",
                dst: "id_desc",
            }],
            IdKind::Heer,
        )
        .unwrap_err();
        match err {
            SharedSchemaError::InvalidIdentifier(s) => {
                assert!(s.contains("DROP"));
            }
        }
    }

    #[test]
    fn rejects_invalid_column_identifier() {
        let err = install_autofill_trigger_for_table_mssql(
            "events",
            &[ColumnPair {
                src: "id",
                dst: "id_desc; --",
            }],
            IdKind::Heer,
        )
        .unwrap_err();
        assert!(matches!(err, SharedSchemaError::InvalidIdentifier(_)));
    }

    #[test]
    #[should_panic(expected = "at least one ColumnPair required")]
    fn empty_pairs_panics() {
        let _ = install_autofill_trigger_for_table_mssql("events", &[], IdKind::Heer);
    }

    #[test]
    fn drop_script_is_idempotent_guarded() {
        let script = drop_autofill_trigger_for_table_mssql("events").unwrap();
        assert!(script.contains("IF OBJECT_ID"));
        assert!(script.contains("DROP TRIGGER zzz_events_autofill_desc"));
    }

    #[test]
    fn drop_rejects_invalid_identifier() {
        let err = drop_autofill_trigger_for_table_mssql("bad name").unwrap_err();
        assert!(matches!(err, SharedSchemaError::InvalidIdentifier(_)));
    }

    #[test]
    fn install_all_desc_tsql_has_all_three_blobs() {
        assert!(INSTALL_ALL_DESC_TSQL.contains("CREATE OR ALTER FUNCTION dbo.heerid_flip_mask"));
        assert!(INSTALL_ALL_DESC_TSQL.contains("CREATE OR ALTER PROCEDURE dbo.heerid_next_desc"));
        assert!(
            INSTALL_ALL_DESC_TSQL.contains("CREATE OR ALTER PROCEDURE dbo.heeranjid_bulk_backfill")
        );
    }

    #[test]
    fn type_aliases_exported() {
        // MssqlColumnPair / MssqlIdKind re-export the shared types so
        // downstream code that prefers the vendor-prefixed names can
        // use them without importing from schema_shared.
        let _pair: MssqlColumnPair<'_> = MssqlColumnPair {
            src: "id",
            dst: "id_desc",
        };
        let _kind: MssqlIdKind = MssqlIdKind::Heer;
    }
}
