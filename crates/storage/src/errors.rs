#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    RocksDb(String),

    #[error("Error writting db: {0}")]
    WriteDb(String),

    #[error("Error reading db: {0}")]
    ReadDb(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}
