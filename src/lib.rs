mod error;
mod heer;
mod postgres;
mod ranj;
mod serde_helpers;

pub use error::Error;
pub use heer::{HEER_NODE_ID_BITS, HEER_SEQUENCE_BITS, HEER_TIMESTAMP_BITS, HeerId, HeerIdParts};
pub use postgres::{
    FETCH_EPOCH_SQL, FETCH_NODE_SQL, HeerConfig, HeerNode, SCHEMA_SQL, fetch_epoch, fetch_node,
    install_schema, validate_heer_node_id,
};
pub use ranj::{
    RANJ_NODE_ID_BITS, RANJ_SEQUENCE_BITS, RANJ_TIMESTAMP_BITS, RanjId, RanjIdParts,
};

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
}
