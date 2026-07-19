//! End-to-end tests of `SmartHedgeEngine` against the real C++ core binary,
//! wired to the synthetic provider and heuristic adviser (the zero-network-
//! dependency path). Each is tagged with the requirement it verifies.
//!
//! Like `smart-hedge-core-bridge`'s own `run_core` test, these skip (pass
//! trivially) when no C++ source/toolchain is available, rather than
//! failing environments without one — only this module has that
//! dependency, the rest of the crate's tests are pure logic.

use std::path::{Path, PathBuf};

use smart_hedge_config::EnvOverrides;
use smart_hedge_model_advisor::{Advisor, AdvisorError};
use smart_hedge_models::{CoreResponse, FeatureSet, MarketSnapshot, ModelAssessment};

use crate::contract::ContractOverrides;
use crate::engine::SmartHedgeEngine;
use crate::error::EngineError;

fn find_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

/// A stub adviser that always fails, to exercise the fallback path
/// (SDH-LLR-057) without depending on a real fallible adviser existing yet.
struct AlwaysFailsAdvisor;

impl Advisor for AlwaysFailsAdvisor {
    fn assess(
        &self,
        _snapshot: &MarketSnapshot,
        _features: &FeatureSet,
        _core: &CoreResponse,
        _contract: &smart_hedge_config::ContractConfig,
    ) -> Result<ModelAssessment, AdvisorError> {
        Err(AdvisorError("stub adviser always fails".to_string()))
    }

    fn name(&self) -> &'static str {
        "AlwaysFailsAdvisor"
    }
}

