use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

use serde_json::json;
use smart_hedge_config::{EnvOverrides, LoadedConfig};
use smart_hedge_engine::{ContractOverrides, SmartHedgeEngine};

use crate::args::ContractOverrideArgs;
use crate::error::CliError;

/// Rust binaries have no equivalent of Python's `Path(__file__).parents[2]`
/// — the compiled executable can be copied anywhere. `smart-hedge.py`'s
/// CLI is always run from the repository root in practice; this matches
/// that by using the process's current directory, as documented on
/// `smart_hedge_config::load_config`.
pub fn project_root() -> Result<PathBuf, CliError> {
    Ok(std::env::current_dir()?)
}

pub fn cpp_source_path(project_root: &Path) -> PathBuf {
    project_root.join("cpp").join("smart_dynamic_hedge.cpp")
}

/// Port of `cli.py`'s `selected = path or os.getenv("SMART_HEDGE_CONFIG")`.
pub fn resolve_config_path(explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit.or_else(|| std::env::var("SMART_HEDGE_CONFIG").ok().map(PathBuf::from))
}

pub fn load_config(config_path: Option<PathBuf>, project_root: &Path) -> Result<LoadedConfig, CliError> {
    Ok(smart_hedge_config::load_config(config_path.as_deref(), &EnvOverrides::from_process_env(), project_root)?)
}

fn to_engine_overrides(o: ContractOverrideArgs) -> ContractOverrides {
    ContractOverrides {
        strike: o.strike,
        implied_volatility: o.vol,
        days_to_expiry: o.days,
        current_shares: o.current_shares,
        contracts: o.contracts,
    }
}

pub fn cmd_build_core(config_path: Option<PathBuf>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let binary = smart_hedge_core_bridge::build_core(&loaded, &root, &cpp_source)?;
    println!("{}", binary.display());
    Ok(0)
}

pub fn cmd_once(config_path: Option<PathBuf>, symbol: &str, overrides: ContractOverrideArgs) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let decision = engine.recommendation(symbol, &to_engine_overrides(overrides))?;
    println!("{}", serde_json::to_string_pretty(&decision).expect("decision is always serializable"));
    Ok(0)
}

pub fn cmd_loop(
    config_path: Option<PathBuf>,
    symbol: &str,
    overrides: ContractOverrideArgs,
    interval: f64,
) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let engine_overrides = to_engine_overrides(overrides);
    let sleep_for = Duration::from_secs_f64(interval.max(1.0));

    // Matches Python's `except KeyboardInterrupt: return 0` in spirit: this
    // loop runs until the process receives Ctrl+C, at which point the OS
    // terminates it directly (Rust has no built-in signal handling without
    // a dependency, and adding one is out of scope for this pass) rather
    // than unwinding back through this function.
    loop {
        let decision = engine.recommendation(symbol, &engine_overrides)?;
        println!("{}", format_loop_line(&decision));
        std::thread::sleep(sleep_for);
    }
}

fn format_loop_line(decision: &serde_json::Value) -> String {
    let p = &decision["policy"];
    let q = &decision["snapshot"]["quote"];
    let m = &decision["model_assessment"];
    let bid = q["bid"].as_f64().unwrap_or(f64::NAN);
    let ask = q["ask"].as_f64().unwrap_or(f64::NAN);
    format!(
        "{} {} mid={:.4} regime={} action={} preview={:.3} blockers={}",
        decision["created_at"].as_str().unwrap_or(""),
        decision["symbol"].as_str().unwrap_or(""),
        (bid + ask) / 2.0,
        m["regime"].as_str().unwrap_or(""),
        p["action"].as_str().unwrap_or(""),
        p["paper_trade_preview_shares"].as_f64().unwrap_or(f64::NAN),
        p["blocking_reasons"],
    )
}

pub fn cmd_replay(config_path: Option<PathBuf>, decision_id: &str) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let value = engine.replay(decision_id)?;
    println!("{}", serde_json::to_string_pretty(&value).expect("replayed decision is always serializable"));
    Ok(0)
}

pub fn cmd_recent(config_path: Option<PathBuf>, limit: i64, symbol: Option<&str>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let values = engine.recent(limit, symbol)?;
    println!("{}", serde_json::to_string_pretty(&values).expect("recent decisions are always serializable"));
    Ok(0)
}

pub fn cmd_self_test(config_path: Option<PathBuf>, symbol: &str) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);

    let binary = smart_hedge_core_bridge::ensure_core(&loaded, &root, &cpp_source)?;
    let status = ProcessCommand::new(&binary)
        .arg("--self-test")
        .status()
        .map_err(|e| CliError::SelfTestFailed(format!("failed to run {}: {e}", binary.display())))?;
    if !status.success() {
        return Err(CliError::SelfTestFailed(format!("{} --self-test exited with {status}", binary.display())));
    }

    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let value = engine.recommendation(symbol, &ContractOverrides::default())?;
    if value["mode"] != json!("paper") {
        return Err(CliError::SelfTestFailed("mode was not paper".to_string()));
    }
    if value["policy"]["live_execution_allowed"] != json!(false) {
        return Err(CliError::SelfTestFailed("live_execution_allowed was not false".to_string()));
    }
    if value["audit"]["broker_order_endpoint_present"] != json!(false) {
        return Err(CliError::SelfTestFailed("broker_order_endpoint_present was not false".to_string()));
    }

    let decision_id = value["decision_id"].as_str().expect("decision_id is always a string");
    let replay = engine.replay(decision_id)?;
    if replay["audit"]["stored_content_hash_valid"] != json!(true) {
        return Err(CliError::SelfTestFailed("stored_content_hash_valid was not true on replay".to_string()));
    }

    println!("rust integration self-test: PASS");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_loop_line_renders_the_expected_shape() {
        let decision = json!({
            "created_at": "2026-07-19T00:00:00Z",
            "symbol": "SPY",
            "snapshot": {"quote": {"bid": 99.0, "ask": 101.0}},
            "model_assessment": {"regime": "calm"},
            "policy": {"action": "hold", "paper_trade_preview_shares": 1.5, "blocking_reasons": []},
        });
        let line = format_loop_line(&decision);
        assert_eq!(line, "2026-07-19T00:00:00Z SPY mid=100.0000 regime=calm action=hold preview=1.500 blockers=[]");
    }

    #[test]
    fn resolve_config_path_prefers_the_explicit_flag_over_the_env_var() {
        // No env var set in this process by default; explicit always wins when present.
        let resolved = resolve_config_path(Some(PathBuf::from("explicit.json")));
        assert_eq!(resolved, Some(PathBuf::from("explicit.json")));
    }

    #[test]
    fn resolve_config_path_is_none_with_neither_flag_nor_env_var() {
        // SMART_HEDGE_CONFIG is not expected to be set in the test environment.
        if std::env::var("SMART_HEDGE_CONFIG").is_err() {
            assert_eq!(resolve_config_path(None), None);
        }
    }
}
