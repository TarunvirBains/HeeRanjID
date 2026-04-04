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

#[cfg(feature = "postgres")]
#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("database returned invalid HeerId: {0}")]
    InvalidHeerId(#[source] Error),
    #[error("database returned invalid RanjId: {0}")]
    InvalidRanjId(#[source] Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(feature = "postgres")]
#[derive(Debug, Error)]
pub enum StartupError {
    #[error("node {0} is not registered or not active")]
    NodeNotActive(u16),
    #[error("heer_config epoch is not configured")]
    MissingEpoch,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
