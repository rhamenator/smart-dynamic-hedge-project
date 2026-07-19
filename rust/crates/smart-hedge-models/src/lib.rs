//! Rust port of `python/smart_hedge/models.py`, the first slice of the
//! Python-to-Rust migration described in `docs/ROADMAP.md` "Language and
//! dependency policy". Built in an isolated `rust/` workspace with zero
//! changes to the existing Python/C++ code — see that ROADMAP section for
//! the migration plan and its rationale.

pub mod assessment;
pub mod bar;
pub mod core_response;
pub mod evidence;
pub mod features;
pub mod policy_decision;
pub mod quote;
pub mod recommendation;
pub mod snapshot;
pub mod time_util;

pub use assessment::ModelAssessment;
pub use bar::Bar;
pub use core_response::{CoreGreeks, CoreHedge, CoreInputs, CorePricing, CoreResponse, CoreRisk};
pub use evidence::EvidenceItem;
pub use features::FeatureSet;
pub use policy_decision::{AppliedLimits, PolicyDecision};
pub use quote::Quote;
pub use recommendation::Recommendation;
pub use snapshot::MarketSnapshot;
pub use time_util::TimestampUtc;
