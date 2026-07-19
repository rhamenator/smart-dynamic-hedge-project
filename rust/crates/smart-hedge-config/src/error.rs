use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    /// Matches Python's `raise ValueError("configuration root must be a JSON object")`.
    RootNotAnObject,
    Io(std::io::Error),
    InvalidJson(String),
    /// Matches `raise ValueError("only mode='paper' is implemented")`.
    LiveModeNotSupported,
    /// Matches `raise ValueError("policy.paper_only must remain true")`.
    PolicyPaperOnlyRequired,
    /// The merged JSON tree didn't deserialize into the typed `Config` —
    /// e.g. a user config file put a string where a number belongs. Python
    /// has no equivalent failure mode since it never validates the merged
    /// dict's shape; this is a deliberate, documented behavior improvement
    /// (fail fast on a malformed config instead of a confusing `TypeError`
    /// deep inside unrelated code later).
    SchemaMismatch(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootNotAnObject => write!(f, "configuration root must be a JSON object"),
            Self::Io(err) => write!(f, "could not read configuration file: {err}"),
            Self::InvalidJson(msg) => write!(f, "configuration file is not valid JSON: {msg}"),
            Self::LiveModeNotSupported => write!(f, "only mode='paper' is implemented"),
            Self::PolicyPaperOnlyRequired => write!(f, "policy.paper_only must remain true"),
            Self::SchemaMismatch(msg) => write!(f, "configuration does not match the expected schema: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::Io(err)
    }
}
