use smart_hedge_config::ContractConfig;
use smart_hedge_models::{CoreResponse, FeatureSet, MarketSnapshot, ModelAssessment};

use crate::advisor::Advisor;
use crate::error::AdvisorError;

fn feature_f64(features: &FeatureSet, key: &str) -> Option<f64> {
    features.values.get(key).and_then(serde_json::Value::as_f64)
}

/// Port of `HeuristicAdvisor`: a transparent, deterministic fallback that
/// classifies a regime and never predicts an order. Verifies:
/// SDH-LLR-055, SDH-HLR-100, SDH-HLR-130.
pub struct HeuristicAdvisor;

impl HeuristicAdvisor {
    pub const NAME: &'static str = "deterministic-regime-heuristic-v1";
}

impl Advisor for HeuristicAdvisor {
    fn assess(
        &self,
        snapshot: &MarketSnapshot,
        features: &FeatureSet,
        core: &CoreResponse,
        contract: &ContractConfig,
    ) -> Result<ModelAssessment, AdvisorError> {
        // Python's `values.get("ewma_volatility") or values.get("realized_volatility")`
        // falls through on a *falsy* ewma value (`None` or exactly `0.0`),
        // not just a missing one — replicated with the `filter` below.
        let ewma = feature_f64(features, "ewma_volatility").filter(|&v| v != 0.0);
        let realized = feature_f64(features, "realized_volatility");
        let rv = ewma.or(realized);

        let implied = contract.implied_volatility;
        let trend = feature_f64(features, "trend_score").unwrap_or(0.0);
        let spread = feature_f64(features, "spread_bps").unwrap_or(0.0);
        let event_risk = matches!(features.values.get("event_risk_flag"), Some(serde_json::Value::Bool(true)));

        let mut risks: Vec<String> = Vec::new();
        let (regime, band, mut urgency): (&str, f64, f64) = if spread > 35.0 {
            risks.push("The observed spread is wide; a frequent hedge would be cost-sensitive.".to_string());
            ("illiquid", 2.0, 0.25)
        } else if event_risk {
            risks.push("An event flag raises gap risk that stock delta hedging cannot remove.".to_string());
            ("jump_risk", 0.75, 0.90)
        } else if rv.is_some_and(|r| r > implied * 1.25) {
            risks.push(
                "Recent realized volatility is materially above the configured implied volatility."
                    .to_string(),
            );
            ("volatile", 0.70, 0.80)
        } else if trend > 1.25 {
            risks.push("Short-horizon return is large relative to estimated volatility.".to_string());
            ("trend_up", 0.85, 0.65)
        } else if trend < -1.25 {
            risks.push("Short-horizon decline is large relative to estimated volatility.".to_string());
            ("trend_down", 0.85, 0.65)
        } else if rv.is_some_and(|r| r < implied * 0.70) {
            ("calm", 1.35, 0.30)
        } else {
            risks.push("Available features do not identify a strong regime.".to_string());
            ("uncertain", 1.0, 0.50)
        };

        let gamma = core.greeks.gamma.abs();
        let gamma_scale = (gamma * snapshot.quote.midpoint() * 5.0).min(1.0);
        urgency = urgency.max(gamma_scale).min(1.0);
        let confidence = (features.data_quality * if rv.is_some() { 0.90 } else { 0.65 }).clamp(0.05, 0.95);

        Ok(ModelAssessment {
            advisor_kind: "heuristic".to_string(),
            model: Self::NAME.to_string(),
            regime: regime.to_string(),
            confidence,
            hedge_urgency: urgency,
            band_multiplier: band,
            summary: format!(
                "Transparent fallback classified the state as {regime}; it proposes only a bounded \
                 change to hedge urgency and the no-trade band."
            ),
            evidence_ids: features.evidence_ids.iter().take(8).cloned().collect(),
            risks,
            scenario_spot_shocks: vec![-0.10, -0.05, 0.05, 0.10],
            data_requests: if rv.is_some() {
                vec![]
            } else {
                vec!["More recent bars for realized volatility".to_string()]
            },
            raw_response_id: String::new(),
            fallback_reason: String::new(),
        })
    }

    fn name(&self) -> &'static str {
        "HeuristicAdvisor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_models::{Bar, CoreGreeks, CoreHedge, CoreInputs, CorePricing, CoreRisk, EvidenceItem, Quote};
    use std::collections::BTreeMap;

    fn base_contract() -> ContractConfig {
        ContractConfig {
            option_type: "put".to_string(),
            exercise_style: "american".to_string(),
            strike: smart_hedge_config::StrikeSpec::Fixed(100.0),
            days_to_expiry: 30.0,
            expiry: None,
            contracts: 1,
            multiplier: 100.0,
            current_shares: 0.0,
            rate: 0.045,
            dividend_yield: 0.012,
            implied_volatility: 0.20,
            base_no_trade_band_shares: 2.0,
        }
    }

