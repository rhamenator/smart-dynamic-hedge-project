use smart_hedge_config::LoadedConfig;
use smart_hedge_data::{AlpacaReadOnlyProvider, MarketDataProvider, SyntheticProvider};
use smart_hedge_model_advisor::{Advisor, HeuristicAdvisor, OpenAIAdvisor};

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

/// Port of `model_advisor.build_advisor`.
pub fn build_advisor(loaded: &LoadedConfig) -> Result<Box<dyn Advisor>, EngineError> {
    match loaded.config.model.kind.to_lowercase().as_str() {
        "heuristic" | "none" | "local" => Ok(Box::new(HeuristicAdvisor)),
        "openai" | "responses" => Ok(Box::new(
            OpenAIAdvisor::from_env(loaded).map_err(EngineError::AdvisorConstructionFailed)?,
        )),
        other => Err(EngineError::UnknownAdvisorKind(other.to_string())),
    }
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
        let dir = std::env::temp_dir().join(format!("smart-hedge-engine-factory-model-test-{}-{}", kind, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"model": {{"kind": "{kind}"}}}}"#)).unwrap();
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
}
