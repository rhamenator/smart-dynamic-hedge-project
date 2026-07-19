use std::time::Duration;

use serde_json::Value;
use smart_hedge_config::LoadedConfig;
use smart_hedge_models::{Bar, MarketSnapshot, Quote, TimestampUtc};

use crate::error::DataError;
use crate::evidence_file::load_evidence_file;
use crate::fred::load_fred_evidence_from_env;
use crate::market_hours::regular_market_state;
use crate::provider::MarketDataProvider;
use crate::rss::load_rss_evidence;

const USER_AGENT: &str = "smart-dynamic-hedge/0.2 read-only";

/// Port of `data.AlpacaReadOnlyProvider`: a read-only U.S. equity
/// quote/bar adapter. Contains no order URL or method — only GET requests
/// against the market-data host. Verifies: SDH-LLR-081.
pub struct AlpacaReadOnlyProvider {
    loaded: LoadedConfig,
    api_key: String,
    api_secret: String,
    base: String,
    feed: String,
    timeout: Duration,
}

impl AlpacaReadOnlyProvider {
    /// Constructs the provider from explicit credentials rather than
    /// reading environment variables directly inside constructible,
    /// testable code — Rust 2024 makes `std::env::set_var` `unsafe`, and
    /// this workspace forbids `unsafe_code` outright, so a test can't
    /// inject an env var to exercise this path; taking credentials as
    /// parameters sidesteps that entirely (same pattern as
    /// `smart_hedge_config::EnvOverrides`). `from_env` below is the thin
    /// wrapper that actually reads the process environment.
    pub fn new(loaded: LoadedConfig, api_key: String, api_secret: String) -> Result<Self, DataError> {
        if api_key.is_empty() || api_secret.is_empty() {
            return Err(DataError::MissingEnvVar("ALPACA_API_KEY_ID and ALPACA_API_SECRET_KEY"));
        }
        let alpaca = &loaded.config.provider.alpaca;
        let base = alpaca.data_base_url.trim_end_matches('/').to_string();
        let feed = alpaca.feed.clone();
        let timeout = Duration::from_secs_f64(alpaca.timeout_seconds.max(0.0));
        Ok(AlpacaReadOnlyProvider { loaded, api_key, api_secret, base, feed, timeout })
    }

    pub fn from_env(loaded: LoadedConfig) -> Result<Self, DataError> {
        let api_key = std::env::var("ALPACA_API_KEY_ID").unwrap_or_default();
        let api_secret = std::env::var("ALPACA_API_SECRET_KEY").unwrap_or_default();
        Self::new(loaded, api_key, api_secret)
    }

    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, DataError> {
        let url = format!("{}{}", self.base, path);
        let mut request = ureq::get(&url)
            .set("APCA-API-KEY-ID", &self.api_key)
            .set("APCA-API-SECRET-KEY", &self.api_secret)
            .set("Accept", "application/json")
            .set("User-Agent", USER_AGENT)
            .timeout(self.timeout);
        for (key, value) in query {
            request = request.query(key, value);
        }
        let response = request.call().map_err(|e| DataError::Http(e.to_string()))?;
        let text = response.into_string().map_err(|e| DataError::Http(e.to_string()))?;
        let decoded: Value = serde_json::from_str(&text).map_err(|e| DataError::InvalidJson(e.to_string()))?;
        if !decoded.is_object() {
            return Err(DataError::UnexpectedResponse("expected a JSON object".to_string()));
        }
        Ok(decoded)
    }
}

/// Python's `value.replace("+00:00", "Z")` — a plain string transform, no
/// fallback logic (the fallback-to-"now"/fallback-to-another-string
/// decisions happen at each call site, matching how Python's `_iso` is
/// composed with `or` differently for bars vs. the quote).
fn iso(value: &str) -> String {
    value.replace("+00:00", "Z")
}

