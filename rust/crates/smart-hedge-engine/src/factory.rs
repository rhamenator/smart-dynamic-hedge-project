use smart_hedge_config::LoadedConfig;
use smart_hedge_data::{MarketDataProvider, SyntheticProvider};
use smart_hedge_model_advisor::{Advisor, HeuristicAdvisor};

use crate::error::EngineError;

/// Port of `data.build_provider`. Recognized-but-unported kinds
/// (`alpaca`/`alpaca-readonly`) return `EngineError::NotYetPorted`, not a
/// silent fallback to the synthetic provider.
pub fn build_provider(loaded: &LoadedConfig) -> Result<Box<dyn MarketDataProvider>, EngineError> {
    match loaded.config.provider.kind.to_lowercase().as_str() {
        "synthetic" => Ok(Box::new(SyntheticProvider::new(loaded.clone()))),
        "alpaca" | "alpaca-readonly" | "alpaca_readonly" => {
            Err(EngineError::NotYetPorted("the Alpaca read-only provider".to_string()))
        }
        other => Err(EngineError::UnknownProviderKind(other.to_string())),
    }
}

/// Port of `model_advisor.build_advisor`. `openai`/`responses` returns
/// `EngineError::NotYetPorted`, not a silent fallback to the heuristic
/// adviser — the whole point of `model.fallback_to_heuristic` is that
/// *runtime* adviser failures fall back visibly (`fallback_reason`
/// recorded); a *configuration* naming an unported adviser is a different
/// kind of problem and should fail at construction time instead.
pub fn build_advisor(loaded: &LoadedConfig) -> Result<Box<dyn Advisor>, EngineError> {
    match loaded.config.model.kind.to_lowercase().as_str() {
        "heuristic" | "none" | "local" => Ok(Box::new(HeuristicAdvisor)),
        "openai" | "responses" => Err(EngineError::NotYetPorted("the OpenAI adviser".to_string())),
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

    #[test]
    fn alpaca_provider_kind_reports_not_yet_ported_not_a_silent_fallback() {
        let loaded = config_with_provider_kind("alpaca-readonly");
        let result = build_provider(&loaded);
        assert!(matches!(result, Err(EngineError::NotYetPorted(_))));
    }

    #[test]
    fn unrecognized_provider_kind_is_a_distinct_error_from_not_yet_ported() {
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

    #[test]
    fn openai_advisor_kind_reports_not_yet_ported() {
        let loaded = config_with_model_kind("openai");
        let result = build_advisor(&loaded);
        assert!(matches!(result, Err(EngineError::NotYetPorted(_))));
    }
}
