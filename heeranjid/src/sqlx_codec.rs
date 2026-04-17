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

#[cfg(test)]
mod tests {
    //! Unit tests for the codec. These do NOT hit Postgres — `PgValueRef`
    //! cannot be constructed without a live connection (its fields are
    //! crate-private in sqlx), so the Decode path is covered by the
    //! integration tests in `heeranjid-sqlx`. Here we assert:
    //!
    //! 1. `Type::type_info()` matches the underlying Postgres column type.
    //! 2. `Encode::encode_by_ref` writes the same byte buffer the underlying
    //!    primitive's `Encode` would — i.e. we're a transparent wrapper at
    //!    the wire level and not silently changing the column layout.
    //!
    //! Combined with the existing `from_i64` / `from_uuid` validation tests
    //! (UUIDv4/v7 rejection, negative HeerId rejection), this gives full
    //! coverage of every codec edge case without standing up a database.
    use super::*;
    use crate::RanjPrecision;
    use sqlx::Type;

    #[test]
    fn heerid_type_info_matches_bigint() {
        assert_eq!(
            <HeerId as Type<Postgres>>::type_info(),
            <i64 as Type<Postgres>>::type_info(),
        );
    }

    #[test]
    fn ranjid_type_info_matches_uuid() {
        assert_eq!(
            <RanjId as Type<Postgres>>::type_info(),
            <Uuid as Type<Postgres>>::type_info(),
        );
    }

    #[test]
    fn heerid_encode_matches_i64_wire_format() {
        let id = HeerId::new(1_234_567_890, 42, 7).unwrap();
        let raw: i64 = id.as_i64();

        let mut our_buf = PgArgumentBuffer::default();
        let _ = <HeerId as Encode<Postgres>>::encode_by_ref(&id, &mut our_buf).unwrap();

        let mut ref_buf = PgArgumentBuffer::default();
        let _ = <i64 as Encode<Postgres>>::encode_by_ref(&raw, &mut ref_buf).unwrap();

        assert_eq!(&**our_buf, &**ref_buf);
    }

    #[test]
    fn ranjid_encode_matches_uuid_wire_format() {
        let id = RanjId::new(42, RanjPrecision::Microseconds, 7, 1).unwrap();
        let raw: Uuid = id.as_uuid();

        let mut our_buf = PgArgumentBuffer::default();
        let _ = <RanjId as Encode<Postgres>>::encode_by_ref(&id, &mut our_buf).unwrap();

        let mut ref_buf = PgArgumentBuffer::default();
        let _ = <Uuid as Encode<Postgres>>::encode_by_ref(&raw, &mut ref_buf).unwrap();

        assert_eq!(&**our_buf, &**ref_buf);
    }
}
