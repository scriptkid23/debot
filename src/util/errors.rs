use std::io;

#[derive(Debug)]
pub enum RaftError {
    StorageError(String),
    InvalidState(String),
    NetworkError(String),
    LogInconsistency,
    IoError(io::Error),
    SerializationError(String),
    InvalidConfig(String),
}

impl std::fmt::Display for RaftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaftError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            RaftError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            RaftError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            RaftError::LogInconsistency => write!(f, "Log inconsistency detected"),
            RaftError::IoError(err) => write!(f, "IO error: {}", err),
            RaftError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            RaftError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
        }
    }
}

impl std::error::Error for RaftError {}

impl From<io::Error> for RaftError {
    fn from(err: io::Error) -> Self {
        RaftError::IoError(err)
    }
}

impl From<bincode::Error> for RaftError {
    fn from(err: bincode::Error) -> Self {
        RaftError::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RaftError>;

