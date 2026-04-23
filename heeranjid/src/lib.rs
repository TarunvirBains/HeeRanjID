//! Core HeeRanjID types.
//!
//! HeeRanjID is designed to let a project start on a single Postgres node with
//! a compact 8-byte integer primary key, and migrate to distributed writers
//! later without rewriting a single ID or schema. [`HeerId`] — a 64-bit
//! time-ordered integer whose layout already carries a `node_id` field —
//! is the primary type you reach for on day one. Going from one writer to
//! many later is a config change (allocate more `node_id` values, bind each
//! service's session); existing IDs stay valid.
//!
//! [`RanjId`] is the natural extension when `HeerId`'s capacity isn't enough
//! (more than 511 nodes, more than 8,191 IDs per node per millisecond, or
//! sub-millisecond timestamp precision). It's a UUIDv8-compatible 128-bit
//! identifier stored as `uuid`, and conversion from `HeerId` is lossless.
//!
//! [`HeerIdDesc`] and [`RanjIdDesc`] are reverse-chronologically-sorted
//! siblings: their raw-bit ordering matches a `DESC` scan, so newest-first
//! `ORDER BY id` is served directly by the primary key index without a
//! secondary descending index or a reverse scan. See the `asc-to-desc`
//! migration playbook at `docs/migrations/asc-to-desc.md` for the workflow
//! that flips an existing column under live writes.

mod convert;
mod error;
mod heer;
mod heer_desc;
pub mod mssql_schema;
#[cfg(feature = "postgres")]
pub mod postgres_codec;
#[cfg(feature = "postgres")]
pub mod postgres_generate;
#[cfg(feature = "postgres")]
pub mod postgres_schema;
mod precision;
mod ranj;
mod ranj_desc;
pub mod reverse_order;
pub mod schema_shared;
mod serde_helpers;
#[cfg(feature = "sqlx")]
mod sqlx_codec;

