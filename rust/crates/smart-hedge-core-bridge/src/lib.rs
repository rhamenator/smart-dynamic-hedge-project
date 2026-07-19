//! Rust port of `python/smart_hedge/core_bridge.py`: cross-platform
//! resolution, on-demand building, and subprocess invocation of the C++
//! deterministic core. See `docs/ROADMAP.md` "Language and dependency
//! policy" for the migration this is part of.

pub mod build;
pub mod error;
pub mod paths;
pub mod run;
pub mod run_with_timeout;
pub mod which;

pub use build::{build_core, ensure_core};
pub use error::CoreError;
pub use paths::{default_binary_path, resolve_binary, windows_multi_config_fallback};
pub use run::run_core;
pub use which::which as which_tool;
