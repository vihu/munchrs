use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum MunchError {
    #[error("Repository not found: {0}")]
    RepoNotFound(String),

    #[error("Repository not indexed: {0}")]
    NotIndexed(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, MunchError>;
