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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeerIdParts {
    pub timestamp_ms: u64,
    pub node_id: u16,
    pub sequence: u16,
}

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

    pub fn as_i64(self) -> i64 {
        self.0
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
