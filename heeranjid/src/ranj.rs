use crate::Error;
use crate::precision::RanjPrecision;
use crate::serde_helpers;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

pub const RANJ_TIMESTAMP_BITS: u8 = 89;
pub const RANJ_PRECISION_BITS: u8 = 2;
pub const RANJ_NODE_ID_BITS: u8 = 15;
pub const RANJ_SEQUENCE_BITS: u8 = 16;
pub const RANJ_UUID_VERSION: u8 = 0b1000;
pub const RANJ_UUID_VARIANT: u8 = 0b10;

const RANJ_TIMESTAMP_MASK: u128 = (1u128 << RANJ_TIMESTAMP_BITS) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RanjIdParts {
    pub timestamp: u128,
    pub precision: RanjPrecision,
    pub node_id: u16,
    pub sequence: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RanjId(
    #[serde(
        serialize_with = "serde_helpers::serialize_display",
        deserialize_with = "serde_helpers::deserialize_from_str_or_int"
    )]
    Uuid,
);

impl RanjId {
    pub const MAX_TIMESTAMP: u128 = RANJ_TIMESTAMP_MASK;
    pub const MAX_NODE_ID: u16 = (1u16 << 15) - 1;
    pub const MAX_SEQUENCE: u16 = u16::MAX;

    pub fn new(timestamp: u128, precision: RanjPrecision, node_id: u16, sequence: u16) -> Result<Self, Error> {
        if timestamp > Self::MAX_TIMESTAMP {
            return Err(Error::TimestampOutOfRange {
                value: timestamp,
                bits: RANJ_TIMESTAMP_BITS,
            });
        }
        if node_id > Self::MAX_NODE_ID {
            return Err(Error::NodeIdOutOfRange {
                value: u32::from(node_id),
                bits: RANJ_NODE_ID_BITS,
            });
        }

        let ts_high = (timestamp >> 41) & ((1u128 << 48) - 1);
        let ts_mid = (timestamp >> 29) & ((1u128 << 12) - 1);
        let ts_low = timestamp & ((1u128 << 29) - 1);

        let raw = (ts_high << 80)
            | (u128::from(RANJ_UUID_VERSION) << 76)
            | (ts_mid << 64)
            | (u128::from(RANJ_UUID_VARIANT) << 62)
            | (u128::from(precision.to_bits()) << 60)
            | (ts_low << 31)
            | (u128::from(node_id) << 16)
            | u128::from(sequence);

        Ok(Self(Uuid::from_u128(raw)))
    }

    pub fn from_uuid(uuid: Uuid) -> Result<Self, Error> {
        let raw = uuid.as_u128();
        let version = ((raw >> 76) & 0xF) as u8;
        let variant = ((raw >> 62) & 0x3) as u8;

        if version != RANJ_UUID_VERSION {
            return Err(Error::InvalidRanjIdVersion);
        }
        if variant != RANJ_UUID_VARIANT {
            return Err(Error::InvalidRanjIdVariant);
        }

        Ok(Self(uuid))
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }

    pub fn into_parts(self) -> RanjIdParts {
        let raw = self.0.as_u128();
        let ts_high = (raw >> 80) & ((1u128 << 48) - 1);
        let ts_mid = (raw >> 64) & ((1u128 << 12) - 1);
        let precision_bits = ((raw >> 60) & 0b11) as u8;
        let ts_low = (raw >> 31) & ((1u128 << 29) - 1);

        RanjIdParts {
            timestamp: (ts_high << 41) | (ts_mid << 29) | ts_low,
            precision: RanjPrecision::from_bits(precision_bits).expect("valid precision bits"),
            node_id: ((raw >> 16) & u128::from(Self::MAX_NODE_ID)) as u16,
            sequence: (raw & 0xFFFF) as u16,
        }
    }

    pub fn timestamp(self) -> u128 {
        self.into_parts().timestamp
    }

    pub fn precision(self) -> RanjPrecision {
        self.into_parts().precision
    }

    pub fn timestamp_micros(self) -> u128 {
        let parts = self.into_parts();
        parts.timestamp / parts.precision.to_micros_divisor()
    }

    pub fn node_id(self) -> u16 {
        self.into_parts().node_id
    }

    pub fn sequence(self) -> u16 {
        self.into_parts().sequence
    }
}

impl fmt::Display for RanjId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RanjId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(s).map_err(|_| Error::InvalidRanjIdString(s.to_owned()))?;
        Self::from_uuid(uuid)
    }
}

impl From<RanjId> for Uuid {
    fn from(id: RanjId) -> Self {
        id.0
    }
}

impl TryFrom<Uuid> for RanjId {
    type Error = Error;

    fn try_from(uuid: Uuid) -> Result<Self, Self::Error> {
        Self::from_uuid(uuid)
    }
}
