use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Error;
use crate::precision::RanjPrecision;
use crate::ranj::{RANJ_FLIP_MASK, RanjId};
use crate::serde_helpers;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct RanjIdDesc(
    #[serde(
        serialize_with = "serde_helpers::serialize_display",
        deserialize_with = "serde_helpers::deserialize_from_str_or_int"
    )]
    Uuid,
);

impl RanjIdDesc {
    pub fn new(
        timestamp: u128,
        precision: RanjPrecision,
        node_id: u16,
        sequence: u16,
    ) -> Result<Self, Error> {
        let asc = RanjId::new(timestamp, precision, node_id, sequence)?;
        let flipped = asc.as_uuid().as_u128() ^ RANJ_FLIP_MASK;
        Ok(Self(Uuid::from_u128(flipped)))
    }

    /// Wrap a raw uuid; validates preserved version/variant bits (§4.2).
    pub fn from_uuid(u: Uuid) -> Result<Self, Error> {
        let raw = u.as_u128();
        let version = ((raw >> 76) & 0xF) as u8;
        let variant = ((raw >> 62) & 0x3) as u8;
        if version != crate::ranj::RANJ_UUID_VERSION {
            return Err(Error::InvalidRanjIdVersion);
        }
        if variant != crate::ranj::RANJ_UUID_VARIANT {
            return Err(Error::InvalidRanjIdVariant);
        }
        Ok(Self(u))
    }

    /// Stored bits; equal to what Postgres holds.
    pub fn as_uuid(self) -> Uuid {
        self.0
    }

    pub fn timestamp(self) -> u128 {
        RanjId::from_uuid(Uuid::from_u128(self.0.as_u128() ^ RANJ_FLIP_MASK))
            .expect("desc→asc XOR preserves version and variant")
            .into_parts()
            .timestamp
    }

    pub fn precision(self) -> RanjPrecision {
        // Precision bits are preserved by the mask; read directly from stored.
        RanjId::from_uuid(Uuid::from_u128(self.0.as_u128() ^ RANJ_FLIP_MASK))
            .expect("desc→asc XOR preserves version and variant")
            .into_parts()
            .precision
    }

    pub fn node_id(self) -> u16 {
        RanjId::from_uuid(Uuid::from_u128(self.0.as_u128() ^ RANJ_FLIP_MASK))
            .expect("desc→asc XOR preserves version and variant")
            .into_parts()
            .node_id
    }

    pub fn sequence(self) -> u16 {
        RanjId::from_uuid(Uuid::from_u128(self.0.as_u128() ^ RANJ_FLIP_MASK))
            .expect("desc→asc XOR preserves version and variant")
            .into_parts()
            .sequence
    }
}

impl fmt::Display for RanjIdDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RanjIdDesc {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let u: Uuid = s
            .parse()
            .map_err(|_| Error::InvalidRanjIdString(s.to_string()))?;
        Self::from_uuid(u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_round_trips_logical_components() {
        let id = RanjIdDesc::new(1_234_567, RanjPrecision::Microseconds, 513, 4096).unwrap();
        assert_eq!(id.timestamp(), 1_234_567);
        assert_eq!(id.precision(), RanjPrecision::Microseconds);
        assert_eq!(id.node_id(), 513);
        assert_eq!(id.sequence(), 4096);
    }

    #[test]
    fn preserves_uuidv8_conformance_after_flip() {
        let id = RanjIdDesc::new(1_000_000, RanjPrecision::Microseconds, 100, 200).unwrap();
        let u = id.as_uuid();
        assert_eq!(u.get_version_num(), 8);
        assert_eq!(u.get_variant(), uuid::Variant::RFC4122);
    }

    #[test]
    fn from_uuid_rejects_non_uuidv8() {
        let v4_bits: u128 = (0x4u128 << 76) | (0x2u128 << 62);
        let err = RanjIdDesc::from_uuid(Uuid::from_u128(v4_bits)).unwrap_err();
        assert!(matches!(err, Error::InvalidRanjIdVersion));
    }

    #[test]
    fn sorts_reverse_chronologically_when_precision_uniform() {
        let a = RanjIdDesc::new(10, RanjPrecision::Microseconds, 0, 0).unwrap();
        let b = RanjIdDesc::new(20, RanjPrecision::Microseconds, 0, 0).unwrap();
        let c = RanjIdDesc::new(30, RanjPrecision::Microseconds, 0, 0).unwrap();
        let mut v = vec![a, b, c];
        v.sort();
        assert_eq!(v, vec![c, b, a]);
    }

    #[test]
    fn round_trip_at_boundaries() {
        let id = RanjIdDesc::new(
            RanjId::MAX_TIMESTAMP,
            RanjPrecision::Microseconds,
            RanjId::MAX_NODE_ID,
            RanjId::MAX_SEQUENCE,
        )
        .unwrap();
        assert_eq!(id.timestamp(), RanjId::MAX_TIMESTAMP);
        assert_eq!(id.node_id(), RanjId::MAX_NODE_ID);
        assert_eq!(id.sequence(), RanjId::MAX_SEQUENCE);
    }

    #[test]
    fn display_and_from_str_round_trip() {
        let id = RanjIdDesc::new(1_000_000, RanjPrecision::Microseconds, 100, 200).unwrap();
        let s = id.to_string();
        let parsed: RanjIdDesc = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn all_precision_variants_round_trip() {
        for prec in [
            RanjPrecision::Microseconds,
            RanjPrecision::Nanoseconds,
            RanjPrecision::Picoseconds,
            RanjPrecision::Femtoseconds,
        ] {
            let id = RanjIdDesc::new(1_000_000, prec, 100, 200).unwrap();
            assert_eq!(id.precision(), prec);
            assert_eq!(id.timestamp(), 1_000_000);
        }
    }
}
