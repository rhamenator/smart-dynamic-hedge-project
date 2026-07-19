//! Rust port of `python/smart_hedge/store.py`. See `docs/ROADMAP.md`
//! "Language and dependency policy" and `requirements/LLR.md`
//! (SDH-LLR-070 through SDH-LLR-073) for the requirements this ports.
//!
//! Unlike every other crate in this workspace, this one depends on
//! `rusqlite` — see the `Cargo.toml` comment for why the SQLite file
//! format is a deliberate exception to "hand-roll instead of depend".

pub mod canonical;
pub mod error;
pub mod field_access;
pub mod store;

#[cfg(test)]
mod integration_tests;

pub use canonical::{canonical_json, hash_payload};
pub use error::StoreError;
pub use store::DecisionStore;
