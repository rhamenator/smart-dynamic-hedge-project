use std::collections::BTreeSet;

use serde_json::Value;
use smart_hedge_models::ModelAssessment;

use crate::error::SchemaError;

/// Port of `ALLOWED_REGIMES`.
pub const ALLOWED_REGIMES: [&str; 7] =
    ["calm", "trend_up", "trend_down", "volatile", "jump_risk", "illiquid", "uncertain"];

const REQUIRED_KEYS: [&str; 9] = [
    "regime",
    "confidence",
    "hedge_urgency",
    "band_multiplier",
    "summary",
    "evidence_ids",
    "risks",
    "scenario_spot_shocks",
    "data_requests",
];

fn finite_number(value: &Value, field: &str, low: f64, high: f64) -> Result<f64, SchemaError> {
    let n = value.as_f64().ok_or_else(|| SchemaError::NotNumeric(field.to_string()))?;
    // `as_f64()` on a `Value::Bool` returns `None` already (bools are a
    // distinct JSON type from numbers), so the "not a bool" exclusion
    // Python needs explicitly is automatic here.
    if !n.is_finite() || !(low..=high).contains(&n) {
        return Err(SchemaError::OutOfRange { field: field.to_string() });
    }
    Ok(n)
}

/// Port of `_string_list`: rejects a non-list or an over-long list, but
/// *truncates* (not rejects) any individual string longer than `item_max`
/// characters — matching Python's `item[:item_max]` exactly.
fn string_list(value: &Value, field: &str, max_items: usize, item_max: usize) -> Result<Vec<String>, SchemaError> {
    let arr = value.as_array().ok_or_else(|| SchemaError::NotAList { field: field.to_string(), max: max_items })?;
    if arr.len() > max_items {
        return Err(SchemaError::NotAList { field: field.to_string(), max: max_items });
    }
    arr.iter()
        .map(|item| {
            item.as_str()
                .map(|s| s.chars().take(item_max).collect())
                .ok_or_else(|| SchemaError::ListItemNotAString { field: field.to_string() })
        })
        .collect()
}

