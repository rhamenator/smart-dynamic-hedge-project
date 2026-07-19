use std::time::Duration;

use serde_json::Value;
use smart_hedge_config::LoadedConfig;
use smart_hedge_models::{EvidenceItem, TimestampUtc};

const USER_AGENT: &str = "smart-dynamic-hedge/0.2";
const MAX_SERIES: usize = 20;

fn missing_key_evidence() -> Vec<EvidenceItem> {
    vec![EvidenceItem {
        evidence_id: "fred-missing-key".to_string(),
        kind: "data_quality".to_string(),
        title: "FRED connector disabled at runtime".to_string(),
        timestamp: TimestampUtc::now().to_iso_string(),
        source: "fred".to_string(),
        value: Value::Null,
        text: "FRED_API_KEY is not set.".to_string(),
        quality: 1.0,
        untrusted_text: false,
    }]
}

fn error_evidence(series_id: &str, reason: &str) -> EvidenceItem {
    EvidenceItem {
        evidence_id: format!("fred-error-{series_id}"),
        kind: "data_quality".to_string(),
        title: format!("FRED {series_id} retrieval error"),
        timestamp: TimestampUtc::now().to_iso_string(),
        source: "FRED".to_string(),
        value: Value::Null,
        text: reason.to_string(),
        quality: 1.0,
        untrusted_text: false,
    }
}

/// Parses the observations response into the one `EvidenceItem` Python
/// produces per series — a pure, directly testable core separated from the
/// network call itself. `raw_value not in (None, ".")`: FRED's own
/// convention for "no data at this observation" is the literal string
/// `"."`, not a missing/null field — replicated exactly.
fn observation_evidence(series_id: &str, payload: &Value) -> EvidenceItem {
    let empty = Value::Null;
    let observation = payload.get("observations").and_then(Value::as_array).and_then(|arr| arr.first()).unwrap_or(&empty);
    let date = observation.get("date").and_then(Value::as_str);
    let raw_value = observation.get("value");
    let numeric = match raw_value {
        Some(Value::String(s)) if s != "." => s.parse::<f64>().ok(),
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    };
    EvidenceItem {
        evidence_id: format!("fred-{series_id}-{}", date.unwrap_or("latest")),
        kind: "macro".to_string(),
        title: format!("FRED {series_id}"),
        timestamp: date.map(str::to_string).unwrap_or_else(|| TimestampUtc::now().to_iso_string()),
        source: "FRED".to_string(),
        value: numeric.map(Value::from).unwrap_or(Value::Null),
        text: String::new(),
        quality: 0.9,
        untrusted_text: false,
    }
}

