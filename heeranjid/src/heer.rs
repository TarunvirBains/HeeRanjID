use crate::Error;
use crate::serde_helpers;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const HEER_TIMESTAMP_BITS: u8 = 41;
pub const HEER_NODE_ID_BITS: u8 = 9;
pub const HEER_SEQUENCE_BITS: u8 = 13;

const HEER_TIMESTAMP_MASK: u64 = (1u64 << HEER_TIMESTAMP_BITS) - 1;
const HEER_NODE_ID_MASK: u64 = (1u64 << HEER_NODE_ID_BITS) - 1;
const HEER_SEQUENCE_MASK: u64 = (1u64 << HEER_SEQUENCE_BITS) - 1;

/// HeerId flip mask — XOR target for converting between asc and desc forms.
/// All 41 timestamp bits + all 13 sequence bits set; node bits and bit 63 zero.
/// = `(((1i64 << 41) - 1) << 22) | ((1i64 << 13) - 1)` = `0x7FFF_FFFF_FFC0_1FFF`.
pub(crate) const HEER_FLIP_MASK: i64 = 0x7FFF_FFFF_FFC0_1FFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeerIdParts {
    pub timestamp_ms: u64,
    pub node_id: u16,
    pub sequence: u16,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HeerId(
    #[serde(
        serialize_with = "serde_helpers::serialize_display",
        deserialize_with = "serde_helpers::deserialize_from_str_or_int"
    )]
    i64,
);

impl HeerId {
    pub const MAX_TIMESTAMP_MS: u64 = HEER_TIMESTAMP_MASK;
    pub const MAX_NODE_ID: u16 = HEER_NODE_ID_MASK as u16;
    pub const MAX_SEQUENCE: u16 = HEER_SEQUENCE_MASK as u16;
    /// Sentinel value (wire-zero) for use as a placeholder ID before
    /// persistence — e.g., when constructing a row in memory before an
    /// `INSERT ... RETURNING id` assigns the real ID.
    ///
    /// Safe as a sentinel: the 41-bit timestamp field of a real
    /// `generate_id()` output is wall-clock-derived from a non-zero
    /// `CURRENT_TIMESTAMP` epoch, so the all-zero bit pattern is provably
    /// outside the image of the generator. Round-trips cleanly through
    /// [`from_i64`](Self::from_i64), `Display`/`FromStr`, and serde.
    pub const ZERO: Self = Self(0);

    pub fn new(timestamp_ms: u64, node_id: u16, sequence: u16) -> Result<Self, Error> {
        if timestamp_ms > Self::MAX_TIMESTAMP_MS {
            return Err(Error::TimestampOutOfRange {
                value: timestamp_ms as u128,
                bits: HEER_TIMESTAMP_BITS,
            });
        }
        if node_id > Self::MAX_NODE_ID {
            return Err(Error::NodeIdOutOfRange {
                value: node_id as u32,
                bits: HEER_NODE_ID_BITS,
            });
        }
        if sequence > Self::MAX_SEQUENCE {
            return Err(Error::SequenceOutOfRange {
                value: sequence as u32,
                bits: HEER_SEQUENCE_BITS,
            });
        }

        let raw = ((timestamp_ms & HEER_TIMESTAMP_MASK)
            << (HEER_NODE_ID_BITS + HEER_SEQUENCE_BITS))
            | ((u64::from(node_id) & HEER_NODE_ID_MASK) << HEER_SEQUENCE_BITS)
            | (u64::from(sequence) & HEER_SEQUENCE_MASK);

        Ok(Self(raw as i64))
    }

    pub fn from_i64(raw: i64) -> Result<Self, Error> {
        if raw < 0 {
            return Err(Error::NegativeHeerId);
        }
        Ok(Self(raw))
    }

    /// Wrap a raw i64 without checking the non-negative invariant.
    ///
    /// Used only by `reverse_order::heer` after an XOR with `HEER_FLIP_MASK`
    /// (bit 63 = 0), which provably preserves the bit-63 = 0 invariant that
    /// `from_i64` enforces. Not exposed outside the crate because normal
    /// callers must go through `from_i64`.
    pub(crate) fn from_i64_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn as_i64(self) -> i64 {
        self.0
    }

    /// Returns `true` iff `self == HeerId::ZERO`. Mirrors `Duration::is_zero()`.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn into_parts(self) -> HeerIdParts {
        let raw = self.0 as u64;
        HeerIdParts {
            timestamp_ms: raw >> (HEER_NODE_ID_BITS + HEER_SEQUENCE_BITS),
            node_id: ((raw >> HEER_SEQUENCE_BITS) & HEER_NODE_ID_MASK) as u16,
            sequence: (raw & HEER_SEQUENCE_MASK) as u16,
        }
    }

    pub fn timestamp_ms(self) -> u64 {
        self.into_parts().timestamp_ms
    }

    pub fn node_id(self) -> u16 {
        self.into_parts().node_id
    }

    pub fn sequence(self) -> u16 {
        self.into_parts().sequence
    }
}

impl fmt::Display for HeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for HeerId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = s
            .parse::<i64>()
            .map_err(|_| Error::InvalidHeerIdString(s.to_owned()))?;
        Self::from_i64(parsed)
    }
}

impl From<HeerId> for i64 {
    fn from(id: HeerId) -> Self {
        id.0
    }
}

impl TryFrom<i64> for HeerId {
    type Error = Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::from_i64(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time proof that the mask preserves bit 63 = 0, which the
    // HeerIdDesc type-level invariant depends on (§4.1).
    const _: () = assert!(HEER_FLIP_MASK >= 0);

    #[test]
    fn heer_flip_mask_matches_spec_derivation() {
        let derived: i64 = (((1i64 << 41) - 1) << 22) | ((1i64 << 13) - 1);
        assert_eq!(derived, HEER_FLIP_MASK);
        assert_eq!(HEER_FLIP_MASK, 9_223_372_036_850_589_695);
    }

    #[test]
    fn zero_const_round_trips_through_from_i64() {
        assert_eq!(HeerId::ZERO, HeerId::from_i64(0).unwrap());
        assert_eq!(HeerId::ZERO.as_i64(), 0);
    }

    #[test]
    fn zero_const_round_trips_through_serde() {
        let json = serde_json::to_string(&HeerId::ZERO).unwrap();
        let parsed: HeerId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, HeerId::ZERO);
    }

    #[test]
    fn is_zero_predicate() {
        assert!(HeerId::ZERO.is_zero());
        assert!(!HeerId::new(1, 0, 0).unwrap().is_zero());
    }

    #[test]
    fn zero_orders_before_real_ids() {
        let real = HeerId::new(1, 0, 0).unwrap();
        assert!(HeerId::ZERO < real);
    }
}
