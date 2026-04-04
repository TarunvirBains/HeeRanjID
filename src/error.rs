use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("timestamp {value} exceeds {bits}-bit limit")]
    TimestampOutOfRange { value: u128, bits: u8 },
    #[error("node_id {value} exceeds {bits}-bit limit")]
    NodeIdOutOfRange { value: u32, bits: u8 },
    #[error("sequence {value} exceeds {bits}-bit limit")]
    SequenceOutOfRange { value: u32, bits: u8 },
    #[error("heerid must be non-negative")]
    NegativeHeerId,
    #[error("uuid version must be 7")]
    InvalidRanjIdVersion,
    #[error("uuid variant must be RFC 4122")]
    InvalidRanjIdVariant,
    #[error("invalid HeerId string: {0}")]
    InvalidHeerIdString(String),
    #[error("invalid RanjId string: {0}")]
    InvalidRanjIdString(String),
}
