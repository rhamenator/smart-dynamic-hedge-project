//! Rust port of `python/smart_hedge/data.py`: `SyntheticProvider`,
//! `AlpacaReadOnlyProvider`, evidence-file loading, and FRED/RSS evidence.
//! `AlpacaReadOnlyProvider`/FRED/RSS need real HTTPS calls to third-party
//! hosts — see this crate's `Cargo.toml` for the `ureq`/`rustls` dependency
//! decision (a documented exception to "hand-roll instead of depend", same
//! reasoning as `smart-hedge-store`'s `rusqlite`). RSS additionally needs a
//! minimal, narrowly-scoped, hand-rolled XML extractor (`rss_xml`) — see
//! that module's doc comment for why hand-rolling it is actually the
//! *safer* choice here (no DTD/entity support at all means no XXE surface
//! to accidentally enable).

pub mod alpaca;
pub mod error;
pub mod evidence_file;
pub mod fred;
pub mod market_hours;
pub mod provider;
pub mod rng;
pub mod rss;
pub mod rss_xml;
pub mod synthetic;

pub use alpaca::AlpacaReadOnlyProvider;
pub use error::DataError;
pub use evidence_file::load_evidence_file;
pub use fred::{load_fred_evidence, load_fred_evidence_from_env};
pub use market_hours::regular_market_state;
pub use provider::MarketDataProvider;
pub use rss::load_rss_evidence;
pub use synthetic::SyntheticProvider;
