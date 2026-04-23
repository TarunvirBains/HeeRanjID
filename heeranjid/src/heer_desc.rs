use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::heer::{HEER_FLIP_MASK, HeerId};
use crate::serde_helpers;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct HeerIdDesc(
    #[serde(
        serialize_with = "serde_helpers::serialize_display",
        deserialize_with = "serde_helpers::deserialize_from_str_or_int"
    )]
    i64,
);

impl HeerIdDesc {
    /// Construct from logical components; stored bits are flipped internally.
    pub fn new(timestamp_ms: u64, node_id: u16, sequence: u16) -> Result<Self, Error> {
        let asc = HeerId::new(timestamp_ms, node_id, sequence)?;
        Ok(Self(asc.as_i64() ^ HEER_FLIP_MASK))
    }

    /// Wrap raw stored (desc-form) bits. Returns `Err(NegativeHeerId)` if
    /// bit 63 is set — which never occurs for values produced through
    /// canonical paths (§4.1).
    pub fn from_i64(raw: i64) -> Result<Self, Error> {
        if raw < 0 {
            return Err(Error::NegativeHeerId);
        }
        Ok(Self(raw))
    }

    /// The stored bits (desc form); equal to the value the database holds.
    pub fn as_i64(self) -> i64 {
        self.0
    }

    /// Logical timestamp in milliseconds.
    pub fn timestamp_ms(self) -> u64 {
        HeerId::from_i64(self.0 ^ HEER_FLIP_MASK)
            .expect("desc→asc XOR preserves bit-63 = 0")
            .into_parts()
            .timestamp_ms
    }

    /// Node ID. Not flipped (node bits are untouched by the mask).
    pub fn node_id(self) -> u16 {
        HeerId::from_i64(self.0 ^ HEER_FLIP_MASK)
            .expect("desc→asc XOR preserves bit-63 = 0")
            .into_parts()
            .node_id
    }

    /// Logical sequence value.
    pub fn sequence(self) -> u16 {
        HeerId::from_i64(self.0 ^ HEER_FLIP_MASK)
            .expect("desc→asc XOR preserves bit-63 = 0")
            .into_parts()
            .sequence
    }
}

impl fmt::Display for HeerIdDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for HeerIdDesc {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw: i64 = s
            .parse()
            .map_err(|_| Error::InvalidHeerIdString(s.to_string()))?;
        Self::from_i64(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_round_trips_logical_components() {
        let id = HeerIdDesc::new(1_234_567, 42, 777).unwrap();
        assert_eq!(id.timestamp_ms(), 1_234_567);
        assert_eq!(id.node_id(), 42);
        assert_eq!(id.sequence(), 777);
    }

    #[test]
    fn stored_bits_differ_from_asc() {
        let asc = crate::HeerId::new(1_234_567, 42, 777).unwrap();
        let desc = HeerIdDesc::new(1_234_567, 42, 777).unwrap();
        assert_ne!(asc.as_i64(), desc.as_i64());
        assert_eq!(asc.as_i64() ^ desc.as_i64(), crate::heer::HEER_FLIP_MASK);
    }

    #[test]
    fn sorts_reverse_chronologically_by_stored_bits() {
        let a = HeerIdDesc::new(10, 0, 0).unwrap();
        let b = HeerIdDesc::new(20, 0, 0).unwrap();
        let c = HeerIdDesc::new(30, 0, 0).unwrap();
        let mut v = vec![a, b, c];
        v.sort();
        // Larger logical timestamps sort first.
        assert_eq!(v, vec![c, b, a]);
    }

    #[test]
    fn top_bit_zero_for_max_fields() {
        let id = HeerIdDesc::new(
            crate::HeerId::MAX_TIMESTAMP_MS,
            crate::HeerId::MAX_NODE_ID,
            crate::HeerId::MAX_SEQUENCE,
        )
        .unwrap();
        assert!(id.as_i64() >= 0, "bit 63 must be zero (§4.1)");
    }

    #[test]
    fn from_i64_rejects_negative() {
        assert_eq!(HeerIdDesc::from_i64(-1).unwrap_err(), Error::NegativeHeerId);
    }

    #[test]
    fn round_trip_at_boundaries() {
        for (ts, node, seq) in [
            (0, 0, 0),
            (crate::HeerId::MAX_TIMESTAMP_MS, 0, 0),
            (0, crate::HeerId::MAX_NODE_ID, 0),
            (0, 0, crate::HeerId::MAX_SEQUENCE),
            (
                crate::HeerId::MAX_TIMESTAMP_MS,
                crate::HeerId::MAX_NODE_ID,
                crate::HeerId::MAX_SEQUENCE,
            ),
        ] {
            let id = HeerIdDesc::new(ts, node, seq).unwrap();
            assert_eq!(id.timestamp_ms(), ts, "ts={ts}");
            assert_eq!(id.node_id(), node, "node={node}");
            assert_eq!(id.sequence(), seq, "seq={seq}");
            assert!(
                id.as_i64() >= 0,
                "bit-63 invariant at ({ts}, {node}, {seq})"
            );
        }
    }

    #[test]
    fn new_propagates_field_overflow_errors() {
        let err = HeerIdDesc::new(crate::HeerId::MAX_TIMESTAMP_MS + 1, 0, 0).unwrap_err();
        assert!(matches!(err, Error::TimestampOutOfRange { .. }));
    }

    #[test]
    fn display_and_from_str_round_trip() {
        let id = HeerIdDesc::new(1000, 5, 42).unwrap();
        let s = id.to_string();
        let parsed: HeerIdDesc = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_round_trip_via_string() {
        let id = HeerIdDesc::new(42, 7, 11).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{}\"", id.as_i64()));
        let parsed: HeerIdDesc = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }
}
