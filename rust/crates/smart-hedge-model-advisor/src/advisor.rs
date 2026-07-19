use smart_hedge_config::ContractConfig;
use smart_hedge_models::{CoreResponse, FeatureSet, MarketSnapshot, ModelAssessment};

use crate::error::AdvisorError;

/// Port of the `Advisor` `Protocol` in `model_advisor.py`. Fallible (unlike
/// Python's `HeuristicAdvisor`, which never raises) so
/// `smart-hedge-engine`'s fallback-on-failure logic can be built and
/// tested against this trait now — see `error::AdvisorError`.
/// `Send + Sync` for the same reason as
/// `smart_hedge_data::MarketDataProvider` — see that trait's doc comment.
pub trait Advisor: Send + Sync {
    fn assess(
        &self,
        snapshot: &MarketSnapshot,
        features: &FeatureSet,
        core: &CoreResponse,
        contract: &ContractConfig,
    ) -> Result<ModelAssessment, AdvisorError>;

    /// A short, stable name for this adviser implementation — used by
    /// `smart-hedge-engine`'s `health()` report, matching Python's
    /// `type(self.advisor).__name__`.
    fn name(&self) -> &'static str;
}
