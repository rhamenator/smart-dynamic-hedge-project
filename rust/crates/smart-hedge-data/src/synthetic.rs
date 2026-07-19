use serde_json::Value;
use smart_hedge_config::LoadedConfig;
use smart_hedge_models::{Bar, EvidenceItem, MarketSnapshot, Quote, TimestampUtc};

use crate::error::DataError;
use crate::evidence_file::load_evidence_file;
use crate::provider::MarketDataProvider;
use crate::rng::Rng;

const BAR_COUNT: usize = 180;
const BARS_PER_YEAR: f64 = 252.0 * 390.0;

fn population_stdev(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

/// Same seed-derivation *formula* as Python (`sum((i+1) * ord(ch) for ...)
/// + bucket`) — preserved exactly since it's cheap to match and is what
/// makes the "same symbol+bucket -> same seed" property (SDH-LLR-122)
/// meaningful, even though the PRNG consuming this seed is not a port of
/// Python's Mersenne Twister (see `rng` module doc comment).
fn derive_seed(symbol: &str, bucket: i64) -> u64 {
    let char_sum: i64 =
        symbol.to_uppercase().chars().enumerate().map(|(i, ch)| (i as i64 + 1) * ch as i64).sum();
    (char_sum + bucket).max(0) as u64
}

/// Port of `SyntheticProvider`. Verifies: SDH-LLR-120, SDH-LLR-121,
/// SDH-LLR-122.
///
/// Owns its `LoadedConfig` (rather than borrowing it with a lifetime) so
/// it can be used as a plain `Box<dyn MarketDataProvider>` — matching
/// Python's duck-typed `Protocol`, where a provider instance is
/// constructed once and captures whatever it needs from `self.config`.
pub struct SyntheticProvider {
    config: LoadedConfig,
}

impl SyntheticProvider {
    pub fn new(config: LoadedConfig) -> Self {
        SyntheticProvider { config }
    }

    /// The testable core: takes `now` explicitly rather than reading the
    /// system clock internally, so determinism (SDH-LLR-122) can actually
    /// be tested — two calls with timestamps in the same 5-second bucket
    /// must be identical; different buckets must differ.
    pub fn snapshot_at(&self, symbol: &str, now: TimestampUtc) -> MarketSnapshot {
        let symbol_upper = symbol.to_uppercase();
        let bucket = now.unix_seconds().div_euclid(5);
        let seed = derive_seed(&symbol_upper, bucket);
        let mut rng = Rng::new(seed);

        let base = self
            .config
            .config
            .contracts
            .get(&symbol_upper)
            .and_then(|c| match c.strike {
                // Python's own synthetic provider would raise trying
                // `float("ATM")` here — an "ATM" strike has no meaning as
                // a price anchor without a live quote to resolve it
                // against, which this function is busy generating. Fall
                // back to the same 100.0 default used when no contract is
                // configured at all, rather than propagating Python's bug.
                smart_hedge_config::StrikeSpec::Fixed(v) => Some(v),
                smart_hedge_config::StrikeSpec::Atm => None,
            })
            .unwrap_or(100.0);
        let anchor = base * (1.0 + 0.03 * (bucket as f64 / 240.0).sin());
        let sigma_per_bar = 0.20 / BARS_PER_YEAR.sqrt();

        let mut closes = vec![anchor];
        for _ in 0..(BAR_COUNT - 1) {
            let mut jump = rng.gauss(0.0, sigma_per_bar);
            if rng.random() < 0.01 {
                jump += rng.sign() * rng.uniform(0.002, 0.008);
            }
            let previous = *closes.last().unwrap();
            closes.push((previous * jump.exp()).max(0.01));
        }

        let now_iso = now.to_iso_string();
        let start_secs = now.unix_seconds() - (closes.len() as i64 - 1) * 60;
        let mut bars = Vec::with_capacity(closes.len());
        for (i, &close) in closes.iter().enumerate() {
            let previous = if i == 0 { close } else { closes[i - 1] };
            let high = previous.max(close) * (1.0 + rng.uniform(0.0, 0.0007));
            let low = previous.min(close) * (1.0 - rng.uniform(0.0, 0.0007));
            let bar_time = TimestampUtc::from_unix(start_secs + i as i64 * 60, 0);
            bars.push(Bar {
                timestamp: bar_time.to_iso_string(),
                open: previous,
                high,
                low,
                close,
                volume: rng.lognormvariate(9.0, 0.55).max(100.0),
            });
        }

        let last = *closes.last().unwrap();
        let spread_bps = rng.uniform(0.5, 4.0);
        let half_spread = last * spread_bps / 20_000.0;
        let quote = Quote::new(&symbol_upper, last - half_spread, last + half_spread, last, now_iso.clone(), "synthetic", "open");

        let log_returns: Vec<f64> = (1..closes.len()).map(|i| (closes[i] / closes[i - 1]).ln()).collect();
        let realized = population_stdev(&log_returns) * BARS_PER_YEAR.sqrt();

        let mut evidence = vec![
            EvidenceItem {
                evidence_id: format!("synthetic-rv-{bucket}"),
                kind: "option_metric".to_string(),
                title: "Synthetic realized volatility".to_string(),
                timestamp: now_iso.clone(),
                source: "synthetic".to_string(),
                value: Value::from(realized),
                text: "Generated solely for exercising the pipeline.".to_string(),
                quality: 1.0,
                untrusted_text: false,
            },
            EvidenceItem {
                evidence_id: format!("synthetic-event-{bucket}"),
                kind: "event".to_string(),
                title: "Synthetic event-risk flag".to_string(),
                timestamp: now_iso,
                source: "synthetic".to_string(),
                value: Value::Bool(bucket % 29 == 0),
                text: "No real-world event is represented.".to_string(),
                quality: 1.0,
                untrusted_text: false,
            },
        ];
        evidence.extend(load_evidence_file(&self.config, &symbol_upper));

        MarketSnapshot::new(symbol_upper, quote, bars, evidence)
    }
}

impl MarketDataProvider for SyntheticProvider {
    fn snapshot(&self, symbol: &str) -> Result<MarketSnapshot, DataError> {
        Ok(self.snapshot_at(symbol, TimestampUtc::now()))
    }

    fn name(&self) -> &'static str {
        "SyntheticProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn config() -> LoadedConfig {
        smart_hedge_config::load_config(None, &EnvOverrides::default(), std::path::Path::new("/root")).unwrap()
    }

    #[test]
    fn produces_the_expected_bar_count() {
        let cfg = config();
        let provider = SyntheticProvider::new(cfg);
        let snapshot = provider.snapshot_at("SPY", TimestampUtc::now());
        assert_eq!(snapshot.bars.len(), 180);
    }

    /// SDH-LLR-121: market state is always "open".
    #[test]
    fn market_state_is_always_open() {
        let cfg = config();
        let provider = SyntheticProvider::new(cfg);
        let snapshot = provider.snapshot_at("SPY", TimestampUtc::now());
        assert_eq!(snapshot.quote.market_state, "open");
    }

    /// SDH-LLR-122: identical (symbol, bucket) -> identical snapshot.
    #[test]
    fn same_symbol_and_bucket_produces_identical_snapshots() {
        let cfg = config();
        let provider = SyntheticProvider::new(cfg);
        let t1 = TimestampUtc::parse_flexible("2026-07-19T14:30:01Z").unwrap();
        let t2 = TimestampUtc::parse_flexible("2026-07-19T14:30:04Z").unwrap(); // same 5s bucket
        let a = provider.snapshot_at("SPY", t1);
        let b = provider.snapshot_at("SPY", t2);
        assert_eq!(a.quote.last, b.quote.last);
        assert_eq!(a.bars.len(), b.bars.len());
        assert_eq!(a.bars[0].close, b.bars[0].close);
    }

    /// SDH-LLR-122: a different bucket produces a different snapshot.
    #[test]
    fn different_bucket_produces_a_different_snapshot() {
        let cfg = config();
        let provider = SyntheticProvider::new(cfg);
        let t1 = TimestampUtc::parse_flexible("2026-07-19T14:30:00Z").unwrap();
        let t2 = TimestampUtc::parse_flexible("2026-07-19T14:30:05Z").unwrap(); // next 5s bucket
        let a = provider.snapshot_at("SPY", t1);
        let b = provider.snapshot_at("SPY", t2);
        assert_ne!(a.quote.last, b.quote.last);
    }

    /// SDH-LLR-120: no network/account is required — this test's mere
    /// existence and success demonstrates that (no HTTP client is even
    /// linked into this crate for the synthetic path).
    #[test]
    fn all_closes_and_the_quote_are_positive_and_finite() {
        let cfg = config();
        let provider = SyntheticProvider::new(cfg);
        let snapshot = provider.snapshot_at("SPY", TimestampUtc::now());
        for bar in &snapshot.bars {
            assert!(bar.close > 0.0 && bar.close.is_finite());
            assert!(bar.high >= bar.low);
        }
        assert!(snapshot.quote.midpoint() > 0.0);
    }

    #[test]
    fn includes_the_two_synthetic_evidence_items_plus_any_file_based_ones() {
        let cfg = config();
        let provider = SyntheticProvider::new(cfg);
        let snapshot = provider.snapshot_at("SPY", TimestampUtc::now());
        assert!(snapshot.evidence.len() >= 2);
        assert!(snapshot.evidence.iter().any(|e| e.kind == "option_metric"));
        assert!(snapshot.evidence.iter().any(|e| e.kind == "event"));
    }

    #[test]
    fn configured_contract_strike_anchors_the_price_path() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-data-synth-anchor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"{"contracts": {"ZZZ": {"strike": 500.0, "days_to_expiry": 30.0, "implied_volatility": 0.2}}}"#).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&config_path), &EnvOverrides::default(), &dir).unwrap();
        let provider = SyntheticProvider::new(loaded);
        let snapshot = provider.snapshot_at("ZZZ", TimestampUtc::parse_flexible("2026-07-19T00:00:00Z").unwrap());
        // Anchor is 500 * (1 +/- up to 3%), so the first close should be
        // in a plausible neighborhood of 500, not near the 100.0 default.
        assert!(snapshot.bars[0].close > 400.0 && snapshot.bars[0].close < 600.0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