pub use convert::{ConflictKind, ConversionConflict, ConversionError};
pub use error::Error;
pub use heer::{HEER_NODE_ID_BITS, HEER_SEQUENCE_BITS, HEER_TIMESTAMP_BITS, HeerId, HeerIdParts};
pub use heer_desc::HeerIdDesc;
pub use precision::{RanjPrecision, generation_precision};
pub use ranj::{
    RANJ_NODE_ID_BITS, RANJ_PRECISION_BITS, RANJ_SEQUENCE_BITS, RANJ_TIMESTAMP_BITS, RanjId,
    RanjIdParts,
};
pub use ranj_desc::RanjIdDesc;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn heerid_round_trips_parts() {
        let id = HeerId::new(1_234_567, 42, 777).unwrap();
        let parts = id.into_parts();

        assert_eq!(parts.timestamp_ms, 1_234_567);
        assert_eq!(parts.node_id, 42);
        assert_eq!(parts.sequence, 777);
    }

    #[test]
    fn heerid_rejects_negative_raw_values() {
        let error = HeerId::from_i64(-1).unwrap_err();
        assert_eq!(error, Error::NegativeHeerId);
    }

    #[test]
    fn ranjid_round_trips_parts() {
        let id = RanjId::new(1_234_567_890_123, RanjPrecision::Microseconds, 513, 4096).unwrap();
        let parts = id.into_parts();

        assert_eq!(parts.timestamp, 1_234_567_890_123);
        assert_eq!(parts.precision, RanjPrecision::Microseconds);
        assert_eq!(parts.node_id, 513);
        assert_eq!(parts.sequence, 4096);
    }

    #[test]
    fn ranjid_validates_uuid_version_and_variant() {
        let random = Uuid::nil();
        let error = RanjId::from_uuid(random).unwrap_err();

        assert_eq!(error, Error::InvalidRanjIdVersion);
    }

    #[test]
    fn ranjid_rejects_uuid_v4() {
        // UUIDv4 (random) — common, but not what RanjId carries. Construct
        // by hand rather than `Uuid::new_v4()` so this crate doesn't need
        // the optional `v4` feature of `uuid` to compile its tests.
        // Version nibble at bits 76-79 = 0x4, variant at 62-63 = 0b10.
        let raw: u128 = (0x4u128 << 76) | (0x2u128 << 62);
        let v4 = Uuid::from_u128(raw);
        assert_eq!(v4.get_version_num(), 4);
        let error = RanjId::from_uuid(v4).unwrap_err();
        assert_eq!(error, Error::InvalidRanjIdVersion);
    }

    #[test]
    fn ranjid_rejects_uuid_v7() {
        // UUIDv7 is time-ordered too but is a distinct standard — RanjId is
        // UUIDv8 (custom layout with precision + node_id bits), so v7 inputs
        // must be rejected to avoid silently deserializing foreign data.
        // Construct a v7 by hand: version nibble at bits 76-79 = 0x7,
        // variant bits at 62-63 = 0b10 (RFC 4122).
        let raw: u128 = (0x7u128 << 76) | (0x2u128 << 62);
        let v7 = Uuid::from_u128(raw);
        assert_eq!(v7.get_version_num(), 7);
        let error = RanjId::from_uuid(v7).unwrap_err();
        assert_eq!(error, Error::InvalidRanjIdVersion);
    }

    #[test]
    fn heerid_orders_by_time_then_node_then_sequence() {
        let a = HeerId::new(10, 1, 1).unwrap();
        let b = HeerId::new(10, 1, 2).unwrap();
        let c = HeerId::new(10, 2, 0).unwrap();
        let d = HeerId::new(11, 0, 0).unwrap();

        assert!(a < b);
        assert!(b < c);
        assert!(c < d);
    }

    #[test]
    fn ranjid_orders_by_time_then_node_then_sequence() {
        let a = RanjId::new(10, RanjPrecision::Microseconds, 1, 1).unwrap();
        let b = RanjId::new(10, RanjPrecision::Microseconds, 1, 2).unwrap();
        let c = RanjId::new(10, RanjPrecision::Microseconds, 2, 0).unwrap();
        let d = RanjId::new(11, RanjPrecision::Microseconds, 0, 0).unwrap();

        assert!(a < b);
        assert!(b < c);
        assert!(c < d);
    }

    #[test]
    fn serde_serializes_heerid_as_a_string() {
        let id = HeerId::new(55, 7, 9).unwrap();
        let json = serde_json::to_string(&id).unwrap();

        assert_eq!(json, format!("\"{}\"", id.as_i64()));
    }

    #[test]
    fn serde_serializes_ranjid_as_a_string() {
        let id = RanjId::new(55, RanjPrecision::Microseconds, 7, 9).unwrap();
        let json = serde_json::to_string(&id).unwrap();

        assert_eq!(json, format!("\"{}\"", id.as_uuid()));
    }

    // ── HeerId boundary tests ──

    #[test]
    fn heerid_accepts_max_field_values() {
        let id = HeerId::new(
            HeerId::MAX_TIMESTAMP_MS,
            HeerId::MAX_NODE_ID,
            HeerId::MAX_SEQUENCE,
        )
        .unwrap();
        let parts = id.into_parts();
        assert_eq!(parts.timestamp_ms, HeerId::MAX_TIMESTAMP_MS);
        assert_eq!(parts.node_id, HeerId::MAX_NODE_ID);
        assert_eq!(parts.sequence, HeerId::MAX_SEQUENCE);
    }

    #[test]
    fn heerid_rejects_overflow_timestamp() {
        let err = HeerId::new(HeerId::MAX_TIMESTAMP_MS + 1, 0, 0).unwrap_err();
        assert!(matches!(err, Error::TimestampOutOfRange { .. }));
    }

    #[test]
    fn heerid_rejects_overflow_node_id() {
        let err = HeerId::new(0, HeerId::MAX_NODE_ID + 1, 0).unwrap_err();
        assert!(matches!(err, Error::NodeIdOutOfRange { .. }));
    }

    #[test]
    fn heerid_rejects_overflow_sequence() {
        let err = HeerId::new(0, 0, HeerId::MAX_SEQUENCE + 1).unwrap_err();
        assert!(matches!(err, Error::SequenceOutOfRange { .. }));
    }

    #[test]
    fn heerid_zero_round_trips() {
        let id = HeerId::new(0, 0, 0).unwrap();
        assert_eq!(id.as_i64(), 0);
        let parts = id.into_parts();
        assert_eq!(parts.timestamp_ms, 0);
        assert_eq!(parts.node_id, 0);
        assert_eq!(parts.sequence, 0);
    }

    #[test]
    fn heerid_from_str_round_trips() {
        let id = HeerId::new(1000, 5, 42).unwrap();
        let s = id.to_string();
        let parsed: HeerId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn heerid_from_str_rejects_negative() {
        let err = "-1".parse::<HeerId>().unwrap_err();
        assert_eq!(err, Error::NegativeHeerId);
    }

    #[test]
    fn heerid_from_str_rejects_garbage() {
        let err = "not_a_number".parse::<HeerId>().unwrap_err();
        assert!(matches!(err, Error::InvalidHeerIdString(_)));
    }

    // ── RanjId boundary tests ──

    #[test]
    fn ranjid_accepts_max_field_values() {
        let id = RanjId::new(
            RanjId::MAX_TIMESTAMP,
            RanjPrecision::Microseconds,
            RanjId::MAX_NODE_ID,
            RanjId::MAX_SEQUENCE,
        )
        .unwrap();
        let parts = id.into_parts();
        assert_eq!(parts.timestamp, RanjId::MAX_TIMESTAMP);
        assert_eq!(parts.node_id, RanjId::MAX_NODE_ID);
        assert_eq!(parts.sequence, RanjId::MAX_SEQUENCE);
    }

    #[test]
    fn ranjid_rejects_overflow_timestamp() {
        let err =
            RanjId::new(RanjId::MAX_TIMESTAMP + 1, RanjPrecision::Microseconds, 0, 0).unwrap_err();
        assert!(matches!(err, Error::TimestampOutOfRange { .. }));
    }

    #[test]
    fn ranjid_zero_round_trips() {
        let id = RanjId::new(0, RanjPrecision::Microseconds, 0, 0).unwrap();
        let parts = id.into_parts();
        assert_eq!(parts.timestamp, 0);
        assert_eq!(parts.node_id, 0);
        assert_eq!(parts.sequence, 0);
    }

    #[test]
    fn ranjid_from_str_round_trips() {
        let id = RanjId::new(1_000_000, RanjPrecision::Microseconds, 100, 200).unwrap();
        let s = id.to_string();
        let parsed: RanjId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn ranjid_from_str_rejects_garbage() {
        let err = "not-a-uuid".parse::<RanjId>().unwrap_err();
        assert!(matches!(err, Error::InvalidRanjIdString(_)));
    }

    #[test]
    fn ranjid_preserves_uuid_version_and_variant() {
        let id = RanjId::new(999_999, RanjPrecision::Microseconds, 42, 7).unwrap();
        let uuid = id.as_uuid();
        assert_eq!(uuid.get_version_num(), 8);
        assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
    }

    #[test]
    fn ranjid_precision_round_trips() {
        for prec in [
            RanjPrecision::Microseconds,
            RanjPrecision::Nanoseconds,
            RanjPrecision::Picoseconds,
            RanjPrecision::Femtoseconds,
        ] {
            let id = RanjId::new(1_000_000, prec, 100, 200).unwrap();
            let parts = id.into_parts();
            assert_eq!(parts.precision, prec);
            assert_eq!(parts.timestamp, 1_000_000);
            assert_eq!(parts.node_id, 100);
            assert_eq!(parts.sequence, 200);
        }
    }

    #[test]
    fn ranjid_timestamp_micros_converts_from_precision() {
        // 1000 nanoseconds = 1 microsecond
        let id = RanjId::new(1000, RanjPrecision::Nanoseconds, 1, 0).unwrap();
        assert_eq!(id.timestamp_micros(), 1); // 1000ns / 1000 = 1us
    }

    #[test]
    fn serde_deserializes_heerid_from_string() {
        let id = HeerId::new(55, 7, 9).unwrap();
        let json = format!("\"{}\"", id.as_i64());
        let parsed: HeerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_deserializes_heerid_from_integer() {
        let id = HeerId::new(55, 7, 9).unwrap();
        let json = id.as_i64().to_string();
        let parsed: HeerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }
}
