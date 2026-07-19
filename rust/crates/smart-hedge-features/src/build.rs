use std::collections::BTreeMap;

use serde_json::Value;
use smart_hedge_config::FeaturesConfig;
use smart_hedge_models::{FeatureSet, MarketSnapshot};

use crate::evidence_summary::summarize;
use crate::returns::{drawdown_from_rolling_peak, horizon_return, log_returns};
use crate::stats::{ewma_variance, mean, sample_stdev};

fn opt(value: Option<f64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

/// Port of `smart_hedge.features.build_features`.
///
/// Verifies: SDH-LLR-110 (data-quality composition), SDH-LLR-111
/// (missing features marked rather than defaulted), SDH-LLR-112 (volume
/// z-score history requirement), SDH-LLR-113 (trend-score volatility
/// floor) — see `requirements/LLR.md`.
pub fn build_features(snapshot: &MarketSnapshot, config: &FeaturesConfig) -> FeatureSet {
    let bars_per_year = config.bars_per_year;
    let decay = config.ewma_lambda;
    let short_window = config.short_window.max(0) as usize;
    let long_window = config.long_window.max(0) as usize;

    let closes: Vec<f64> = snapshot.bars.iter().map(|b| b.close).filter(|&c| c > 0.0).collect();
    let volumes: Vec<f64> = snapshot.bars.iter().map(|b| b.volume.max(0.0)).collect();
    let returns = log_returns(&closes);

    let mut missing: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let realized = if returns.len() >= 2 {
        sample_stdev(&returns).map(|sd| sd * bars_per_year.sqrt())
    } else {
        missing.push("realized_volatility".to_string());
        None
    };

    let ewma_vol = ewma_variance(&returns, decay).map(|v| (v * bars_per_year).sqrt());
    if ewma_vol.is_none() {
        missing.push("ewma_volatility".to_string());
    }

    let short_return = horizon_return(&closes, short_window);
    let long_return = horizon_return(&closes, long_window);
    if short_return.is_none() {
        missing.push(format!("return_{short_window}_bars"));
    }
    if long_return.is_none() {
        missing.push(format!("return_{long_window}_bars"));
    }

    let drawdown = drawdown_from_rolling_peak(&closes, long_window);

    let volume_z = if volumes.len() >= 21 {
        let history = &volumes[volumes.len() - 21..volumes.len() - 1];
        match sample_stdev(history) {
            Some(sd) if sd > 0.0 => mean(history).map(|m| (volumes[volumes.len() - 1] - m) / sd),
            _ => None,
        }
    } else {
        None
    };
    if volume_z.is_none() {
        warnings.push("volume_zscore_unavailable".to_string());
    }

    let trend_score = match (short_return, realized) {
        (Some(sr), Some(r)) if r > 1e-9 => {
            let horizon_years = short_window as f64 / bars_per_year;
            Some(sr / (r * horizon_years.sqrt()))
        }
        _ => None,
    };

    let (evidence_numeric, event_risk) = summarize(&snapshot.evidence);
    let evidence_quality: Vec<f64> = snapshot.evidence.iter().map(|e| e.quality).collect();

    let mut values: BTreeMap<String, Value> = BTreeMap::new();
    values.insert("spot".to_string(), Value::from(snapshot.quote.midpoint()));
    values.insert("bid".to_string(), Value::from(snapshot.quote.bid));
    values.insert("ask".to_string(), Value::from(snapshot.quote.ask));
    values.insert("spread_bps".to_string(), Value::from(snapshot.quote.spread_bps()));
    values.insert("market_state".to_string(), Value::String(snapshot.quote.market_state.clone()));
    values.insert("bar_count".to_string(), Value::from(snapshot.bars.len() as f64));
    values.insert("realized_volatility".to_string(), opt(realized));
    values.insert("ewma_volatility".to_string(), opt(ewma_vol));
    values.insert(format!("return_{short_window}_bars"), opt(short_return));
    values.insert(format!("return_{long_window}_bars"), opt(long_return));
    values.insert("drawdown_from_rolling_peak".to_string(), opt(drawdown));
    values.insert("volume_zscore".to_string(), opt(volume_z));
    values.insert("trend_score".to_string(), opt(trend_score));
    values.insert("event_risk_flag".to_string(), Value::Bool(event_risk));
    for (key, value) in evidence_numeric {
        values.insert(key, Value::from(value));
    }

    let mut quality_components = vec![
        if snapshot.quote.midpoint() > 0.0 { 1.0 } else { 0.0 },
        if snapshot.quote.spread_bps().is_finite() { 1.0 } else { 0.0 },
        (snapshot.bars.len() as f64 / (long_window as f64 + 1.0).max(1.0)).min(1.0),
        1.0 - (missing.len() as f64 / 6.0).min(1.0),
    ];
    if !evidence_quality.is_empty() {
        quality_components.push(mean(&evidence_quality).unwrap_or(0.0));
    }
    let data_quality = mean(&quality_components).unwrap_or(0.0).clamp(0.0, 1.0);

    FeatureSet {
        values,
        missing,
        warnings,
        data_quality,
        evidence_ids: snapshot.evidence.iter().map(|e| e.evidence_id.clone()).collect(),
    }
}
