use std::fmt;

use crate::args::ArgsError;

#[derive(Debug)]
pub enum CliError {
    Args(ArgsError),
    Config(smart_hedge_config::ConfigError),
    Core(smart_hedge_core_bridge::CoreError),
    Engine(smart_hedge_engine::EngineError),
    Io(std::io::Error),
    /// `serve`/`mcp` are recognized commands that need a dependency
    /// decision (HTTP server, MCP-over-stdio) this binary hasn't made yet —
    /// distinct from an unrecognized command entirely.
    NotYetImplemented(&'static str),
    SelfTestFailed(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Args(e) => write!(f, "{e}"),
            Self::Config(e) => write!(f, "{e}"),
            Self::Core(e) => write!(f, "{e}"),
            Self::Engine(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::NotYetImplemented(what) => write!(f, "{what} is not yet implemented in the Rust CLI"),
            Self::SelfTestFailed(msg) => write!(f, "self-test failed: {msg}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ArgsError> for CliError {
    fn from(e: ArgsError) -> Self {
        CliError::Args(e)
    }
}
impl From<smart_hedge_config::ConfigError> for CliError {
    fn from(e: smart_hedge_config::ConfigError) -> Self {
        CliError::Config(e)
    }
}
impl From<smart_hedge_core_bridge::CoreError> for CliError {
    fn from(e: smart_hedge_core_bridge::CoreError) -> Self {
        CliError::Core(e)
    }
}
impl From<smart_hedge_engine::EngineError> for CliError {
    fn from(e: smart_hedge_engine::EngineError) -> Self {
        CliError::Engine(e)
    }
}
impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}
