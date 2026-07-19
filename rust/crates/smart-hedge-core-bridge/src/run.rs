use std::path::Path;
use std::process::Command;
use std::time::Duration;

use smart_hedge_config::{ContractConfig, LoadedConfig};
use smart_hedge_models::CoreResponse;

use crate::build::ensure_core;
use crate::error::CoreError;
use crate::run_with_timeout::run_command_with_timeout;

/// Port of `smart_hedge.core_bridge.run_core`: resolves/builds the binary,
/// invokes it with the same CLI flags Python constructs, and parses its
/// `--json` output into a typed `CoreResponse` (rather than a raw dict —
/// see `smart_hedge_models::core_response` for why that also subsumes
/// Python's separate "hedge"/"greeks" key-presence check).
///
/// `strike` is an explicit, already-resolved `f64` parameter (like
/// `spot`) rather than read from `contract.strike` — `ContractConfig`'s
/// strike may be the unresolved `"ATM"` literal (see
/// `smart_hedge_config::StrikeSpec`); resolving that against a live quote
/// is `smart-hedge-engine`'s job, done before this function is ever
/// called, matching Python's `engine.py` flow where `contract["strike"]`
/// is always a plain float by the time `run_core` reads it.
pub fn run_core(
    loaded: &LoadedConfig,
    project_root: &Path,
    cpp_source: &Path,
    contract: &ContractConfig,
    spot: f64,
    strike: f64,
) -> Result<CoreResponse, CoreError> {
    let binary = ensure_core(loaded, project_root, cpp_source)?;

    let mut command = Command::new(&binary);
    command
        .arg("--spot")
        .arg(spot.to_string())
        .arg("--strike")
        .arg(strike.to_string())
        .arg("--rate")
        .arg(contract.rate.to_string())
        .arg("--dividend-yield")
        .arg(contract.dividend_yield.to_string())
        .arg("--vol")
        .arg(contract.implied_volatility.to_string())
        .arg("--days")
        .arg(contract.days_to_expiry.to_string())
        .arg("--type")
        .arg(&contract.option_type)
        .arg("--style")
        .arg(&contract.exercise_style)
        .arg("--contracts")
        .arg(contract.contracts.to_string())
        .arg("--multiplier")
        .arg(contract.multiplier.to_string())
        .arg("--current-shares")
        .arg(contract.current_shares.to_string())
        .arg("--tree-steps")
        .arg(loaded.config.core.tree_steps.to_string())
        .arg("--no-trade-band")
        .arg(contract.base_no_trade_band_shares.to_string())
        .arg("--json");

    let timeout = Duration::from_secs_f64(loaded.config.core.timeout_seconds.max(0.0));
    let output = run_command_with_timeout(&mut command, timeout)?;

    if !output.status.success() {
        return Err(CoreError::NonZeroExit { code: output.status.code(), stderr: output.stderr });
    }

    serde_json::from_str(&output.stdout).map_err(|e| CoreError::InvalidJson(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;
    use std::path::PathBuf;

    fn find_repo_root() -> PathBuf {
        // rust/crates/smart-hedge-core-bridge -> repo root is 3 levels up.
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
    }

    /// Integration test against the real C++ binary. Skips (passes
    /// trivially) if no toolchain is available to build it, rather than
    /// failing CI environments that don't have a C++ compiler — the
    /// pure-logic tests elsewhere in this crate don't have that
    /// dependency, only this one does.
    #[test]
    fn run_core_against_the_real_cpp_binary_produces_a_parseable_response() {
        let root = find_repo_root();
        let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");
        if !cpp_source.exists() {
            eprintln!("skipping: {} not found", cpp_source.display());
            return;
        }
        if crate::which::which("cmake").is_none()
            && crate::which::which("g++").is_none()
            && crate::which::which("clang++").is_none()
        {
            eprintln!("skipping: no cmake/g++/clang++ toolchain found on PATH");
            return;
        }

        let loaded = smart_hedge_config::load_config(None, &EnvOverrides::default(), &root).unwrap();
        let contract = loaded.config.contracts.get("SPY").expect("default config has an SPY contract").clone();

        let result = run_core(&loaded, &root, &cpp_source, &contract, 100.0, 100.0);
        let response = match result {
            Ok(r) => r,
            Err(e) => panic!("run_core failed: {e}"),
        };
        assert!(response.hedge.target_stock_shares.is_finite());
        assert_eq!(response.inputs.option_type, "put");
    }
}
