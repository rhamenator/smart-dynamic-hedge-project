use std::fmt;

#[derive(Debug)]
pub enum DataError {
    Io(std::io::Error),
    InvalidJson(String),
    /// A required environment variable (credential) was not set.
    MissingEnvVar(&'static str),
    /// A network request failed (transport error or non-2xx/3xx status).
    Http(String),
    /// A response parsed as JSON but not into the shape this provider expects.
    UnexpectedResponse(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidJson(msg) => write!(f, "invalid JSON: {msg}"),
            Self::MissingEnvVar(name) => write!(f, "{name} is not set"),
            Self::Http(msg) => write!(f, "HTTP request failed: {msg}"),
            Self::UnexpectedResponse(msg) => write!(f, "unexpected response: {msg}"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<std::io::Error> for DataError {
    fn from(err: std::io::Error) -> Self {
        DataError::Io(err)
    }
}
