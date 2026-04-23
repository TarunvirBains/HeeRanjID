//! Vendor-neutral primitives shared between [`postgres_schema`] and
//! [`mssql_schema`].
//!
//! These types and helpers do not depend on any database client crate,
//! so they can be referenced from both the Postgres and MSSQL schema
//! modules without forcing a caller to pull in `tokio-postgres` just to
//! generate MSSQL T-SQL strings.
//!
//! [`postgres_schema`]: crate::postgres_schema
//! [`mssql_schema`]: crate::mssql_schema

/// Error returned by the per-table autofill helpers.
///
/// Variants shared between vendors. `postgres_schema` wraps this into
/// its own `SchemaError` that also carries a `TokioPostgres` variant
/// for client-execution failures; `mssql_schema` uses this enum
/// directly because it only generates strings.
#[derive(Debug)]
pub enum SharedSchemaError {
    /// Caller passed a table/column name that failed [`validate_ident`]:
    /// empty, longer than 63 chars, or contains non-[A-Za-z0-9_] bytes
    /// (including the SQL-injection shapes `;`, `"`, `'`, whitespace,
    /// `--`). The offending value is carried for diagnostics only —
    /// callers should not echo it to untrusted clients.
    InvalidIdentifier(String),
}

impl std::fmt::Display for SharedSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SharedSchemaError::InvalidIdentifier(s) => {
                write!(f, "invalid SQL identifier: {s:?}")
            }
        }
    }
}

impl std::error::Error for SharedSchemaError {}

/// A source/destination column pair for a per-table autofill trigger.
///
/// `src` is the existing ascending-sort column, `dst` is the descending
/// sibling that the trigger keeps in sync. Multiple pairs may be passed
/// in one call to an installer; each gets an independent body in the
/// generated trigger.
pub struct ColumnPair<'a> {
    /// Source (ascending) column name.
    pub src: &'a str,
    /// Destination (descending) column name.
    pub dst: &'a str,
}

/// Which flip family the generated trigger should call:
/// `heerid_to_desc` (64-bit) or `ranjid_to_desc` (128-bit / bytea /
/// BINARY(16)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    /// HeerId (64-bit ascending -> descending via `heerid_to_desc`).
    Heer,
    /// RanjId (128-bit ascending -> descending via `ranjid_to_desc`).
    Ranj,
}

impl IdKind {
    /// Name of the vendor-agnostic function that flips an ascending id
    /// to its descending sibling. Both Postgres and MSSQL expose the
    /// same names so the emitter code can share this value.
    pub fn flip_fn(&self) -> &'static str {
        match self {
            IdKind::Heer => "heerid_to_desc",
            IdKind::Ranj => "ranjid_to_desc",
        }
    }
}

/// Validates that `s` is a safe, unquoted SQL identifier suitable for
/// interpolation into DDL.
///
/// Accepts `^[A-Za-z_][A-Za-z0-9_]*$` up to 63 bytes (the Postgres
/// `NAMEDATALEN - 1` ceiling; MSSQL's own limit is 128, but we keep the
/// tighter bound so generated scripts are portable between vendors).
/// Rejects everything else — including strings containing `;`, `"`,
/// `'`, whitespace, or `--`, which would open an SQL-injection channel
/// when interpolated into trigger DDL.
///
/// This is a belt-and-braces check: callers should never pass untrusted
/// input here, but if they do, we fail closed rather than exec arbitrary
/// DDL.
pub fn validate_ident(s: &str) -> Result<(), SharedSchemaError> {
    // 63 = Postgres NAMEDATALEN - 1. MSSQL allows up to 128, but we
    // keep the tighter Postgres bound for cross-vendor portability.
    if s.is_empty() || s.len() > 63 {
        return Err(SharedSchemaError::InvalidIdentifier(s.to_string()));
    }
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty checked above");
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(SharedSchemaError::InvalidIdentifier(s.to_string()));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(SharedSchemaError::InvalidIdentifier(s.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ident_rejects_sql_injection_attempts() {
        assert!(validate_ident("tbl; DROP TABLE users").is_err());
        assert!(validate_ident("\"quoted\"").is_err());
        assert!(validate_ident("it's").is_err());
        assert!(validate_ident("tbl--").is_err());
        assert!(validate_ident("two words").is_err());
        assert!(validate_ident("tab\tname").is_err());
        assert!(validate_ident("nl\nname").is_err());
        assert!(validate_ident("").is_err());
        assert!(validate_ident(&"x".repeat(64)).is_err());
        assert!(validate_ident("1tbl").is_err());
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
        assert!(validate_ident(&"a".repeat(63)).is_ok());
    }

    #[test]
    fn id_kind_flip_fn_matches_sql_names() {
        assert_eq!(IdKind::Heer.flip_fn(), "heerid_to_desc");
        assert_eq!(IdKind::Ranj.flip_fn(), "ranjid_to_desc");
    }
}
