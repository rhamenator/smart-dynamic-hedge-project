//! Portfolio-level Greeks aggregation across multiple option positions —
//! Phase 4's "C++ portfolio pricing/Greeks/hedging expansion" in
//! `docs/ROADMAP.md`.
//!
//! **The C++ core is unchanged and remains authoritative for every
//! individual position's value/Greeks/hedge target** — this crate calls
//! `smart_hedge_core_bridge::run_core` once per configured contract
//! (exactly as `smart-hedge-engine::recommendation` already does for a
//! single symbol) and only performs pure arithmetic aggregation across
//! the results. No pricing math lives here, deliberately: aggregating
//! already-computed, already-tested per-position numbers is a much
//! smaller, lower-risk surface than teaching the deterministic core about
//! multiple positions directly.
//!
//! ## Why dollar-denominated aggregates, not raw per-underlying numbers
//!
//! A position's `target_stock_shares`/`option_position_delta_shares` are
//! share counts *in that position's own underlying* — SPY shares and QQQ
//! shares are not the same unit, so summing them across positions on
//! different underlyings would produce a number with no meaning (you
//! cannot hedge a "275 combined SPY+QQQ shares" delta with a single stock
//! order). Every aggregate this module actually sums is dollar-
//! denominated instead — dollar delta, dollar gamma P&L, dollar vega,
//! dollar theta, dollar rho, stock notional, option notional — which
//! *are* meaningfully additive across different underlyings, the same
//! convention real portfolio risk systems use. Per-position share counts
//! are still reported, just not summed.

use std::path::Path;

use serde::{Deserialize, Serialize};
use smart_hedge_config::LoadedConfig;
use smart_hedge_engine::{build_provider, resolve_contract, resolved_strike, ContractOverrides};
use smart_hedge_models::CoreResponse;

pub use smart_hedge_engine::EngineError as PortfolioError;

/// One position's full C++ core result, alongside the symbol it's for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPosition {
    pub symbol: String,
    pub core: CoreResponse,
}

/// Dollar-denominated risk aggregates across every position in a
/// portfolio, plus a few notional totals. See the module doc comment for
/// why these are dollar-denominated rather than raw share counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub position_count: usize,
    /// Sum of each position's `option_position_delta_shares × spot` — the
    /// dollar P&L equivalent of a $1 move in each position's own
    /// underlying, summed across underlyings.
    pub dollar_delta: f64,
    /// Sum of each position's `risk.position_gamma_pnl_for_1pct_move`
    /// (already dollar-denominated by the C++ core).
    pub dollar_gamma_pnl_for_1pct_move: f64,
    /// Sum of `contracts × multiplier × vega_per_vol_point` per position.
    pub dollar_vega_per_vol_point: f64,
    /// Sum of `contracts × multiplier × theta_per_calendar_day` per
    /// position — total portfolio time decay per calendar day.
    pub dollar_theta_per_day: f64,
    /// Sum of `contracts × multiplier × rho_per_rate_point` per position.
    pub dollar_rho_per_rate_point: f64,
    /// Sum of each position's `hedge.stock_notional` — total dollar value
    /// of the stock hedge across all positions.
    pub total_stock_notional: f64,
    /// Sum of `contracts × multiplier × pricing.model_price` per position
    /// — total dollar value of the option positions themselves.
    pub total_option_notional: f64,
}

/// Builds one `PortfolioPosition` per `symbol` in `symbols`, in order,
/// stopping at the first error (an unknown symbol, a market-data failure,
/// or a C++ core failure) — matching `smart-hedge-engine::recommendation`'s
/// own fail-fast behavior for a single symbol, extended to a list.
pub fn build_portfolio(
    loaded: &LoadedConfig,
    project_root: &Path,
    cpp_source: &Path,
    symbols: &[String],
) -> Result<Vec<PortfolioPosition>, PortfolioError> {
    let provider = build_provider(loaded)?;
    let now = smart_hedge_models::TimestampUtc::now();

    let mut positions = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        let snapshot = provider.snapshot(symbol)?;
        let midpoint = snapshot.quote.midpoint();
        let contract = resolve_contract(&loaded.config, symbol, &ContractOverrides::default(), midpoint, now)?;
        let strike = resolved_strike(&contract);
        let core = smart_hedge_core_bridge::run_core(loaded, project_root, cpp_source, &contract, midpoint, strike)?;
        positions.push(PortfolioPosition { symbol: symbol.clone(), core });
    }
    Ok(positions)
}