/// Port of the bar-list construction inside `AlpacaReadOnlyProvider.snapshot`:
/// takes the already-reversed-to-chronological `bars` array from the API
/// response and keeps only items with all four required OHLC keys, erroring
/// (not silently zero-filling) if a present key isn't numeric — Python's
/// `float(item["o"])` would raise the same way on non-numeric data.
fn parse_bars(raw_bars_chronological: &[Value]) -> Result<Vec<Bar>, DataError> {
    let mut bars = Vec::new();
    for item in raw_bars_chronological {
        let Some(map) = item.as_object() else { continue };
        if !["o", "h", "l", "c"].iter().all(|k| map.contains_key(*k)) {
            continue;
        }
        let field = |key: &str| -> Result<f64, DataError> {
            map[key].as_f64().ok_or_else(|| DataError::UnexpectedResponse(format!("bar field {key} was not numeric")))
        };
        let timestamp = match map.get("t").and_then(Value::as_str) {
            Some(s) if !s.is_empty() => iso(s),
            _ => TimestampUtc::now().to_iso_string(),
        };
        bars.push(Bar {
            timestamp,
            open: field("o")?,
            high: field("h")?,
            low: field("l")?,
            close: field("c")?,
            volume: map.get("v").and_then(Value::as_f64).unwrap_or(0.0),
        });
    }
    Ok(bars)
}

/// Port of the `Quote` construction inside `AlpacaReadOnlyProvider.snapshot`.
/// `quote_json` is the `"quote"` sub-object of the quotes-latest response
/// (may be absent/null, matching Python's `quote_payload.get("quote") or {}`).
fn build_quote(symbol: &str, quote_json: &Value, last_bar: &Bar, feed: &str, market_state: &str) -> Quote {
    // `float(q.get("bp") or bars[-1].close)`: falls back on missing *or*
    // exactly-zero, matching Python's `or` truthiness (not just absence).
    let bid = quote_json.get("bp").and_then(Value::as_f64).filter(|v| *v != 0.0).unwrap_or(last_bar.close);
    let ask = quote_json.get("ap").and_then(Value::as_f64).filter(|v| *v != 0.0).unwrap_or(last_bar.close);
    let raw_t = quote_json.get("t").and_then(Value::as_str).filter(|s| !s.is_empty());
    let timestamp = iso(raw_t.unwrap_or(last_bar.timestamp.as_str()));
    Quote::new(symbol, bid, ask, last_bar.close, timestamp, format!("alpaca:{feed}"), market_state)
}

impl MarketDataProvider for AlpacaReadOnlyProvider {
    fn snapshot(&self, symbol: &str) -> Result<MarketSnapshot, DataError> {
        let normalized = symbol.to_uppercase();
        let alpaca = &self.loaded.config.provider.alpaca;
        let bar_limit = alpaca.bar_limit;
        let bar_timeframe = alpaca.bar_timeframe.clone();

        let quote_payload =
            self.get(&format!("/v2/stocks/{normalized}/quotes/latest"), &[("feed", self.feed.clone())])?;

        let seven_days_ago = TimestampUtc::from_unix(TimestampUtc::now().unix_seconds() - 7 * 86_400, 0);
        let bars_payload = self.get(
            &format!("/v2/stocks/{normalized}/bars"),
            &[
                ("feed", self.feed.clone()),
                ("timeframe", bar_timeframe),
                ("limit", bar_limit.to_string()),
                ("adjustment", "all".to_string()),
                ("start", seven_days_ago.to_iso_string()),
                ("sort", "desc".to_string()),
            ],
        )?;

        let quote_json = quote_payload.get("quote").cloned().unwrap_or(Value::Null);
        let mut raw_bars: Vec<Value> = bars_payload.get("bars").and_then(Value::as_array).cloned().unwrap_or_default();
        raw_bars.reverse(); // API returns descending (most recent first); we need chronological.

        let bars = parse_bars(&raw_bars)?;
        if bars.is_empty() {
            return Err(DataError::UnexpectedResponse("market-data provider returned no bars".to_string()));
        }

        let last_bar = bars.last().expect("just checked non-empty");
        let market_state = regular_market_state(TimestampUtc::now());
        let quote = build_quote(&normalized, &quote_json, last_bar, &self.feed, market_state);

        let mut evidence = load_evidence_file(&self.loaded, &normalized);
        evidence.extend(load_fred_evidence_from_env(&self.loaded));
        evidence.extend(load_rss_evidence(&self.loaded, &normalized));

        Ok(MarketSnapshot::new(normalized, quote, bars, evidence))
    }

