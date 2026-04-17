//! SQLx codec implementations for [`HeerId`] and [`RanjId`].
//!
//! # What
//! Implements `sqlx::Encode`, `sqlx::Decode`, and `sqlx::Type` for `HeerId`
//! and `RanjId` so that these types can be used directly in SQLx
//! `query_as` / `FromRow` APIs without manual `i64`/`Uuid` conversions.
//!
//! # How
//! - `HeerId` maps to the Postgres `BIGINT` (`i64`) column type.
//!   Encoding: `HeerId::as_i64()`. Decoding: `HeerId::from_i64(raw)?`.
//! - `RanjId` maps to the Postgres `UUID` column type.
//!   Encoding: `RanjId::as_uuid()`. Decoding: `RanjId::from_uuid(uuid)?`.
//!
//! # Why here
//! Rust's orphan rules require that at least one of the trait or type in an
//! impl is defined in the current crate. Since `HeerId` and `RanjId` are
//! defined in this crate, the impls live here behind the `sqlx` feature flag.
//! Enable with `heeranjid = { features = ["sqlx"] }`.

use sqlx::{
    Decode, Encode,
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef, Postgres},
};
use uuid::Uuid;

use crate::{HeerId, RanjId};

// ---------------------------------------------------------------------------
// HeerId — BIGINT
// ---------------------------------------------------------------------------

impl sqlx::Type<Postgres> for HeerId {
    fn type_info() -> PgTypeInfo {
        <i64 as sqlx::Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <i64 as sqlx::Type<Postgres>>::compatible(ty)
    }
}

impl<'r> Decode<'r, Postgres> for HeerId {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let raw = <i64 as Decode<Postgres>>::decode(value)?;
        HeerId::from_i64(raw).map_err(|e| Box::new(e) as BoxDynError)
    }
}

impl Encode<'_, Postgres> for HeerId {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <i64 as Encode<Postgres>>::encode_by_ref(&self.as_i64(), buf)
    }
}

// ---------------------------------------------------------------------------
// RanjId — UUID
// ---------------------------------------------------------------------------

impl sqlx::Type<Postgres> for RanjId {
    fn type_info() -> PgTypeInfo {
        <Uuid as sqlx::Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <Uuid as sqlx::Type<Postgres>>::compatible(ty)
    }
}

impl<'r> Decode<'r, Postgres> for RanjId {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let uuid = <Uuid as Decode<Postgres>>::decode(value)?;
        RanjId::from_uuid(uuid).map_err(|e| Box::new(e) as BoxDynError)
    }
}

impl Encode<'_, Postgres> for RanjId {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <Uuid as Encode<Postgres>>::encode_by_ref(&self.as_uuid(), buf)
    }
}
