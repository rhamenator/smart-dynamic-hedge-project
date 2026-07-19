use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assessment::ModelAssessment;
use crate::features::FeatureSet;
use crate::policy_decision::PolicyDecision;
use crate::snapshot::MarketSnapshot;

/// Port of `smart_hedge.models.Recommendation`. `contract`,
/// `deterministic_core`, and `audit` are `dict[str, Any]` passthrough
/// blobs in Python (the contract config, the raw C++ core JSON response,
/// and audit metadata respectively) — kept as `serde_json::Value` here for
/// the same reason `EvidenceItem::value` is: they are genuinely dynamic,
/// not something this crate computes on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    pub decision_id: String,
    pub created_at: String,
    pub mode: String,
    pub symbol: String,
    pub contract: Value,
    pub snapshot: MarketSnapshot,
    pub features: FeatureSet,
    pub deterministic_core: Value,
    pub model_assessment: ModelAssessment,
    pub policy: PolicyDecision,
    pub audit: Value,
}