/// Returns `None` (meaning: skip the calling test) if there's no C++
/// toolchain available to build/run the deterministic core against.
fn engine_or_skip(fallback_to_heuristic: bool) -> Option<(SmartHedgeEngine, PathBuf)> {
    let root = find_repo_root();
    let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");
    if !cpp_source.exists() {
        eprintln!("skipping: {} not found", cpp_source.display());
        return None;
    }
    if smart_hedge_core_bridge::which_tool("cmake").is_none()
        && smart_hedge_core_bridge::which_tool("g++").is_none()
        && smart_hedge_core_bridge::which_tool("clang++").is_none()
    {
        eprintln!("skipping: no cmake/g++/clang++ toolchain found on PATH");
        return None;
    }

    // Each test gets its own directory/sqlite file — tests run in parallel
    // threads within this one process, so keying only on the process ID (or
    // even pid+fallback flag, shared by two tests) let concurrent tests
    // race on the same sqlite file.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join(format!("smart-hedge-engine-itest-{}-{}-{n}", std::process::id(), fallback_to_heuristic));
    std::fs::create_dir_all(&dir).ok();
    let config_path = dir.join("config.json");
    let sqlite_path = dir.join("decisions.sqlite3");
    std::fs::write(
        &config_path,
        format!(
            r#"{{"model": {{"fallback_to_heuristic": {fallback_to_heuristic}}}, "storage": {{"sqlite_path": "{}"}}}}"#,
            sqlite_path.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .unwrap();

    let loaded = smart_hedge_config::load_config(Some(&config_path), &EnvOverrides::default(), &root).unwrap();
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source).expect("engine construction should succeed");
    Some((engine, dir))
}

fn cleanup(dir: &Path) {
    std::fs::remove_dir_all(dir).ok();
}

/// SDH-LLR-133/134/136: a recommendation against the synthetic/heuristic
/// path produces a fully populated decision, and `health()` reports the
/// real provider/adviser names in use.
#[test]
fn recommendation_and_health_report_the_synthetic_heuristic_path() {
    let Some((engine, dir)) = engine_or_skip(true) else { return };

    let health = engine.health();
    assert_eq!(health["provider"], "SyntheticProvider");
    assert_eq!(health["advisor"], "HeuristicAdvisor");
    assert_eq!(health["broker_order_endpoint_present"], false);

    let decision = engine.recommendation("SPY", &ContractOverrides::default()).expect("recommendation should succeed");
    assert!(decision["decision_id"].is_string());
    assert_eq!(decision["symbol"], "SPY");
    assert_eq!(decision["audit"]["fallback_used"], false);
    assert_eq!(decision["audit"]["secrets_sent_to_model"], false);
    assert!(decision["audit"]["decision_store_content_hash"].is_string());
    assert!(decision["deterministic_core"]["hedge"]["target_stock_shares"].is_number());

    cleanup(&dir);
}

/// SDH-LLR-135: `replay` returns the exact stored decision, tagged as a
/// replay rather than re-run against the network/core.
#[test]
fn replay_returns_the_stored_decision_tagged_as_a_replay() {
    let Some((engine, dir)) = engine_or_skip(true) else { return };

    let decision = engine.recommendation("SPY", &ContractOverrides::default()).unwrap();
    let decision_id = decision["decision_id"].as_str().unwrap();

    let replayed = engine.replay(decision_id).expect("replay should find the stored decision");
    assert_eq!(replayed["decision_id"], decision["decision_id"]);
    assert_eq!(replayed["audit"]["replay_mode"], "stored_inputs_and_outputs_no_network");

    let missing = engine.replay("does-not-exist");
    assert!(matches!(missing, Err(EngineError::DecisionNotFound(_))));

    cleanup(&dir);
}

/// `recent` returns decisions most-recent-first and honors the symbol filter.
#[test]
fn recent_returns_stored_decisions_filtered_by_symbol() {
    let Some((engine, dir)) = engine_or_skip(true) else { return };

    engine.recommendation("SPY", &ContractOverrides::default()).unwrap();
    let recent = engine.recent(10, Some("SPY")).unwrap();
    assert!(!recent.is_empty());
    assert!(recent.iter().all(|d| d["symbol"] == "SPY"));

    let none_for_other_symbol = engine.recent(10, Some("ZZZZ")).unwrap();
    assert!(none_for_other_symbol.is_empty());

    cleanup(&dir);
}

/// SDH-LLR-057: when the active adviser fails and
/// `model.fallback_to_heuristic` is true, the engine falls back to
/// `HeuristicAdvisor` transparently and records why.
#[test]
fn adviser_failure_falls_back_to_heuristic_when_enabled() {
    let Some((engine_template, dir)) = engine_or_skip(true) else { return };
    let root = find_repo_root();
    let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");
    let loaded = smart_hedge_config::load_config(
        Some(&dir.join("config.json")),
        &EnvOverrides::default(),
        &root,
    )
    .unwrap();
    drop(engine_template);

    let provider = smart_hedge_data::SyntheticProvider::new(loaded.clone());
    let engine = SmartHedgeEngine::with_components(
        loaded,
        root,
        cpp_source,
        Box::new(provider),
        Box::new(AlwaysFailsAdvisor),
    )
    .unwrap();

    let decision = engine.recommendation("SPY", &ContractOverrides::default()).expect("fallback should succeed");
    assert_eq!(decision["audit"]["fallback_used"], true);
    assert!(!decision["audit"]["fallback_reason"].as_str().unwrap().is_empty());
    assert_eq!(decision["model_assessment"]["advisor_kind"], "heuristic");
    assert!(!decision["model_assessment"]["fallback_reason"].as_str().unwrap().is_empty());

    cleanup(&dir);
}

/// SDH-LLR-057: when `model.fallback_to_heuristic` is false, an adviser
/// failure propagates instead of silently falling back.
#[test]
fn adviser_failure_propagates_when_fallback_disabled() {
    let Some((engine_template, dir)) = engine_or_skip(false) else { return };
    let root = find_repo_root();
    let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");
    let loaded = smart_hedge_config::load_config(
        Some(&dir.join("config.json")),
        &EnvOverrides::default(),
        &root,
    )
    .unwrap();
    drop(engine_template);

    let provider = smart_hedge_data::SyntheticProvider::new(loaded.clone());
    let engine = SmartHedgeEngine::with_components(
        loaded,
        root,
        cpp_source,
        Box::new(provider),
        Box::new(AlwaysFailsAdvisor),
    )
    .unwrap();

    let result = engine.recommendation("SPY", &ContractOverrides::default());
    assert!(matches!(result, Err(EngineError::AdvisorFailedAndFallbackDisabled(_))));

    cleanup(&dir);
}

/// An unknown symbol (no `contracts.<SYMBOL>` entry) is rejected before
/// ever reaching the core or adviser.
#[test]
fn unknown_symbol_is_rejected_before_calling_the_core() {
    let Some((engine, dir)) = engine_or_skip(true) else { return };
    let result = engine.recommendation("NOPE", &ContractOverrides::default());
    assert!(matches!(result, Err(EngineError::UnknownSymbol(_))));
    cleanup(&dir);
}
