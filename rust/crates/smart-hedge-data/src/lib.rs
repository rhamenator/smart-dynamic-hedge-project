//! Rust port of `python/smart_hedge/data.py`'s `SyntheticProvider` and
//! evidence-file loading. `AlpacaReadOnlyProvider`, FRED, and RSS evidence
//! are **not yet ported** — each needs an HTTP-client (and, for RSS, an
//! XML-parser) dependency decision, deferred per `requirements/LLR.md`
//! `SDH-LLR-126`. See `docs/ROADMAP.md` "Language and dependency policy".

pub mod error;
pub mod evidence_file;
pub mod provider;
pub mod rng;
pub mod synthetic;

pub use error::DataError;
pub use evidence_file::load_evidence_file;
pub use provider::MarketDataProvider;
pub use synthetic::SyntheticProvider;
