use smart_hedge_config::{Config, ContractConfig, StrikeSpec};
use smart_hedge_models::TimestampUtc;

use crate::error::EngineError;

/// Explicit per-call contract overrides — mirrors the five fields
/// `cli.py`'s `_overrides` actually constructs from CLI flags (all typed
/// `float`/`int` by `argparse`, so `strike` here is never the `"ATM"`
/// literal; only a *configured* contract's `strike` can be `"ATM"`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ContractOverrides {
    pub strike: Option<f64>,
    pub implied_volatility: Option<f64>,
    pub days_to_expiry: Option<f64>,
    pub current_shares: Option<f64>,
    pub contracts: Option<i64>,
}

fn days_to_expiry_from_date(expiry_str: &str, now: TimestampUtc) -> Result<f64, EngineError> {
    let close_str = format!("{expiry_str}T21:00:00Z");
    let close = TimestampUtc::parse_flexible(&close_str)
        .ok_or_else(|| EngineError::InvalidExpiryDate(expiry_str.to_string()))?;
    Ok((now.seconds_until(&close) / 86_400.0).max(0.0))
}

/// Port of `engine.SmartHedgeEngine.contract_for` plus the ATM-strike
/// resolution and strike validation from the start of `recommendation`
/// (kept together here since Python interleaves them and the ordering —
/// override, then expiry-vs-days_to_expiry precedence, then validation —
/// matters). `midpoint` and `now` are explicit parameters rather than
/// read from a live quote/clock internally, so ATM and expiry-date
/// resolution (SDH-LLR-131, SDH-LLR-132) are directly testable.
///
/// On success, the returned `ContractConfig.strike` is always
/// `StrikeSpec::Fixed` — ATM has been resolved and `expiry` has been
/// cleared (`contract.pop("expiry", None)` in Python), matching what
/// `core_bridge::run_core` and the output `Recommendation.contract` blob
/// both expect.
pub fn resolve_contract(
    config: &Config,
    symbol: &str,
    overrides: &ContractOverrides,
    midpoint: f64,
    now: TimestampUtc,
) -> Result<ContractConfig, EngineError> {
    let mut contract =
        config.contracts.get(symbol).cloned().ok_or_else(|| EngineError::UnknownSymbol(symbol.to_string()))?;

    if let Some(v) = overrides.strike {
        contract.strike = StrikeSpec::Fixed(v);
    }
    if let Some(v) = overrides.implied_volatility {
        contract.implied_volatility = v;
    }
    if let Some(v) = overrides.days_to_expiry {
        contract.days_to_expiry = v;
    }
    if let Some(v) = overrides.current_shares {
        contract.current_shares = v;
    }
    if let Some(v) = overrides.contracts {
        contract.contracts = v;
    }

    // Matches Python's `_days_to_expiry`, called unconditionally after
    // overrides are applied: a configured `expiry` date always wins over
    // whatever `days_to_expiry` ended up as — including over an explicit
    // per-call `--days` override, which Python silently discards in that
    // case. This is exactly the surprising-but-real precedence
    // SDH-LLR-132 documents.
    if let Some(expiry_str) = contract.expiry.clone() {
        contract.days_to_expiry = days_to_expiry_from_date(&expiry_str, now)?;
    }
    contract.days_to_expiry = contract.days_to_expiry.max(0.0);
    contract.expiry = None;

    if !matches!(contract.option_type.as_str(), "call" | "put") {
        return Err(EngineError::InvalidOptionType(contract.option_type.clone()));
    }
    if !matches!(contract.exercise_style.as_str(), "american" | "european") {
        return Err(EngineError::InvalidExerciseStyle(contract.exercise_style.clone()));
    }

    let resolved_strike = match contract.strike {
        StrikeSpec::Atm => midpoint.round(),
        StrikeSpec::Fixed(v) => v,
    };
    if !resolved_strike.is_finite() || resolved_strike <= 0.0 {
        return Err(EngineError::InvalidStrike(resolved_strike.to_string()));
    }
    contract.strike = StrikeSpec::Fixed(resolved_strike);

    Ok(contract)
}

