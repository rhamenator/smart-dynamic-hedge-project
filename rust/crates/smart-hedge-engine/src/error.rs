use std::fmt;

#[derive(Debug)]
pub enum EngineError {
    /// No `contracts.<SYMBOL>` entry exists in the config.
    UnknownSymbol(String),
    InvalidOptionType(String),
    InvalidExerciseStyle(String),
    InvalidStrike(String),
    InvalidExpiryDate(String),
    Core(smart_hedge_core_bridge::CoreError),
    Data(smart_hedge_data::DataError),
    Store(smart_hedge_store::StoreError),
    /// The active adviser failed and `model.fallback_to_heuristic` is
    /// false, so the failure propagates instead of falling back.
    AdvisorFailedAndFallbackDisabled(smart_hedge_model_advisor::AdvisorError),
    /// A *configuration* problem building the adviser itself (e.g. the
    /// OpenAI adviser missing `OPENAI_API_KEY`) — distinct from a runtime
    /// `assess()` failure, which `AdvisorFailedAndFallbackDisabled` /
    /// `model.fallback_to_heuristic` covers instead. This always fails
    /// construction outright; there is no fallback for "the config names
    /// an adviser that can't even be built."
    AdvisorConstructionFailed(smart_hedge_model_advisor::AdvisorError),
    /// `replay`/similar looked up a decision ID that doesn't exist.
    DecisionNotFound(String),
    UnknownProviderKind(String),
    UnknownAdvisorKind(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSymbol(s) => write!(f, "no contract configured for {s}; add contracts.{s} to the config"),
            Self::InvalidOptionType(s) => write!(f, "option_type must be call or put, got {s}"),
            Self::InvalidExerciseStyle(s) => write!(f, "exercise_style must be american or european, got {s}"),
            Self::InvalidStrike(s) => write!(f, "strike must be positive and finite, got {s}"),
            Self::InvalidExpiryDate(s) => write!(f, "invalid expiry date: {s}"),
            Self::Core(e) => write!(f, "{e}"),
            Self::Data(e) => write!(f, "{e}"),
            Self::Store(e) => write!(f, "{e}"),
            Self::AdvisorFailedAndFallbackDisabled(e) => write!(f, "adviser failed and fallback is disabled: {e}"),
            Self::AdvisorConstructionFailed(e) => write!(f, "failed to construct the configured adviser: {e}"),
            Self::DecisionNotFound(id) => write!(f, "decision not found: {id}"),
            Self::UnknownProviderKind(k) => write!(f, "unknown provider kind: {k}"),
            Self::UnknownAdvisorKind(k) => write!(f, "unknown model adviser kind: {k}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<smart_hedge_core_bridge::CoreError> for EngineError {
    fn from(err: smart_hedge_core_bridge::CoreError) -> Self {
        EngineError::Core(err)
    }
}

impl From<smart_hedge_data::DataError> for EngineError {
    fn from(err: smart_hedge_data::DataError) -> Self {
        EngineError::Data(err)
    }
}

impl From<smart_hedge_store::StoreError> for EngineError {
    fn from(err: smart_hedge_store::StoreError) -> Self {
        EngineError::Store(err)
    }
}
