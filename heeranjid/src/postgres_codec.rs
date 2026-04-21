//! `postgres_types` codec implementations for [`HeerId`] and [`RanjId`].
//!
//! # What
//! Implements `postgres_types::ToSql` and `postgres_types::FromSql` for
//! `HeerId` and `RanjId` so that these types work directly with the
//! `tokio-postgres` / `deadpool-postgres` driver stack without manual
//! `i64` or `Uuid` conversions at the call site.
//!
//! # How
//! - `HeerId` maps to the Postgres `BIGINT` (`int8`) column type.
//!   Encoding: delegates to `<i64 as ToSql>` via `HeerId::as_i64()`.
//!   Decoding: delegates to `<i64 as FromSql>` then validates with
//!   `HeerId::from_i64`.
//! - `RanjId` maps to the Postgres `UUID` column type.
//!   Encoding: delegates to `<Uuid as ToSql>` via `RanjId::as_uuid()`.
//!   Decoding: delegates to `<Uuid as FromSql>` then validates with
//!   `RanjId::from_uuid`. Validation rejects non-UUIDv8 inputs
//!   (version or variant mismatch produces an error rather than silent
//!   corruption).
//!
//! # Why here
//! Rust's orphan rules require that at least one of the trait or type in an
//! impl is defined in the current crate. Since `HeerId` and `RanjId` are
//! defined in `heeranjid`, the codec impls live here and are gated behind
//! the `postgres` feature flag so callers that do not use this driver stack
//! pay no compile cost.

use postgres_types::{FromSql, IsNull, ToSql, Type, to_sql_checked};
use uuid::Uuid;

use crate::{HeerId, RanjId};

// ---------------------------------------------------------------------------
// HeerId — BIGINT (int8)
// ---------------------------------------------------------------------------

impl ToSql for HeerId {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        <i64 as ToSql>::to_sql(&self.as_i64(), ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for HeerId {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let value = <i64 as FromSql>::from_sql(ty, raw)?;
        HeerId::from_i64(value).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Sync + Send>)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }
}

// ---------------------------------------------------------------------------
// RanjId — UUID
// ---------------------------------------------------------------------------

impl ToSql for RanjId {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        <Uuid as ToSql>::to_sql(&self.as_uuid(), ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::UUID
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for RanjId {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let uuid = <Uuid as FromSql>::from_sql(ty, raw)?;
        RanjId::from_uuid(uuid).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Sync + Send>)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::UUID
    }
}
