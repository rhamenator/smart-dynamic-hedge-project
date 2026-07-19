use serde::{Deserialize, Serialize};

/// Port of `smart_hedge.models.ModelAssessment`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelAssessment {
    pub advisor_kind: String,
    pub model: String,
    pub regime: String,
    pub confidence: f64,
    pub hedge_urgency: f64,
    pub band_multiplier: f64,
    pub summary: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub scenario_spot_shocks: Vec<f64>,
    #[serde(default)]
    pub data_requests: Vec<String>,
    #[serde(default)]
    pub raw_response_id: String,
    #[serde(default)]
    pub fallback_reason: String,
}
