use smart_hedge_config::LoadedConfig;
use smart_hedge_data::{AlpacaReadOnlyProvider, MarketDataProvider, SyntheticProvider};
use smart_hedge_model_advisor::{Advisor, HeuristicAdvisor, ModelUri, OpenAIAdvisor};

use crate::error::EngineError;

/// Port of `data.build_provider`.
pub fn build_provider(loaded: &LoadedConfig) -> Result<Box<dyn MarketDataProvider>, EngineError> {
    match loaded.config.provider.kind.to_lowercase().as_str() {
        "synthetic" => Ok(Box::new(SyntheticProvider::new(loaded.clone()))),
        "alpaca" | "alpaca-readonly" | "alpaca_readonly" => {
            Ok(Box::new(AlpacaReadOnlyProvider::from_env(loaded.clone())?))
        }
        other => Err(EngineError::UnknownProviderKind(other.to_string())),
    }
}

/// Port of `model_advisor.build_advisor`. Unchanged behavior — the
/// legacy `model.kind`/`model.name` single-adviser path, still the
/// default `SmartHedgeEngine::new` uses when no `MODEL_URI` router name
/// is given. `build_advisor_by_name(loaded, "default")` below is a
/// strict superset: it only diverges from this function when
/// `config.model.models` actually has an entry for the requested name.
pub fn build_advisor(loaded: &LoadedConfig) -> Result<Box<dyn Advisor>, EngineError> {
    match loaded.config.model.kind.to_lowercase().as_str() {
        "heuristic" | "none" | "local" => Ok(Box::new(HeuristicAdvisor)),
        "openai" | "responses" => Ok(Box::new(
            OpenAIAdvisor::from_env(loaded).map_err(EngineError::AdvisorConstructionFailed)?,
        )),
        other => Err(EngineError::UnknownAdvisorKind(other.to_string())),
    }
}

