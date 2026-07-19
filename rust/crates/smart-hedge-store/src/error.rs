use std::fmt;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    InvalidJson(String),
    /// The payload is missing (or has the wrong type for) one of the
    /// fields `append` needs to index directly: `decision_id`,
    /// `created_at`, `symbol`, `policy.action`,
    /// `model_assessment.advisor_kind`.
    MalformedPayload(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "sqlite error: {err}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidJson(msg) => write!(f, "invalid JSON: {msg}"),
            Self::MalformedPayload(msg) => write!(f, "malformed decision payload: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        StoreError::Sqlite(err)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(err: std::io::Error) -> Self {
        StoreError::Io(err)
    }
}
