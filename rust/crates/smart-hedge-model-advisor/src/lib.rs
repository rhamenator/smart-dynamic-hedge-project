//! Rust port of `python/smart_hedge/model_advisor.py`: schema validation,
//! `HeuristicAdvisor`, and `OpenAIAdvisor`. `OpenAIAdvisor` needs a real
//! HTTPS call — see this crate's `Cargo.toml` for the `ureq`/`rustls`
//! dependency decision (same reasoning as `smart-hedge-data`'s).

pub mod advisor;
pub mod error;
pub mod heuristic;
mod http_util;
#[cfg(test)]
mod mock_http_test_support;
pub mod model_uri;
pub mod openai;
pub mod router;
pub mod schema;

pub use advisor::Advisor;
pub use error::{AdvisorError, SchemaError};
pub use heuristic::HeuristicAdvisor;
pub use model_uri::{ModelUri, ModelUriError};
pub use openai::OpenAIAdvisor;
pub use router::build_advisor_from_uri;
pub use schema::{assessment_json_schema, validate_assessment_payload, ALLOWED_REGIMES};
