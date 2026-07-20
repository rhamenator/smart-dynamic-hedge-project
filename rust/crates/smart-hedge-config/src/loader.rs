use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::defaults::default_config_json;
use crate::env_overrides::EnvOverrides;
use crate::error::ConfigError;
use crate::merge::deep_merge;
use crate::paths::expand_user;
use crate::types::Config;

/// The typed config plus the bookkeeping Python smuggled into the dict
/// itself as `_config_path`/`_config_dir`. Kept as separate fields here
/// instead, since polluting the JSON schema with underscore-prefixed
/// metadata keys is a Python-dict convenience this crate doesn't need.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    pub config: Config,
    pub config_dir: PathBuf,
    pub config_path: Option<PathBuf>,
}

fn set_nested_string(root: &mut Value, path: &[&str], new_value: &str) {
    let mut node = root;
    for key in path {
        if !node.is_object() {
            *node = Value::Object(serde_json::Map::new());
        }
        let map = node.as_object_mut().expect("just ensured this is an object");
        node = map.entry(*key).or_insert(Value::Object(serde_json::Map::new()));
    }
    *node = Value::String(new_value.to_string());
}

fn apply_env_overrides(config: &mut Value, env: &EnvOverrides) {
    if let Some(v) = &env.provider_kind {
        set_nested_string(config, &["provider", "kind"], v);
    }
    if let Some(v) = &env.model_kind {
        set_nested_string(config, &["model", "kind"], v);
    }
    if let Some(v) = &env.openai_model {
        set_nested_string(config, &["model", "name"], v);
    }
    if let Some(v) = &env.core_binary {
        set_nested_string(config, &["core", "binary"], v);
    }
    if let Some(v) = &env.storage_sqlite_path {
        set_nested_string(config, &["storage", "sqlite_path"], v);
    }
}