/// Extracts the resolved numeric strike from a contract that
/// `resolve_contract` has already processed (its `strike` is always
/// `Fixed` afterward). Panics if called on an unresolved contract — a
/// programming error, not a runtime condition to handle gracefully.
pub fn resolved_strike(contract: &ContractConfig) -> f64 {
    match contract.strike {
        StrikeSpec::Fixed(v) => v,
        StrikeSpec::Atm => unreachable!("resolve_contract always resolves ATM before returning"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn config_with_contracts(contracts_json: &str) -> Config {
        // Tests run in parallel threads within this one process, so the
        // directory name must be unique per call, not just per process —
        // sharing one path let concurrent tests overwrite each other's
        // config.json and race on remove_dir_all.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-engine-contract-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"contracts": {contracts_json}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded.config
    }

    /// SDH-LLR-130: an invalid option_type is rejected.
    #[test]
    fn invalid_option_type_is_rejected() {
        let config = config_with_contracts(
            r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2, "option_type": "straddle"}}"#,
        );
        let result = resolve_contract(&config, "SPY", &ContractOverrides::default(), 100.0, TimestampUtc::now());
        assert!(matches!(result, Err(EngineError::InvalidOptionType(_))));
    }

    /// SDH-LLR-130: an invalid exercise_style is rejected.
    #[test]
    fn invalid_exercise_style_is_rejected() {
        let config = config_with_contracts(
            r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2, "exercise_style": "bermudan"}}"#,
        );
        let result = resolve_contract(&config, "SPY", &ContractOverrides::default(), 100.0, TimestampUtc::now());
        assert!(matches!(result, Err(EngineError::InvalidExerciseStyle(_))));
    }

    /// SDH-LLR-131: an "ATM" strike resolves to the rounded midpoint.
    #[test]
    fn atm_strike_resolves_to_rounded_midpoint() {
        let config = config_with_contracts(r#"{"SPY": {"strike": "ATM", "implied_volatility": 0.2}}"#);
        let contract =
            resolve_contract(&config, "SPY", &ContractOverrides::default(), 123.6, TimestampUtc::now()).unwrap();
        assert_eq!(resolved_strike(&contract), 124.0);
    }

    #[test]
    fn unknown_symbol_is_rejected() {
        let config = config_with_contracts(r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2}}"#);
        let result = resolve_contract(&config, "NOPE", &ContractOverrides::default(), 100.0, TimestampUtc::now());
        assert!(matches!(result, Err(EngineError::UnknownSymbol(_))));
    }

    /// SDH-LLR-132: an `expiry` date overrides the static `days_to_expiry`.
    #[test]
    fn expiry_date_overrides_static_days_to_expiry() {
        let config = config_with_contracts(
            r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2, "days_to_expiry": 999.0, "expiry": "2026-07-20"}}"#,
        );
        let now = TimestampUtc::parse_flexible("2026-07-19T00:00:00Z").unwrap();
        let contract = resolve_contract(&config, "SPY", &ContractOverrides::default(), 100.0, now).unwrap();
        // 2026-07-20T21:00:00Z minus 2026-07-19T00:00:00Z = 1 day + 21 hours = 1.875 days.
        assert!((contract.days_to_expiry - 1.875).abs() < 1e-9);
        assert!(contract.expiry.is_none()); // popped, matching Python
    }

    /// SDH-LLR-132: `expiry` wins even over an explicit per-call override.
    #[test]
    fn expiry_date_overrides_even_an_explicit_days_to_expiry_override() {
        let config = config_with_contracts(
            r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2, "expiry": "2026-07-20"}}"#,
        );
        let now = TimestampUtc::parse_flexible("2026-07-19T00:00:00Z").unwrap();
        let overrides = ContractOverrides { days_to_expiry: Some(365.0), ..Default::default() };
        let contract = resolve_contract(&config, "SPY", &overrides, 100.0, now).unwrap();
        assert!((contract.days_to_expiry - 1.875).abs() < 1e-9, "expiry should have won, got {}", contract.days_to_expiry);
    }

    #[test]
    fn days_to_expiry_from_date_is_floored_at_zero_for_a_past_date() {
        let config =
            config_with_contracts(r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2, "expiry": "2020-01-01"}}"#);
        let now = TimestampUtc::parse_flexible("2026-07-19T00:00:00Z").unwrap();
        let contract = resolve_contract(&config, "SPY", &ContractOverrides::default(), 100.0, now).unwrap();
        assert_eq!(contract.days_to_expiry, 0.0);
    }

    #[test]
    fn overrides_apply_before_validation() {
        let config = config_with_contracts(r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2}}"#);
        let overrides = ContractOverrides { strike: Some(150.0), current_shares: Some(-5.0), ..Default::default() };
        let contract =
            resolve_contract(&config, "SPY", &overrides, 100.0, TimestampUtc::now()).unwrap();
        assert_eq!(resolved_strike(&contract), 150.0);
        assert_eq!(contract.current_shares, -5.0);
    }

    #[test]
    fn nonpositive_resolved_strike_is_rejected() {
        let config = config_with_contracts(r#"{"SPY": {"strike": -5.0, "implied_volatility": 0.2}}"#);
        let result = resolve_contract(&config, "SPY", &ContractOverrides::default(), 100.0, TimestampUtc::now());
        assert!(matches!(result, Err(EngineError::InvalidStrike(_))));
    }
}
