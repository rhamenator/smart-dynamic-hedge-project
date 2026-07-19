use std::fmt;

#[derive(Debug)]
pub enum DataError {
    Io(std::io::Error),
    InvalidJson(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidJson(msg) => write!(f, "invalid JSON: {msg}"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<std::io::Error> for DataError {
    fn from(err: std::io::Error) -> Self {
        DataError::Io(err)
    }
}