/// Aggregates already-computed positions into dollar-denominated
/// portfolio totals. Pure arithmetic, no I/O — the reason this is a
/// separate function from `build_portfolio` rather than folded into it:
/// tests can exercise the aggregation math directly against hand-built
/// `CoreResponse` fixtures without needing the C++ core or a market-data
/// provider at all.
pub fn summarize(positions: &[PortfolioPosition]) -> PortfolioSummary {
    let mut summary = PortfolioSummary { position_count: positions.len(), ..Default::default() };
    for position in positions {
        let core = &position.core;
        let scale = core.inputs.contracts as f64 * core.inputs.multiplier;
        summary.dollar_delta += core.hedge.option_position_delta_shares * core.inputs.spot;
        summary.dollar_gamma_pnl_for_1pct_move += core.risk.position_gamma_pnl_for_1pct_move;
        summary.dollar_vega_per_vol_point += scale * core.greeks.vega_per_vol_point;
        summary.dollar_theta_per_day += scale * core.greeks.theta_per_calendar_day;
        summary.dollar_rho_per_rate_point += scale * core.greeks.rho_per_rate_point;
        summary.total_stock_notional += core.hedge.stock_notional;
        summary.total_option_notional += scale * core.pricing.model_price;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_models::{CoreGreeks, CoreHedge, CoreInputs, CorePricing, CoreRisk};

    fn position(symbol: &str, spot: f64, delta_shares: f64, contracts: i64, multiplier: f64) -> PortfolioPosition {
        PortfolioPosition {
            symbol: symbol.to_string(),
            core: CoreResponse {
                engine_version: "test".to_string(),
                inputs: CoreInputs {
                    spot,
                    strike: spot,
                    rate: 0.045,
                    dividend_yield: 0.0,
                    volatility: 0.2,
                    days_to_expiry: 30.0,
                    option_type: "put".to_string(),
                    exercise_style: "american".to_string(),
                    contracts,
                    multiplier,
                    current_shares: 0.0,
                    tree_steps: 600,
                    base_no_trade_band_shares: 2.0,
                },
                pricing: CorePricing { model_price: 3.0, european_price: 2.9, early_exercise_premium: 0.1 },
                greeks: CoreGreeks {
                    delta: delta_shares / (contracts as f64 * multiplier),
                    gamma: 0.01,
                    vega_per_vol_point: 0.1,
                    theta_per_calendar_day: -0.02,
                    rho_per_rate_point: -0.05,
                },
                hedge: CoreHedge {
                    option_position_delta_shares: delta_shares,
                    target_stock_shares: delta_shares,
                    raw_trade_shares: delta_shares,
                    recommended_trade_shares: delta_shares,
                    action: "paper_rebalance_preview".to_string(),
                    stock_notional: delta_shares.abs() * spot,
                },
                risk: CoreRisk { position_gamma_pnl_for_1pct_move: 5.0 },
            },
        }
    }

    #[test]
    fn empty_portfolio_summarizes_to_all_zeros() {
        let summary = summarize(&[]);
        assert_eq!(summary, PortfolioSummary { position_count: 0, ..Default::default() });
    }

    #[test]
    fn single_position_dollar_delta_is_shares_times_spot() {
        let positions = vec![position("SPY", 100.0, -45.0, 1, 100.0)];
        let summary = summarize(&positions);
        assert_eq!(summary.position_count, 1);
        assert_eq!(summary.dollar_delta, -4500.0);
    }

    #[test]
    fn dollar_greeks_are_scaled_by_contracts_and_multiplier() {
        let positions = vec![position("SPY", 100.0, -45.0, 2, 100.0)];
        let summary = summarize(&positions);
        // scale = 2 * 100 = 200; vega_per_vol_point = 0.1 -> 20.0
        assert!((summary.dollar_vega_per_vol_point - 20.0).abs() < 1e-9);
        // theta_per_calendar_day = -0.02 -> -4.0
        assert!((summary.dollar_theta_per_day - (-4.0)).abs() < 1e-9);
        // rho_per_rate_point = -0.05 -> -10.0
        assert!((summary.dollar_rho_per_rate_point - (-10.0)).abs() < 1e-9);
    }

    #[test]
    fn dollar_delta_across_different_underlyings_is_meaningfully_additive() {
        // SPY short 45 delta-shares at $100, QQQ long 20 delta-shares at
        // $400: dollar delta is -4500 + 8000 = 3500, even though "shares"
        // themselves are not additive across the two underlyings.
        let positions = vec![position("SPY", 100.0, -45.0, 1, 100.0), position("QQQ", 400.0, 20.0, 1, 100.0)];
        let summary = summarize(&positions);
        assert_eq!(summary.position_count, 2);
        assert!((summary.dollar_delta - 3500.0).abs() < 1e-9);
    }

    #[test]
    fn gamma_pnl_and_notional_totals_sum_across_positions() {
        let positions = vec![position("SPY", 100.0, -45.0, 1, 100.0), position("QQQ", 400.0, 20.0, 1, 100.0)];
        let summary = summarize(&positions);
        assert_eq!(summary.dollar_gamma_pnl_for_1pct_move, 10.0); // 5.0 + 5.0
        assert_eq!(summary.total_stock_notional, 4500.0 + 8000.0);
        assert_eq!(summary.total_option_notional, 300.0 + 300.0); // 1*100*3.0 each
    }

    #[test]
    fn build_portfolio_rejects_an_unconfigured_symbol() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-portfolio-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"{"contracts": {"SPY": {"strike": 100.0, "implied_volatility": 0.2}}}"#).unwrap();
        let loaded =
            smart_hedge_config::load_config(Some(&config_path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();

        let result = build_portfolio(&loaded, &dir, &dir.join("nonexistent.cpp"), &["NOPE".to_string()]);
        assert!(matches!(result, Err(PortfolioError::UnknownSymbol(_))));
        std::fs::remove_dir_all(&dir).ok();
    }
}
