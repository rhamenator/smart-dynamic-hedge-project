use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Port of `smart_hedge.models.FeatureSet`. `values` is `dict[str, float |
/// str | bool | None]` in Python — a `BTreeMap` (not `HashMap`) so JSON
/// output has deterministic key ordering, matching this codebase's general
/// preference for reproducible output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureSet {
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub data_quality: f64,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}
