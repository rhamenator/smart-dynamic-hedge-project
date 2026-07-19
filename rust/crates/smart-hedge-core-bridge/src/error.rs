use std::fmt;
use std::time::Duration;

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    /// No `cmake`, `g++`, or `clang++` was found on `PATH`.
    NoToolchainFound,
    /// The build command ran but the expected binary still doesn't exist.
    BuildSucceededButBinaryMissing(std::path::PathBuf),
    /// `auto_build` is disabled and the binary is not present.
    BinaryNotFound(std::path::PathBuf),
    Timeout(Duration),
    NonZeroExit { code: Option<i32>, stderr: String },
    InvalidJson(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error running the C++ core: {err}"),
            Self::NoToolchainFound => {
                write!(f, "cmake, g++, or clang++ is required to build the C++ core")
            }
            Self::BuildSucceededButBinaryMissing(path) => {
                write!(f, "build completed but core binary was not found at {}", path.display())
            }
            Self::BinaryNotFound(path) => {
                write!(f, "core binary not found: {} (auto-build is disabled)", path.display())
            }
            Self::Timeout(d) => write!(f, "C++ core timed out after {:.1}s", d.as_secs_f64()),
            Self::NonZeroExit { code, stderr } => {
                if stderr.trim().is_empty() {
                    write!(f, "C++ core exited with status {code:?}")
                } else {
                    write!(f, "{}", stderr.trim())
                }
            }
            Self::InvalidJson(msg) => write!(f, "C++ core returned invalid or incomplete JSON: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::Io(err)
    }
}
