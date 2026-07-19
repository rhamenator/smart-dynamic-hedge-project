//! Rust port of `python/smart_hedge/model_advisor.py`'s schema validation
//! and `HeuristicAdvisor`. `OpenAIAdvisor` is **not yet ported** — it
//! needs an HTTP-client dependency decision, deferred per
//! `requirements/LLR.md` `SDH-LLR-126`. See `docs/ROADMAP.md` "Language
//! and dependency policy".

pub mod advisor;
pub mod error;
pub mod heuristic;
pub mod schema;

pub use advisor::Advisor;
pub use error::{AdvisorError, SchemaError};
pub use heuristic::HeuristicAdvisor;
pub use schema::{validate_assessment_payload, ALLOWED_REGIMES};
