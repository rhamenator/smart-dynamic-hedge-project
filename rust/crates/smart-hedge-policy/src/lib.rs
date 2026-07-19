//! Rust port of `python/smart_hedge/policy.py`. See `docs/ROADMAP.md`
//! "Language and dependency policy" for the migration this is part of.

pub mod evaluate;
pub mod rounding;

#[cfg(test)]
mod parity_tests;

pub use evaluate::{evaluate_policy, POLICY_VERSION};
pub use rounding::round_half_to_even;
