use serde::{Deserialize, Serialize};

use crate::bar::Bar;
use crate::evidence::EvidenceItem;
use crate::quote::Quote;
use crate::time_util::TimestampUtc;

/// Port of `smart_hedge.models.MarketSnapshot`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub symbol: String,
    pub quote: Quote,
    pub bars: Vec<Bar>,
    pub evidence: Vec<EvidenceItem>,
    #[serde(default = "default_received_at")]
    pub received_at: String,
}

fn default_received_at() -> String {
    TimestampUtc::now().to_iso_string()
}

impl MarketSnapshot {
    pub fn new(symbol: impl Into<String>, quote: Quote, bars: Vec<Bar>, evidence: Vec<EvidenceItem>) -> Self {
        MarketSnapshot {
            symbol: symbol.into(),
            quote,
            bars,
            evidence,
            received_at: default_received_at(),
        }
    }
}