fn fetch_observations(series_id: &str, api_key: &str, timeout: Duration) -> Result<Value, String> {
    let response = ureq::get("https://api.stlouisfed.org/fred/series/observations")
        .set("User-Agent", USER_AGENT)
        .query("series_id", series_id)
        .query("api_key", api_key)
        .query("file_type", "json")
        .query("sort_order", "desc")
        .query("limit", "1")
        .timeout(timeout)
        .call()
        .map_err(|e| e.to_string())?;
    let text = response.into_string().map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// Port of `data.load_fred_evidence`. Never returns an error — a connector
/// failure becomes a `data_quality` evidence item, not a process failure,
/// matching Python's `except Exception as exc: output.append(...)`.
/// `api_key` is an explicit parameter (not read from the environment
/// directly) for the same testability reason as
/// `AlpacaReadOnlyProvider::new` — see that module.
pub fn load_fred_evidence(loaded: &LoadedConfig, api_key: Option<&str>) -> Vec<EvidenceItem> {
    let fred = &loaded.config.provider.fred;
    if !fred.enabled {
        return vec![];
    }
    let Some(api_key) = api_key.filter(|k| !k.is_empty()) else {
        return missing_key_evidence();
    };
    let timeout = Duration::from_secs_f64(fred.timeout_seconds.max(0.0));

    capped_series(&fred.series)
        .iter()
        .map(|series_id| match fetch_observations(series_id, api_key, timeout) {
            Ok(payload) => observation_evidence(series_id, &payload),
            Err(reason) => error_evidence(series_id, &reason),
        })
        .collect()
}

/// Python's `list(fred.get("series", []))[:20]` — a pure slice, split out
/// so the cap itself is testable without making any real network call.
fn capped_series(series: &[String]) -> &[String] {
    &series[..series.len().min(MAX_SERIES)]
}

pub fn load_fred_evidence_from_env(loaded: &LoadedConfig) -> Vec<EvidenceItem> {
    let api_key = std::env::var("FRED_API_KEY").ok();
    load_fred_evidence(loaded, api_key.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use smart_hedge_config::EnvOverrides;

    fn config_with_fred(fred_json: &str) -> LoadedConfig {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-data-fred-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"provider": {{"fred": {fred_json}}}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    #[test]
    fn disabled_fred_returns_no_evidence() {
        let loaded = config_with_fred(r#"{"enabled": false}"#);
        assert!(load_fred_evidence(&loaded, Some("key")).is_empty());
    }

    #[test]
    fn enabled_without_a_key_reports_the_connector_as_disabled() {
        let loaded = config_with_fred(r#"{"enabled": true, "series": ["VIXCLS"]}"#);
        let evidence = load_fred_evidence(&loaded, None);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].evidence_id, "fred-missing-key");
    }

    #[test]
    fn enabled_with_an_empty_key_string_is_treated_as_no_key() {
        let loaded = config_with_fred(r#"{"enabled": true, "series": ["VIXCLS"]}"#);
        let evidence = load_fred_evidence(&loaded, Some(""));
        assert_eq!(evidence[0].evidence_id, "fred-missing-key");
    }

    #[test]
    fn observation_evidence_parses_a_numeric_value() {
        let payload = json!({"observations": [{"date": "2026-07-18", "value": "18.5"}]});
        let item = observation_evidence("VIXCLS", &payload);
        assert_eq!(item.value, json!(18.5));
        assert_eq!(item.timestamp, "2026-07-18");
        assert_eq!(item.evidence_id, "fred-VIXCLS-2026-07-18");
    }

    /// FRED's own convention for "no observation value" is the literal
    /// string `"."`, not a missing/null field.
    #[test]
    fn observation_evidence_treats_the_dot_placeholder_as_missing() {
        let payload = json!({"observations": [{"date": "2026-07-18", "value": "."}]});
        let item = observation_evidence("VIXCLS", &payload);
        assert_eq!(item.value, Value::Null);
    }

    #[test]
    fn observation_evidence_handles_an_empty_observations_array() {
        let payload = json!({"observations": []});
        let item = observation_evidence("VIXCLS", &payload);
        assert_eq!(item.value, Value::Null);
        assert_eq!(item.evidence_id, "fred-VIXCLS-latest");
    }

    #[test]
    fn error_evidence_is_kind_data_quality_not_macro() {
        let item = error_evidence("VIXCLS", "Transport");
        assert_eq!(item.kind, "data_quality");
        assert_eq!(item.text, "Transport");
    }

    /// Pure logic, no network call: `load_fred_evidence` itself would need
    /// a real reachable FRED endpoint to test the cap end-to-end, which
    /// automated tests must not depend on.
    #[test]
    fn series_list_is_capped_at_twenty() {
        let series: Vec<String> = (0..30).map(|i| format!("S{i}")).collect();
        assert_eq!(capped_series(&series).len(), MAX_SERIES);
    }

    #[test]
    fn series_list_shorter_than_the_cap_is_unaffected() {
        let series: Vec<String> = (0..5).map(|i| format!("S{i}")).collect();
        assert_eq!(capped_series(&series).len(), 5);
    }
}
