use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("database returned invalid HeerId: {0}")]
    InvalidHeerId(#[source] heeranjid::Error),
    #[error("database returned invalid RanjId: {0}")]
    InvalidRanjId(#[source] heeranjid::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("node {0} is not registered or not active")]
    NodeNotActive(u16),
    #[error("heer_config epoch is not configured")]
    MissingEpoch,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
