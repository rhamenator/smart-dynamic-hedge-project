//! Rust port of `python/smart_hedge/features.py`. See `docs/ROADMAP.md`
//! "Language and dependency policy" and `requirements/LLR.md`
//! (SDH-LLR-110 through SDH-LLR-113) for the requirements this ports.

pub mod build;
pub mod evidence_summary;
pub mod returns;
pub mod stats;

#[cfg(test)]
mod integration_tests;

pub use build::build_features;
