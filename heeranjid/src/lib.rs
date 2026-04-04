mod error;
mod heer;
mod ranj;
mod serde_helpers;

pub use error::Error;
pub use heer::{HEER_NODE_ID_BITS, HEER_SEQUENCE_BITS, HEER_TIMESTAMP_BITS, HeerId, HeerIdParts};
pub use ranj::{RANJ_NODE_ID_BITS, RANJ_SEQUENCE_BITS, RANJ_TIMESTAMP_BITS, RanjId, RanjIdParts};

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
        let id = RanjId::new(1_234_567_890_123, 513, 4096).unwrap();
        let parts = id.into_parts();

        assert_eq!(parts.timestamp_micros, 1_234_567_890_123);
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
        let a = RanjId::new(10, 1, 1).unwrap();
        let b = RanjId::new(10, 1, 2).unwrap();
        let c = RanjId::new(10, 2, 0).unwrap();
        let d = RanjId::new(11, 0, 0).unwrap();

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
        let id = RanjId::new(55, 7, 9).unwrap();
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
            RanjId::MAX_TIMESTAMP_MICROS,
            RanjId::MAX_NODE_ID,
            RanjId::MAX_SEQUENCE,
        )
        .unwrap();
        let parts = id.into_parts();
        assert_eq!(parts.timestamp_micros, RanjId::MAX_TIMESTAMP_MICROS);
        assert_eq!(parts.node_id, RanjId::MAX_NODE_ID);
        assert_eq!(parts.sequence, RanjId::MAX_SEQUENCE);
    }

    #[test]
    fn ranjid_rejects_overflow_timestamp() {
        let err = RanjId::new(RanjId::MAX_TIMESTAMP_MICROS + 1, 0, 0).unwrap_err();
        assert!(matches!(err, Error::TimestampOutOfRange { .. }));
    }

    #[test]
    fn ranjid_zero_round_trips() {
        let id = RanjId::new(0, 0, 0).unwrap();
        let parts = id.into_parts();
        assert_eq!(parts.timestamp_micros, 0);
        assert_eq!(parts.node_id, 0);
        assert_eq!(parts.sequence, 0);
    }

    #[test]
    fn ranjid_from_str_round_trips() {
        let id = RanjId::new(1_000_000, 100, 200).unwrap();
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
        let id = RanjId::new(999_999, 42, 7).unwrap();
        let uuid = id.as_uuid();
        assert_eq!(uuid.get_version_num(), 7);
        assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
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