/// The `MODEL_URI` router's entry point: resolves `name` against
/// `config.model.models` (a `{"name": "scheme://identifier"}` registry —
/// see `smart_hedge_model_advisor::model_uri`) and builds the adviser
/// that URI names. If `models` has no entry for `name`, falls back to
/// `build_advisor()`'s legacy `kind`/`name` selection *only when `name`
/// is `"default"`* — requesting any other, genuinely unconfigured name is
/// an error, not a silent fallback to whatever the legacy single adviser
/// happens to be (that would defeat the point of asking for a specific
/// named model).
pub fn build_advisor_by_name(loaded: &LoadedConfig, name: &str) -> Result<Box<dyn Advisor>, EngineError> {
    if let Some(uri_str) = loaded.config.model.models.get(name) {
        let uri = ModelUri::parse(uri_str)
            .map_err(|e| EngineError::AdvisorConstructionFailed(smart_hedge_model_advisor::AdvisorError(e.to_string())))?;
        return smart_hedge_model_advisor::build_advisor_from_uri(loaded, &uri).map_err(EngineError::AdvisorConstructionFailed);
    }
    if name == "default" {
        return build_advisor(loaded);
    }
    Err(EngineError::UnknownAdvisorKind(format!(
        "no model named {name:?} in config.model.models (configure it there, or use \"default\" for the legacy model.kind/model.name selection)"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn config_with_provider_kind(kind: &str) -> LoadedConfig {
        let dir = std::env::temp_dir().join(format!("smart-hedge-engine-factory-test-{}-{}", kind, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"provider": {{"kind": "{kind}"}}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    fn config_with_model_kind(kind: &str) -> LoadedConfig {
        // A counter suffix, not just `kind`+PID: two tests calling this
        // with the *same* `kind` (as now happens — both the original
        // `heuristic_advisor_builds_successfully` and the newer
        // `build_advisor_by_name_default_with_no_registry_falls_back_to_legacy_kind`
        // use "heuristic") would otherwise share one temp directory path
        // and race on `remove_dir_all` when run in parallel threads,
        // producing an intermittent "file not found" failure that has
        // nothing to do with the code under test.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-engine-factory-model-test-{}-{}-{n}", kind, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"model": {{"kind": "{kind}"}}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    fn config_with_models_registry(models_json: &str) -> LoadedConfig {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-engine-factory-router-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"model": {{"models": {models_json}}}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    #[test]
    fn synthetic_provider_builds_successfully() {
        let loaded = config_with_provider_kind("synthetic");
        let provider = build_provider(&loaded).unwrap();
        assert_eq!(provider.name(), "SyntheticProvider");
    }

    /// Without `ALPACA_API_KEY_ID`/`ALPACA_API_SECRET_KEY` set in the test
    /// environment, building an Alpaca provider fails fast with a specific
    /// missing-credentials error rather than silently falling back to
    /// another provider or panicking.
    #[test]
    fn alpaca_provider_kind_without_credentials_fails_fast() {
        let loaded = config_with_provider_kind("alpaca-readonly");
        let result = build_provider(&loaded);
        assert!(matches!(result, Err(EngineError::Data(_))));
    }

    #[test]
    fn unrecognized_provider_kind_is_a_distinct_error_from_a_known_but_misconfigured_one() {
        let loaded = config_with_provider_kind("totally-made-up");
        let result = build_provider(&loaded);
        assert!(matches!(result, Err(EngineError::UnknownProviderKind(_))));
    }

    #[test]
    fn heuristic_advisor_builds_successfully() {
        let loaded = config_with_model_kind("heuristic");
        let advisor = build_advisor(&loaded).unwrap();
        assert_eq!(advisor.name(), "HeuristicAdvisor");
    }

    /// Without `OPENAI_API_KEY`/a real `model.name` set in the test
    /// environment, building an OpenAI adviser fails fast at construction
    /// time with a specific error rather than panicking or silently
    /// falling back to the heuristic adviser.
    #[test]
    fn openai_advisor_kind_without_configuration_fails_fast() {
        let loaded = config_with_model_kind("openai");
        let result = build_advisor(&loaded);
        assert!(matches!(result, Err(EngineError::AdvisorConstructionFailed(_))));
    }

    #[test]
    fn build_advisor_by_name_default_with_no_registry_falls_back_to_legacy_kind() {
        let loaded = config_with_model_kind("heuristic");
        let advisor = build_advisor_by_name(&loaded, "default").unwrap();
        assert_eq!(advisor.name(), "HeuristicAdvisor");
    }

    #[test]
    fn build_advisor_by_name_routes_a_registered_name_to_its_uri() {
        let loaded = config_with_models_registry(r#"{"default": "heuristic://default"}"#);
        let advisor = build_advisor_by_name(&loaded, "default").unwrap();
        assert_eq!(advisor.name(), "HeuristicAdvisor");
    }

    #[test]
    fn build_advisor_by_name_an_unregistered_non_default_name_is_an_error_not_a_silent_fallback() {
        let loaded = config_with_models_registry(r#"{"default": "heuristic://default"}"#);
        let result = build_advisor_by_name(&loaded, "totally-unconfigured");
        assert!(matches!(result, Err(EngineError::UnknownAdvisorKind(_))));
    }

    #[test]
    fn build_advisor_by_name_a_registered_openai_uri_without_credentials_fails_fast() {
        let loaded = config_with_models_registry(r#"{"aggressive": "openai://gpt-4.1"}"#);
        let result = build_advisor_by_name(&loaded, "aggressive");
        assert!(matches!(result, Err(EngineError::AdvisorConstructionFailed(_))));
    }

    #[test]
    fn build_advisor_by_name_registry_entry_takes_priority_over_legacy_kind_even_for_default() {
        // config.model.kind defaults to "heuristic", but an explicit
        // registry entry for "default" naming an unconfigured openai
        // model should still be consulted first (and fail fast on
        // missing credentials, not silently succeed via the heuristic
        // fallback path).
        let loaded = config_with_models_registry(r#"{"default": "openai://gpt-4.1"}"#);
        let result = build_advisor_by_name(&loaded, "default");
        assert!(matches!(result, Err(EngineError::AdvisorConstructionFailed(_))));
    }
}
