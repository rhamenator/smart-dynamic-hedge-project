//! A point-in-time backtester — `docs/ROADMAP.md` Phase 4's named gap.
//! Steps a deterministic synthetic price path day by day through the
//! *same* real pipeline `smart-hedge-engine::recommendation` uses for a
//! live decision (`build_features` → the real C++ core → the heuristic
//! adviser → `evaluate_policy`), threading each day's resulting
//! `paper_trade_preview_shares` forward into the next day's
//! `current_shares` and decrementing `days_to_expiry` — so an option
//! genuinely decays toward expiry over the run, not a fixed snapshot
//! replayed unchanged.
//!
//! "Point-in-time" here means what `smart_hedge_data::SyntheticProvider::snapshot_at`
//! already guarantees: a given `(symbol, timestamp)` pair always produces
//! the same snapshot, and a day's snapshot is generated only from that
//! day's own timestamp — there is no code path by which a later day's
//! synthetic price could leak into an earlier day's decision. No
//! look-ahead is possible by construction, not merely by convention.
//!
//! **This is a synthetic backtester, not a historical one.** There is no
//! real market-data history anywhere in this system (see
//! `smart-hedge-data`'s own `AlpacaReadOnlyProvider`, which only ever
//! fetches *current* quotes/bars, never a historical archive). README.md
//! "What this does not prove" already lists real historical option-chain
//! data and realistic exercise/assignment behavior as future work; this
//! crate does not change that. What it *does* prove: the full
//! feature→core→adviser→policy pipeline behaves sensibly over a
//! multi-day run with evolving inputs, using only synthetic data — the
//! "local synthetic/fixture mode needs no paid service" acceptance
//! criterion applied to a backtest, not just a single `once` call.

use std::path::Path;

use serde::{Deserialize, Serialize};
use smart_hedge_config::LoadedConfig;
use smart_hedge_data::SyntheticProvider;
use smart_hedge_engine::{resolve_contract, resolved_strike, ContractOverrides, EngineError};
use smart_hedge_model_advisor::{Advisor, HeuristicAdvisor};
use smart_hedge_models::TimestampUtc;

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyResult {
    pub day_index: u32,
    pub timestamp: String,
    pub spot: f64,
    pub days_to_expiry: f64,
    pub current_shares_before: f64,
    pub target_stock_shares: f64,
    pub trade_shares: f64,
    pub current_shares_after: f64,
    pub action: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    pub symbol: String,
    pub days: Vec<DailyResult>,
    /// Sum of `|trade_shares|` across every day — how much hedging
    /// activity the run implied in total.
    pub total_turnover_shares: f64,
    /// Number of days whose policy actually proposed a trade
    /// (`action == "paper_rebalance_preview"`), out of `days.len()`.
    pub trading_days: usize,
    pub final_current_shares: f64,
}

pub struct BacktestConfig {
    pub symbol: String,
    pub num_days: u32,
    pub start: TimestampUtc,
}