    fn name(&self) -> &'static str {
        "AlpacaReadOnlyProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use smart_hedge_config::EnvOverrides;

    fn loaded_config() -> LoadedConfig {
        smart_hedge_config::load_config(None, &EnvOverrides::default(), std::path::Path::new("/root")).unwrap()
    }

    #[test]
    fn missing_api_key_is_rejected() {
        let result = AlpacaReadOnlyProvider::new(loaded_config(), "".to_string(), "secret".to_string());
        assert!(matches!(result, Err(DataError::MissingEnvVar(_))));
    }

    #[test]
    fn missing_api_secret_is_rejected() {
        let result = AlpacaReadOnlyProvider::new(loaded_config(), "key".to_string(), "".to_string());
        assert!(matches!(result, Err(DataError::MissingEnvVar(_))));
    }

    #[test]
    fn valid_credentials_construct_successfully() {
        let result = AlpacaReadOnlyProvider::new(loaded_config(), "key".to_string(), "secret".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn parse_bars_keeps_only_items_with_all_four_ohlc_keys() {
        let raw = vec![
            json!({"o": 1.0, "h": 2.0, "l": 0.5, "c": 1.5, "v": 100.0, "t": "2026-07-19T00:00:00Z"}),
            json!({"o": 1.0, "h": 2.0}), // missing l/c -> skipped
        ];
        let bars = parse_bars(&raw).unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, 1.5);
    }

    #[test]
    fn parse_bars_rejects_a_non_numeric_required_field() {
        let raw = vec![json!({"o": "not-a-number", "h": 2.0, "l": 0.5, "c": 1.5})];
        let result = parse_bars(&raw);
        assert!(matches!(result, Err(DataError::UnexpectedResponse(_))));
    }

    #[test]
    fn parse_bars_defaults_missing_timestamp_to_now() {
        let raw = vec![json!({"o": 1.0, "h": 2.0, "l": 0.5, "c": 1.5})];
        let bars = parse_bars(&raw).unwrap();
        assert!(!bars[0].timestamp.is_empty());
    }

    #[test]
    fn parse_bars_defaults_missing_volume_to_zero() {
        let raw = vec![json!({"o": 1.0, "h": 2.0, "l": 0.5, "c": 1.5})];
        let bars = parse_bars(&raw).unwrap();
        assert_eq!(bars[0].volume, 0.0);
    }

    fn sample_bar() -> Bar {
        Bar { timestamp: "2026-07-19T00:00:00Z".to_string(), open: 1.0, high: 2.0, low: 0.5, close: 1.5, volume: 10.0 }
    }

    #[test]
    fn build_quote_uses_bp_ap_when_present_and_nonzero() {
        let q = build_quote("SPY", &json!({"bp": 1.1, "ap": 1.2, "t": "2026-07-19T01:00:00Z"}), &sample_bar(), "iex", "open");
        assert_eq!(q.bid, 1.1);
        assert_eq!(q.ask, 1.2);
    }

    #[test]
    fn build_quote_falls_back_to_last_close_when_bp_is_exactly_zero() {
        // Matches Python's `q.get("bp") or bars[-1].close` falsy-zero semantics.
        let q = build_quote("SPY", &json!({"bp": 0.0, "ap": 1.2}), &sample_bar(), "iex", "open");
        assert_eq!(q.bid, 1.5);
    }

    #[test]
    fn build_quote_falls_back_to_last_close_when_quote_json_is_null() {
        let q = build_quote("SPY", &Value::Null, &sample_bar(), "iex", "open");
        assert_eq!(q.bid, 1.5);
        assert_eq!(q.ask, 1.5);
    }

    #[test]
    fn build_quote_source_includes_the_feed() {
        let q = build_quote("SPY", &Value::Null, &sample_bar(), "sip", "open");
        assert_eq!(q.source, "alpaca:sip");
    }

    #[test]
    fn iso_replaces_the_utc_offset_suffix() {
        assert_eq!(iso("2026-07-19T00:00:00+00:00"), "2026-07-19T00:00:00Z");
    }

    /// Real end-to-end test: a local mock server stands in for
    /// `data.alpaca.markets` (its base URL is already configurable via
    /// `provider.alpaca.data_base_url`, no code change needed), returning
    /// the exact JSON shapes the real quotes-latest/bars endpoints return,
    /// and `AlpacaReadOnlyProvider::snapshot` makes real HTTP requests
    /// against it — not just the pure `parse_bars`/`build_quote` units
    /// above.
    #[test]
    fn snapshot_makes_a_real_http_round_trip_against_a_mock_alpaca() {
        let port = crate::mock_http_test_support::start(vec![
            (
                "/v2/stocks/SPY/quotes/latest",
                (200, "application/json", r#"{"quote": {"bp": 99.5, "ap": 100.5, "t": "2026-07-19T20:00:00Z"}}"#.to_string()),
            ),
            (
                "/v2/stocks/SPY/bars",
                (
                    200,
                    "application/json",
                    r#"{"bars": [
                        {"t": "2026-07-19T19:59:00Z", "o": 99.0, "h": 100.0, "l": 98.5, "c": 99.8, "v": 500.0},
                        {"t": "2026-07-19T20:00:00Z", "o": 99.8, "h": 100.2, "l": 99.5, "c": 100.0, "v": 700.0}
                    ]}"#
                        .to_string(),
                ),
            ),
        ]);

        let dir = std::env::temp_dir().join(format!("smart-hedge-data-alpaca-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            format!(r#"{{"provider": {{"alpaca": {{"data_base_url": "http://127.0.0.1:{port}"}}}}}}"#),
        )
        .unwrap();
        let loaded = smart_hedge_config::load_config(Some(&config_path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        let provider = AlpacaReadOnlyProvider::new(loaded, "key".to_string(), "secret".to_string()).unwrap();
        let snapshot = provider.snapshot("spy").expect("snapshot should succeed against the mock server");

        assert_eq!(snapshot.symbol, "SPY");
        assert_eq!(snapshot.bars.len(), 2);
        assert_eq!(snapshot.quote.bid, 99.5);
        assert_eq!(snapshot.quote.ask, 100.5);
        assert_eq!(snapshot.quote.source, "alpaca:iex");
    }

    /// Same real HTTP path, but the mock quotes-latest endpoint returns a
    /// non-2xx status — `snapshot` should surface that as an `Err`, not
    /// panic or silently substitute a default quote.
    #[test]
    fn snapshot_surfaces_a_real_http_error_from_the_quote_endpoint() {
        let port = crate::mock_http_test_support::start(vec![(
            "/v2/stocks/SPY/quotes/latest",
            (500, "text/plain", "internal error".to_string()),
        )]);

        let dir = std::env::temp_dir().join(format!("smart-hedge-data-alpaca-e2e-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            format!(r#"{{"provider": {{"alpaca": {{"data_base_url": "http://127.0.0.1:{port}"}}}}}}"#),
        )
        .unwrap();
        let loaded = smart_hedge_config::load_config(Some(&config_path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        let provider = AlpacaReadOnlyProvider::new(loaded, "key".to_string(), "secret".to_string()).unwrap();
        let result = provider.snapshot("SPY");
        assert!(matches!(result, Err(DataError::Http(_))));
    }
}