/// Port of `validate_assessment_payload`.
pub fn validate_assessment_payload(
    payload: &Value,
    advisor_kind: &str,
    model: &str,
    response_id: &str,
) -> Result<ModelAssessment, SchemaError> {
    let obj = payload.as_object().ok_or_else(|| SchemaError::KeyMismatch {
        missing: REQUIRED_KEYS.iter().map(|s| s.to_string()).collect(),
        extra: vec![],
    })?;

    let expected: BTreeSet<&str> = REQUIRED_KEYS.into_iter().collect();
    let actual: BTreeSet<&str> = obj.keys().map(String::as_str).collect();
    if actual != expected {
        let missing: Vec<String> = expected.difference(&actual).map(|s| s.to_string()).collect();
        let extra: Vec<String> = actual.difference(&expected).map(|s| s.to_string()).collect();
        return Err(SchemaError::KeyMismatch { missing, extra });
    }

    let regime = payload["regime"]
        .as_str()
        .ok_or_else(|| SchemaError::InvalidRegime(payload["regime"].to_string()))?;
    if !ALLOWED_REGIMES.contains(&regime) {
        return Err(SchemaError::InvalidRegime(regime.to_string()));
    }

    let shocks_raw = payload["scenario_spot_shocks"]
        .as_array()
        .ok_or(SchemaError::ScenarioShocksCountOutOfRange)?;
    if !(1..=7).contains(&shocks_raw.len()) {
        return Err(SchemaError::ScenarioShocksCountOutOfRange);
    }
    let shocks = shocks_raw
        .iter()
        .map(|x| finite_number(x, "scenario shock", -0.30, 0.30))
        .collect::<Result<Vec<f64>, SchemaError>>()?;

    let summary = payload["summary"].as_str().filter(|s| s.chars().count() <= 1000).ok_or(SchemaError::SummaryInvalid)?;

    Ok(ModelAssessment {
        advisor_kind: advisor_kind.to_string(),
        model: model.to_string(),
        regime: regime.to_string(),
        confidence: finite_number(&payload["confidence"], "confidence", 0.0, 1.0)?,
        hedge_urgency: finite_number(&payload["hedge_urgency"], "hedge_urgency", 0.0, 1.0)?,
        band_multiplier: finite_number(&payload["band_multiplier"], "band_multiplier", 0.5, 3.0)?,
        summary: summary.to_string(),
        evidence_ids: string_list(&payload["evidence_ids"], "evidence_ids", 8, 160)?,
        risks: string_list(&payload["risks"], "risks", 8, 240)?,
        scenario_spot_shocks: shocks,
        data_requests: string_list(&payload["data_requests"], "data_requests", 8, 240)?,
        raw_response_id: response_id.to_string(),
        fallback_reason: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_payload() -> Value {
        json!({
            "regime": "uncertain",
            "confidence": 0.5,
            "hedge_urgency": 0.5,
            "band_multiplier": 1.0,
            "summary": "No strong regime.",
            "evidence_ids": [],
            "risks": [],
            "scenario_spot_shocks": [-0.05, 0.05],
            "data_requests": []
        })
    }

    /// Transcription of `test_model_schema.py::test_valid_payload`.
    #[test]
    fn valid_payload_is_accepted() {
        let result = validate_assessment_payload(&valid_payload(), "test", "test", "").unwrap();
        assert_eq!(result.regime, "uncertain");
    }

    /// Transcription of `test_model_schema.py::test_extra_trade_field_rejected`.
    #[test]
    fn extra_field_is_rejected() {
        let mut payload = valid_payload();
        payload["buy_shares"] = json!(100);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::KeyMismatch { .. })));
    }

    /// Transcription of `test_model_schema.py::test_out_of_range_band_rejected`.
    #[test]
    fn out_of_range_band_multiplier_is_rejected() {
        let mut payload = valid_payload();
        payload["band_multiplier"] = json!(50);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::OutOfRange { .. })));
    }

    #[test]
    fn missing_field_is_rejected() {
        let mut payload = valid_payload();
        payload.as_object_mut().unwrap().remove("summary");
        let result = validate_assessment_payload(&payload, "test", "test", "");
        match result {
            Err(SchemaError::KeyMismatch { missing, .. }) => assert_eq!(missing, vec!["summary".to_string()]),
            other => panic!("expected KeyMismatch, got {other:?}"),
        }
    }

    #[test]
    fn invalid_regime_is_rejected() {
        let mut payload = valid_payload();
        payload["regime"] = json!("not-a-real-regime");
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::InvalidRegime(_))));
    }

    #[test]
    fn regime_as_non_string_is_rejected() {
        let mut payload = valid_payload();
        payload["regime"] = json!(5);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::InvalidRegime(_))));
    }

    #[test]
    fn boolean_confidence_is_rejected_as_non_numeric() {
        let mut payload = valid_payload();
        payload["confidence"] = json!(true);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::NotNumeric(_))));
    }

    #[test]
    fn nonfinite_scenario_shock_is_rejected() {
        // JSON has no NaN/Infinity literal, but a value out of [-0.30, 0.30]
        // exercises the same range check.
        let mut payload = valid_payload();
        payload["scenario_spot_shocks"] = json!([0.5]);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::OutOfRange { .. })));
    }

    #[test]
    fn zero_scenario_shocks_is_rejected() {
        let mut payload = valid_payload();
        payload["scenario_spot_shocks"] = json!([]);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::ScenarioShocksCountOutOfRange)));
    }

    #[test]
    fn too_many_scenario_shocks_is_rejected() {
        let mut payload = valid_payload();
        payload["scenario_spot_shocks"] = json!([0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08]);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::ScenarioShocksCountOutOfRange)));
    }

    #[test]
    fn string_list_items_are_truncated_not_rejected_when_too_long() {
        let mut payload = valid_payload();
        let long_risk = "x".repeat(300);
        payload["risks"] = json!([long_risk]);
        let result = validate_assessment_payload(&payload, "test", "test", "").unwrap();
        assert_eq!(result.risks[0].chars().count(), 240);
    }

    #[test]
    fn too_many_list_items_is_rejected() {
        let mut payload = valid_payload();
        payload["risks"] = json!(["a", "b", "c", "d", "e", "f", "g", "h", "i"]); // 9 > max of 8
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::NotAList { .. })));
    }

    #[test]
    fn non_string_list_item_is_rejected() {
        let mut payload = valid_payload();
        payload["risks"] = json!([5]);
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::ListItemNotAString { .. })));
    }

    #[test]
    fn overlong_summary_is_rejected() {
        let mut payload = valid_payload();
        payload["summary"] = json!("x".repeat(1001));
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(matches!(result, Err(SchemaError::SummaryInvalid)));
    }

    #[test]
    fn summary_at_exactly_the_length_limit_is_accepted() {
        let mut payload = valid_payload();
        payload["summary"] = json!("x".repeat(1000));
        let result = validate_assessment_payload(&payload, "test", "test", "");
        assert!(result.is_ok());
    }
}