    fn base_core(gamma: f64) -> CoreResponse {
        CoreResponse {
            engine_version: "test".to_string(),
            inputs: CoreInputs {
                spot: 100.0,
                strike: 100.0,
                rate: 0.045,
                dividend_yield: 0.012,
                volatility: 0.20,
                days_to_expiry: 30.0,
                option_type: "put".to_string(),
                exercise_style: "american".to_string(),
                contracts: 1,
                multiplier: 100.0,
                current_shares: 0.0,
                tree_steps: 600,
                base_no_trade_band_shares: 2.0,
            },
            pricing: CorePricing { model_price: 3.5, european_price: 3.4, early_exercise_premium: 0.1 },
            greeks: CoreGreeks {
                delta: -0.45,
                gamma,
                vega_per_vol_point: 0.15,
                theta_per_calendar_day: -0.01,
                rho_per_rate_point: -0.05,
            },
            hedge: CoreHedge {
                option_position_delta_shares: -45.0,
                target_stock_shares: 45.0,
                raw_trade_shares: 45.0,
                recommended_trade_shares: 45.0,
                action: "paper_rebalance_preview".to_string(),
                stock_notional: 4500.0,
            },
            risk: CoreRisk { position_gamma_pnl_for_1pct_move: 1.0 },
        }
    }

    fn base_snapshot() -> MarketSnapshot {
        MarketSnapshot::new(
            "TEST",
            Quote::new("TEST", 99.99, 100.01, 100.0, "2026-07-19T00:00:00Z", "test", "open"),
            vec![Bar {
                timestamp: "2026-07-19T00:00:00Z".to_string(),
                open: 100.0,
                high: 100.0,
                low: 100.0,
                close: 100.0,
                volume: 1000.0,
            }],
            Vec::<EvidenceItem>::new(),
        )
    }

    fn features_with(values: BTreeMap<String, serde_json::Value>, data_quality: f64) -> FeatureSet {
        FeatureSet { values, missing: vec![], warnings: vec![], data_quality, evidence_ids: vec![] }
    }

    #[test]
    fn wide_spread_produces_illiquid_regime() {
        let mut values = BTreeMap::new();
        values.insert("spread_bps".to_string(), serde_json::json!(50.0));
        let features = features_with(values, 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.regime, "illiquid");
        assert_eq!(result.band_multiplier, 2.0);
    }

    #[test]
    fn event_risk_flag_produces_jump_risk_regime() {
        let mut values = BTreeMap::new();
        values.insert("event_risk_flag".to_string(), serde_json::json!(true));
        let features = features_with(values, 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.regime, "jump_risk");
    }

    #[test]
    fn high_realized_volatility_produces_volatile_regime() {
        let mut values = BTreeMap::new();
        values.insert("realized_volatility".to_string(), serde_json::json!(0.30)); // > 0.20 * 1.25
        let features = features_with(values, 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.regime, "volatile");
    }

    #[test]
    fn low_realized_volatility_produces_calm_regime() {
        let mut values = BTreeMap::new();
        values.insert("realized_volatility".to_string(), serde_json::json!(0.10)); // < 0.20 * 0.70
        let features = features_with(values, 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.regime, "calm");
    }

    #[test]
    fn no_strong_signal_produces_uncertain_regime_with_a_risk_note() {
        let features = features_with(BTreeMap::new(), 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.regime, "uncertain");
        assert!(!result.risks.is_empty());
    }

    #[test]
    fn missing_volatility_requests_more_bars() {
        let features = features_with(BTreeMap::new(), 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.data_requests, vec!["More recent bars for realized volatility".to_string()]);
    }

    #[test]
    fn zero_ewma_volatility_falls_back_to_realized_volatility() {
        // Python's `values.get("ewma_volatility") or values.get("realized_volatility")`
        // treats an exact 0.0 ewma value as falsy and falls through.
        let mut values = BTreeMap::new();
        values.insert("ewma_volatility".to_string(), serde_json::json!(0.0));
        values.insert("realized_volatility".to_string(), serde_json::json!(0.30));
        let features = features_with(values, 1.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        // 0.30 > 0.20 * 1.25 = 0.25, so this only classifies as "volatile"
        // if the fallback to realized_volatility actually happened.
        assert_eq!(result.regime, "volatile");
    }

    #[test]
    fn high_gamma_raises_urgency_regardless_of_regime() {
        let features = features_with(BTreeMap::new(), 1.0);
        let low_gamma =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        let high_gamma =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.5), &base_contract()).unwrap();
        assert!(high_gamma.hedge_urgency >= low_gamma.hedge_urgency);
        assert!(high_gamma.hedge_urgency <= 1.0);
    }

    #[test]
    fn confidence_is_bounded_to_0_05_and_0_95() {
        let features = features_with(BTreeMap::new(), 0.0);
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert!(result.confidence >= 0.05 && result.confidence <= 0.95);
    }

    #[test]
    fn evidence_ids_are_capped_at_eight() {
        let mut features = features_with(BTreeMap::new(), 1.0);
        features.evidence_ids = (0..20).map(|i| format!("e{i}")).collect();
        let result =
            HeuristicAdvisor.assess(&base_snapshot(), &features, &base_core(0.0), &base_contract()).unwrap();
        assert_eq!(result.evidence_ids.len(), 8);
    }
}
