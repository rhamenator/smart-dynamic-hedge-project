use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Port of `smart_hedge.models.EvidenceItem`. `value` is `float | str | bool
/// | None` in Python; represented here as `serde_json::Value` to preserve
/// that dynamism rather than forcing a premature choice of type — this
/// mirrors how the field is actually used (passed through to JSON output,
/// not computed on).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub evidence_id: String,
    pub kind: String,
    pub title: String,
    pub timestamp: String,
    pub source: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub text: String,
    #[serde(default = "default_quality")]
    pub quality: f64,
    #[serde(default = "default_untrusted_text")]
    pub untrusted_text: bool,
}

fn default_quality() -> f64 {
    0.5
}

fn default_untrusted_text() -> bool {
    true
}