/// Runs the backtest. Fails only the way a single `recommendation` call
/// can fail (unknown symbol, invalid contract config, C++ core failure)
/// — a per-day failure aborts the whole run rather than silently
/// skipping a day, matching this codebase's fail-fast convention
/// elsewhere.
pub fn run_backtest(
    loaded: &LoadedConfig,
    project_root: &Path,
    cpp_source: &Path,
    config: &BacktestConfig,
) -> Result<BacktestReport, EngineError> {
    let symbol = config.symbol.to_uppercase();
    let provider = SyntheticProvider::new(loaded.clone());
    let advisor = HeuristicAdvisor;

    let initial_days_to_expiry = loaded
        .config
        .contracts
        .get(&symbol)
        .map(|c| c.days_to_expiry)
        .ok_or_else(|| EngineError::UnknownSymbol(symbol.clone()))?;

    let mut running_current_shares = loaded.config.contracts.get(&symbol).map(|c| c.current_shares).unwrap_or(0.0);
    let mut days = Vec::with_capacity(config.num_days as usize);
    let mut total_turnover = 0.0;
    let mut trading_days = 0usize;

    for day_index in 0..config.num_days {
        let now = TimestampUtc::from_unix(config.start.unix_seconds() + i64::from(day_index) * SECONDS_PER_DAY, 0);
        let snapshot = provider.snapshot_at(&symbol, now);
        let midpoint = snapshot.quote.midpoint();

        let remaining_days = (initial_days_to_expiry - f64::from(day_index)).max(0.0);
        let overrides =
            ContractOverrides { days_to_expiry: Some(remaining_days), current_shares: Some(running_current_shares), ..Default::default() };
        let contract = resolve_contract(&loaded.config, &symbol, &overrides, midpoint, now)?;
        let strike = resolved_strike(&contract);

        let features = smart_hedge_features::build_features(&snapshot, &loaded.config.features);
        let core = smart_hedge_core_bridge::run_core(loaded, project_root, cpp_source, &contract, midpoint, strike)?;
        let assessment = advisor.assess(&snapshot, &features, &core, &contract).expect("HeuristicAdvisor::assess is infallible");
        let policy = smart_hedge_policy::evaluate_policy(&loaded.config, &snapshot, &features, &core, &assessment, now);

        let trade_shares = policy.paper_trade_preview_shares;
        total_turnover += trade_shares.abs();
        if policy.action == "paper_rebalance_preview" {
            trading_days += 1;
        }
        let current_shares_before = running_current_shares;
        running_current_shares += trade_shares;

        days.push(DailyResult {
            day_index,
            timestamp: now.to_iso_string(),
            spot: midpoint,
            days_to_expiry: remaining_days,
            current_shares_before,
            target_stock_shares: policy.target_stock_shares,
            trade_shares,
            current_shares_after: running_current_shares,
            action: policy.action,
            blocking_reasons: policy.blocking_reasons,
        });
    }

    Ok(BacktestReport { symbol, days, total_turnover_shares: total_turnover, trading_days, final_current_shares: running_current_shares })
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn loaded_config() -> LoadedConfig {
        smart_hedge_config::load_config(None, &EnvOverrides::default(), std::path::Path::new("/root")).unwrap()
    }

    fn nonexistent_cpp() -> std::path::PathBuf {
        std::env::temp_dir().join("smart-hedge-backtest-test-nonexistent.cpp")
    }

    #[test]
    fn unknown_symbol_is_rejected_before_running_any_day() {
        let loaded = loaded_config();
        let config = BacktestConfig { symbol: "NOPE".to_string(), num_days: 5, start: TimestampUtc::parse_flexible("2026-01-01T00:00:00Z").unwrap() };
        let result = run_backtest(&loaded, Path::new("/root"), &nonexistent_cpp(), &config);
        assert!(matches!(result, Err(EngineError::UnknownSymbol(_))));
    }

    #[test]
    fn zero_days_produces_an_empty_but_valid_report() {
        let loaded = loaded_config();
        let config = BacktestConfig { symbol: "SPY".to_string(), num_days: 0, start: TimestampUtc::parse_flexible("2026-01-01T00:00:00Z").unwrap() };
        // No C++ core needed at all when there are zero days to run --
        // this should succeed even with a nonexistent cpp_source/binary.
        let report = run_backtest(&loaded, Path::new("/root"), &nonexistent_cpp(), &config).unwrap();
        assert!(report.days.is_empty());
        assert_eq!(report.trading_days, 0);
        assert_eq!(report.total_turnover_shares, 0.0);
    }

    fn find_repo_root() -> std::path::PathBuf {
        // rust/crates/smart-hedge-backtest -> repo root is 3 levels up.
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
    }

    /// Real integration test against the real C++ binary, same
    /// skip-gracefully-without-a-toolchain convention
    /// `smart-hedge-core-bridge`'s own integration test uses.
    #[test]
    fn a_real_multi_day_run_decays_days_to_expiry_and_evolves_current_shares() {
        let root = find_repo_root();
        let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");
        if !cpp_source.exists() {
            eprintln!("skipping: {} not found", cpp_source.display());
            return;
        }
        if smart_hedge_core_bridge::which_tool("cmake").is_none()
            && smart_hedge_core_bridge::which_tool("g++").is_none()
            && smart_hedge_core_bridge::which_tool("clang++").is_none()
        {
            eprintln!("skipping: no cmake/g++/clang++ toolchain found on PATH");
            return;
        }

        let loaded = loaded_config();
        let config = BacktestConfig { symbol: "SPY".to_string(), num_days: 10, start: TimestampUtc::parse_flexible("2026-01-01T00:00:00Z").unwrap() };
        let report = run_backtest(&loaded, &root, &cpp_source, &config).unwrap();

        assert_eq!(report.days.len(), 10);
        assert_eq!(report.symbol, "SPY");

        // days_to_expiry strictly decreases day over day (the default
        // SPY contract's 30-day expiry is far from hitting the zero
        // floor within 10 simulated days).
        for window in report.days.windows(2) {
            assert!(
                window[1].days_to_expiry < window[0].days_to_expiry,
                "days_to_expiry should decay: day {} had {}, day {} had {}",
                window[0].day_index,
                window[0].days_to_expiry,
                window[1].day_index,
                window[1].days_to_expiry
            );
        }

        // current_shares_after on day N always feeds current_shares_before
        // on day N+1 -- the whole point of a point-in-time backtest, not
        // a series of independent, unrelated single-day previews.
        for window in report.days.windows(2) {
            assert_eq!(window[0].current_shares_after, window[1].current_shares_before);
        }
        assert_eq!(report.days.last().unwrap().current_shares_after, report.final_current_shares);

        // Every day produced a finite spot price and a real policy action.
        for day in &report.days {
            assert!(day.spot.is_finite() && day.spot > 0.0);
            assert!(!day.action.is_empty());
        }
    }
}