/// Port of `smart_hedge.config.load_config`.
///
/// Unlike Python, `project_root` is a required parameter rather than a
/// value this crate derives from `__file__` — a compiled binary has no
/// clean equivalent of "the directory containing this source file", and
/// guessing (e.g. current working directory) is a decision for the future
/// CLI/dashboard entry point to make explicitly, not something to bury in
/// a library crate. Pass `std::env::current_dir()` for parity with running
/// the Python CLI from the repository root.
pub fn load_config(
    path: Option<&Path>,
    env: &EnvOverrides,
    project_root: &Path,
) -> Result<LoadedConfig, ConfigError> {
    let mut merged = default_config_json();

    let (config_dir, config_path) = match path {
        Some(raw_path) => {
            let expanded = expand_user(&raw_path.to_string_lossy());
            let text = std::fs::read_to_string(&expanded)?;
            let user_value: Value =
                serde_json::from_str(&text).map_err(|e| ConfigError::InvalidJson(e.to_string()))?;
            if !user_value.is_object() {
                return Err(ConfigError::RootNotAnObject);
            }
            deep_merge(&mut merged, &user_value);
            let dir = expanded
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| project_root.to_path_buf());
            (dir, Some(expanded))
        }
        None => (project_root.to_path_buf(), None),
    };

    apply_env_overrides(&mut merged, env);

    let config: Config =
        serde_json::from_value(merged).map_err(|e| ConfigError::SchemaMismatch(e.to_string()))?;

    // Hard stop: this research project deliberately has no live mode.
    if config.mode.to_lowercase() != "paper" {
        return Err(ConfigError::LiveModeNotSupported);
    }
    if !config.policy.paper_only {
        return Err(ConfigError::PolicyPaperOnlyRequired);
    }

    Ok(LoadedConfig { config, config_dir, config_path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strike_spec::StrikeSpec;

    #[test]
    fn loads_defaults_with_no_override_file() {
        let loaded = load_config(None, &EnvOverrides::default(), Path::new("/root")).unwrap();
        assert_eq!(loaded.config.mode, "paper");
        assert_eq!(loaded.config.provider.kind, "synthetic");
        assert_eq!(loaded.config_dir, PathBuf::from("/root"));
        assert!(loaded.config_path.is_none());
        assert_eq!(loaded.config.contracts.len(), 1);
        assert!(loaded.config.contracts.contains_key("SPY"));
    }

    #[test]
    fn env_overrides_apply_on_top_of_defaults() {
        let env = EnvOverrides {
            provider_kind: Some("alpaca-readonly".to_string()),
            model_kind: Some("openai".to_string()),
            openai_model: Some("gpt-test".to_string()),
            core_binary: Some("/opt/core".to_string()),
            storage_sqlite_path: Some("/tmp/decisions.sqlite3".to_string()),
        };
        let loaded = load_config(None, &env, Path::new("/root")).unwrap();
        assert_eq!(loaded.config.provider.kind, "alpaca-readonly");
        assert_eq!(loaded.config.model.kind, "openai");
        assert_eq!(loaded.config.model.name, "gpt-test");
        assert_eq!(loaded.config.core.binary, "/opt/core");
        assert_eq!(loaded.config.storage.sqlite_path, "/tmp/decisions.sqlite3");
    }

    #[test]
    fn model_registry_defaults_to_empty_preserving_legacy_kind_name_selection() {
        let loaded = load_config(None, &EnvOverrides::default(), Path::new("/root")).unwrap();
        assert!(loaded.config.model.models.is_empty());
    }

    #[test]
    fn model_registry_can_be_configured_with_named_uris() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-models-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            r#"{"model": {"models": {"default": "heuristic://default", "aggressive": "openai://gpt-4.1"}}}"#,
        )
        .unwrap();

        let loaded = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root")).unwrap();
        assert_eq!(loaded.config.model.models.get("default"), Some(&"heuristic://default".to_string()));
        assert_eq!(loaded.config.model.models.get("aggressive"), Some(&"openai://gpt-4.1".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_brand_new_contract_symbol_gets_only_the_fields_it_specifies_plus_defaults() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-newcontract-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        // "QQQ" does not exist in the built-in defaults (only "SPY" does),
        // so the deep-merge cannot merge SPY's fields onto it — it must
        // deserialize from exactly these three fields plus per-field
        // defaults, matching `core_bridge.py`'s `contract.get(key, default)`
        // tolerance.
        std::fs::write(
            &config_path,
            r#"{"contracts": {"QQQ": {"strike": 50.0, "days_to_expiry": 14.0, "implied_volatility": 0.30}}}"#,
        )
        .unwrap();

        let loaded = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root")).unwrap();
        assert_eq!(loaded.config.contracts.len(), 2); // SPY (default) + QQQ (new)
        let qqq = &loaded.config.contracts["QQQ"];
        assert_eq!(qqq.strike, StrikeSpec::Fixed(50.0));
        assert_eq!(qqq.option_type, "call");
        assert_eq!(qqq.exercise_style, "american");
        assert_eq!(qqq.multiplier, 100.0);
        assert_eq!(qqq.current_shares, 0.0);
        assert_eq!(qqq.contracts, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Regression test for the SDH-LLR-025 correction: a contract symbol
    /// that specifies `expiry` but not `days_to_expiry` must still load
    /// successfully — `days_to_expiry` defaults to `30.0` at the config
    /// layer, and the dynamic override from `expiry` happens later, in
    /// `smart-hedge-engine`.
    #[test]
    fn a_contract_symbol_with_only_an_expiry_date_needs_no_days_to_expiry() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-expiryonly-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            r#"{"contracts": {"QQQ": {"strike": 50.0, "expiry": "2026-12-19", "implied_volatility": 0.30}}}"#,
        )
        .unwrap();

        let loaded = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root")).unwrap();
        let qqq = &loaded.config.contracts["QQQ"];
        assert_eq!(qqq.expiry.as_deref(), Some("2026-12-19"));
        assert_eq!(qqq.days_to_expiry, 30.0); // config-layer default; engine.rs will override it

        std::fs::remove_dir_all(&dir).ok();
    }

    /// SDH-LLR-131: a configured `"strike": "ATM"` must load successfully
    /// (case-insensitively) — the earlier plain-`f64` schema would have
    /// rejected this at config-load time even though it's valid Python
    /// input.
    #[test]
    fn atm_strike_literal_loads_successfully_case_insensitively() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-atmstrike-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            r#"{"contracts": {"QQQ": {"strike": "atm", "days_to_expiry": 30.0, "implied_volatility": 0.30}}}"#,
        )
        .unwrap();

        let loaded = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root")).unwrap();
        assert_eq!(loaded.config.contracts["QQQ"].strike, StrikeSpec::Atm);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_contract_symbol_missing_a_required_field_fails_fast_at_load_time() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-badcontract-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        // Missing `implied_volatility`, which Python only discovers is
        // absent when that specific symbol is later priced.
        std::fs::write(&config_path, r#"{"contracts": {"QQQ": {"strike": 50.0, "days_to_expiry": 14.0}}}"#).unwrap();

        let result = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root"));
        assert!(matches!(result, Err(ConfigError::SchemaMismatch(_))));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn user_config_file_deep_merges_over_defaults() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"{"policy": {"max_spread_bps": 99.0}}"#).unwrap();

        let loaded = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root")).unwrap();
        assert_eq!(loaded.config.policy.max_spread_bps, 99.0);
        // Untouched defaults must survive the merge.
        assert_eq!(loaded.config.policy.max_quote_age_seconds, 45.0);
        assert_eq!(loaded.config_dir, dir);
        assert_eq!(loaded.config_path, Some(config_path.clone()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_object_config_file_is_rejected() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-arr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"[1, 2, 3]"#).unwrap();

        let result = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root"));
        assert!(matches!(result, Err(ConfigError::RootNotAnObject)));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_json_file_is_rejected() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-badjson-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, "{not valid json").unwrap();

        let result = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root"));
        assert!(matches!(result, Err(ConfigError::InvalidJson(_))));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_config_file_is_an_io_error() {
        let result = load_config(
            Some(Path::new("/definitely/does/not/exist/config.json")),
            &EnvOverrides::default(),
            Path::new("/root"),
        );
        assert!(matches!(result, Err(ConfigError::Io(_))));
    }

    #[test]
    fn non_paper_mode_is_rejected() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"{"mode": "live"}"#).unwrap();

        let result = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root"));
        assert!(matches!(result, Err(ConfigError::LiveModeNotSupported)));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn paper_only_false_is_rejected_even_if_mode_is_paper() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-notpaperonly-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"{"policy": {"paper_only": false}}"#).unwrap();

        let result = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root"));
        assert!(matches!(result, Err(ConfigError::PolicyPaperOnlyRequired)));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn schema_mismatch_is_reported_instead_of_silently_ignored() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-config-test-schema-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        // `max_spread_bps` must be a number, not a string.
        std::fs::write(&config_path, r#"{"policy": {"max_spread_bps": "not-a-number"}}"#).unwrap();

        let result = load_config(Some(&config_path), &EnvOverrides::default(), Path::new("/root"));
        assert!(matches!(result, Err(ConfigError::SchemaMismatch(_))));

        std::fs::remove_dir_all(&dir).ok();
    }
}
