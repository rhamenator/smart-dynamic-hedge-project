use std::fmt;

/// Port of the `ValueError`s Python's `validate_assessment_payload` raises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    KeyMismatch { missing: Vec<String>, extra: Vec<String> },
    InvalidRegime(String),
    NotNumeric(String),
    OutOfRange { field: String },
    NotAList { field: String, max: usize },
    ListItemNotAString { field: String },
    ScenarioShocksCountOutOfRange,
    SummaryInvalid,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyMismatch { missing, extra } => {
                write!(f, "assessment keys mismatch; missing={missing:?}, extra={extra:?}")
            }
            Self::InvalidRegime(r) => write!(f, "invalid regime: {r}"),
            Self::NotNumeric(field) => write!(f, "{field} must be numeric"),
            Self::OutOfRange { field } => write!(f, "{field} is out of its allowed range"),
            Self::NotAList { field, max } => write!(f, "{field} must be a list with at most {max} items"),
            Self::ListItemNotAString { field } => write!(f, "{field} items must be strings"),
            Self::ScenarioShocksCountOutOfRange => {
                write!(f, "scenario_spot_shocks must contain 1 to 7 values")
            }
            Self::SummaryInvalid => write!(f, "summary must be a string no longer than 1000 characters"),
        }
    }
}

impl std::error::Error for SchemaError {}

/// Errors an `Advisor` implementation can return from `assess`. Only
/// `HeuristicAdvisor` exists today and never fails; this exists so
/// `smart-hedge-engine`'s fallback-on-adviser-failure logic (SDH-LLR-057)
/// can be built and tested against the trait now, using a
/// deliberately-failing test stub, without waiting for a real fallible
/// adviser (e.g. an OpenAI-backed one) to exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorError(pub String);

impl fmt::Display for AdvisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AdvisorError {}
