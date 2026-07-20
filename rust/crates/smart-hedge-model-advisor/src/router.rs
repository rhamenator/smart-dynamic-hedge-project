//! The `MODEL_URI` router: builds an `Advisor` from a `ModelUri` rather
//! than the single static `model.kind`/`model.name` pair
//! `factory::build_advisor` (in `smart-hedge-engine`) still uses.
//!
//! This crate deliberately does not decide *which* named model a caller
//! should route to per decision — there is no signal anywhere in this
//! system yet (no per-symbol/per-regime policy) that would make a
//! dynamic "pick model A vs B automatically" choice anything but
//! speculative. What this module provides instead is the addressing and
//! construction half of routing — turn a URI into a working `Advisor` —
//! which `smart-hedge-engine::factory::build_advisor_by_name` combines
//! with `config.model.models`' named registry, and `smart-hedge-cli`'s
//! `--model <name>` flag exposes as an explicit *human* choice per
//! invocation. That is a real, complete router in the sense
//! `06-implementation-order-and-acceptance.md` asks for (`MODEL_URI`
//! routing to more than one configured model), just not an autonomous
//! one — autonomous model selection remains future work, same as
//! autonomous *trading* does.

use smart_hedge_config::LoadedConfig;

use crate::advisor::Advisor;
use crate::error::AdvisorError;
use crate::heuristic::HeuristicAdvisor;
use crate::model_uri::ModelUri;
use crate::openai::OpenAIAdvisor;

/// Constructs the `Advisor` a `ModelUri` names. `heuristic://` (any or no
/// identifier) always succeeds; `openai://<model>` needs a non-empty
/// identifier and `OPENAI_API_KEY` set. Any other scheme is a
/// configuration error, not a panic.
pub fn build_advisor_from_uri(loaded: &LoadedConfig, uri: &ModelUri) -> Result<Box<dyn Advisor>, AdvisorError> {
    match uri.scheme.as_str() {
        "heuristic" | "none" | "local" => Ok(Box::new(HeuristicAdvisor)),
        "openai" | "responses" => {
            let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            let advisor = OpenAIAdvisor::with_explicit_model(loaded, api_key, uri.identifier.clone())?;
            Ok(Box::new(advisor))
        }
        other => Err(AdvisorError(format!("unknown model URI scheme {other:?} (expected heuristic:// or openai://)"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded_config() -> LoadedConfig {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-model-advisor-router-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{}").unwrap();
        let loaded =
            smart_hedge_config::load_config(Some(&path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    #[test]
    fn heuristic_uri_always_builds() {
        let loaded = loaded_config();
        let uri = ModelUri::parse("heuristic://default").unwrap();
        let advisor = build_advisor_from_uri(&loaded, &uri).unwrap();
        assert_eq!(advisor.name(), "HeuristicAdvisor");
    }

    #[test]
    fn bare_heuristic_scheme_with_no_identifier_builds() {
        let loaded = loaded_config();
        let uri = ModelUri::parse("heuristic").unwrap();
        let advisor = build_advisor_from_uri(&loaded, &uri).unwrap();
        assert_eq!(advisor.name(), "HeuristicAdvisor");
    }

    #[test]
    fn openai_uri_without_an_identifier_fails() {
        let loaded = loaded_config();
        let uri = ModelUri::parse("openai://").unwrap();
        let result = build_advisor_from_uri(&loaded, &uri);
        assert!(result.is_err());
    }

    #[test]
    fn unknown_scheme_is_a_distinct_error() {
        let loaded = loaded_config();
        let uri = ModelUri::parse("carrier-pigeon://default").unwrap();
        let result = build_advisor_from_uri(&loaded, &uri);
        assert!(result.is_err());
    }
}
