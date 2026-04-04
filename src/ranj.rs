use crate::Error;
use crate::serde_helpers;
use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

pub const RANJ_TIMESTAMP_BITS: u8 = 90;
pub const RANJ_NODE_ID_BITS: u8 = 16;
pub const RANJ_SEQUENCE_BITS: u8 = 16;
pub const RANJ_UUID_VERSION: u8 = 0b0111;
pub const RANJ_UUID_VARIANT: u8 = 0b10;

const RANJ_TIMESTAMP_MASK: u128 = (1u128 << RANJ_TIMESTAMP_BITS) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RanjIdParts {
    pub timestamp_micros: u128,
    pub node_id: u16,
    pub sequence: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Type, Serialize, Deserialize)]
#[sqlx(transparent)]
pub struct RanjId(
    #[serde(
        serialize_with = "serde_helpers::serialize_display",
        deserialize_with = "serde_helpers::deserialize_from_str_or_int"
    )]
    Uuid,
);

impl RanjId {
    pub const MAX_TIMESTAMP_MICROS: u128 = RANJ_TIMESTAMP_MASK;
    pub const MAX_NODE_ID: u16 = u16::MAX;
    pub const MAX_SEQUENCE: u16 = u16::MAX;

    pub fn new(timestamp_micros: u128, node_id: u16, sequence: u16) -> Result<Self, Error> {
        if timestamp_micros > Self::MAX_TIMESTAMP_MICROS {
            return Err(Error::TimestampOutOfRange {
                value: timestamp_micros,
                bits: RANJ_TIMESTAMP_BITS,
            });
        }

        let timestamp_high = (timestamp_micros >> 42) & ((1u128 << 48) - 1);
        let timestamp_mid = (timestamp_micros >> 30) & ((1u128 << 12) - 1);
        let timestamp_low = timestamp_micros & ((1u128 << 30) - 1);

        let raw = (timestamp_high << 80)
            | (u128::from(RANJ_UUID_VERSION) << 76)
            | (timestamp_mid << 64)
            | (u128::from(RANJ_UUID_VARIANT) << 62)
            | (timestamp_low << 32)
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
        let timestamp_high = (raw >> 80) & ((1u128 << 48) - 1);
        let timestamp_mid = (raw >> 64) & ((1u128 << 12) - 1);
        let timestamp_low = (raw >> 32) & ((1u128 << 30) - 1);

        RanjIdParts {
            timestamp_micros: (timestamp_high << 42) | (timestamp_mid << 30) | timestamp_low,
            node_id: ((raw >> 16) & 0xFFFF) as u16,
            sequence: (raw & 0xFFFF) as u16,
        }
    }

    pub fn timestamp_micros(self) -> u128 {
        self.into_parts().timestamp_micros
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
