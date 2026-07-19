use smart_hedge_models::MarketSnapshot;

use crate::error::DataError;

/// Port of the `MarketDataProvider` `Protocol` in `data.py`. `Send + Sync`
/// so `Box<dyn MarketDataProvider>` (and, transitively, `SmartHedgeEngine`)
/// can be shared across the dashboard's per-connection threads via a plain
/// `Arc` — every current implementor (`SyntheticProvider`,
/// `AlpacaReadOnlyProvider`) already satisfies this automatically, since
/// neither holds anything but owned, non-`Rc`/non-interior-mutable data.
pub trait MarketDataProvider: Send + Sync {
    fn snapshot(&self, symbol: &str) -> Result<MarketSnapshot, DataError>;

    /// A short, stable name for this provider implementation — used by
    /// `smart-hedge-engine`'s `health()` report, matching Python's
    /// `type(self.provider).__name__`. An explicit method rather than
    /// `std::any::type_name`, which would produce a noisy fully-qualified
    /// path instead of a clean name.
    fn name(&self) -> &'static str;
}
