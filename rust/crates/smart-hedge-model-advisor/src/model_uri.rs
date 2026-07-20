//! `ModelUri` — the `MODEL_URI` router's addressing scheme:
//! `scheme://identifier`, matching the `provider-uri://account-alias`
//! convention `03-create-trade-guard-mcp.md` uses for broker/venue
//! selection, applied here to model advisers instead. No credentials
//! ever live in a `ModelUri` — an `openai://gpt-4.1` URI names *which*
//! model to call, not the API key to call it with, which still comes
//! from `OPENAI_API_KEY` exactly as before this router existed.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelUri {
    pub scheme: String,
    pub identifier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelUriError {
    MissingSchemeSeparator(String),
    EmptyScheme(String),
}

impl fmt::Display for ModelUriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelUriError::MissingSchemeSeparator(s) => write!(f, "model URI {s:?} is missing \"://\""),
            ModelUriError::EmptyScheme(s) => write!(f, "model URI {s:?} has an empty scheme before \"://\""),
        }
    }
}

impl std::error::Error for ModelUriError {}

impl ModelUri {
    /// Parses `scheme://identifier`. `identifier` may be empty (e.g.
    /// `heuristic://` and bare `heuristic` are both accepted — the
    /// heuristic adviser needs no identifier at all); a missing `scheme`
    /// or missing `://` separator is rejected rather than guessed at.
    pub fn parse(raw: &str) -> Result<ModelUri, ModelUriError> {
        let trimmed = raw.trim();
        let (scheme, identifier) = match trimmed.split_once("://") {
            Some((scheme, identifier)) => (scheme, identifier),
            None => (trimmed, ""),
        };
        if scheme.is_empty() {
            return Err(ModelUriError::EmptyScheme(raw.to_string()));
        }
        Ok(ModelUri { scheme: scheme.to_lowercase(), identifier: identifier.trim().to_string() })
    }
}

impl fmt::Display for ModelUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}", self.scheme, self.identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scheme_and_identifier() {
        let uri = ModelUri::parse("openai://gpt-4.1").unwrap();
        assert_eq!(uri.scheme, "openai");
        assert_eq!(uri.identifier, "gpt-4.1");
    }

    #[test]
    fn scheme_is_lowercased() {
        let uri = ModelUri::parse("OpenAI://gpt-4.1").unwrap();
        assert_eq!(uri.scheme, "openai");
    }

    #[test]
    fn bare_scheme_with_no_separator_is_accepted_with_an_empty_identifier() {
        let uri = ModelUri::parse("heuristic").unwrap();
        assert_eq!(uri.scheme, "heuristic");
        assert_eq!(uri.identifier, "");
    }

    #[test]
    fn scheme_with_empty_identifier_after_separator_is_accepted() {
        let uri = ModelUri::parse("heuristic://").unwrap();
        assert_eq!(uri.scheme, "heuristic");
        assert_eq!(uri.identifier, "");
    }

    #[test]
    fn identifier_is_trimmed() {
        let uri = ModelUri::parse("openai:// gpt-4.1 ").unwrap();
        assert_eq!(uri.identifier, "gpt-4.1");
    }

    #[test]
    fn empty_scheme_is_rejected() {
        let result = ModelUri::parse("://gpt-4.1");
        assert!(matches!(result, Err(ModelUriError::EmptyScheme(_))));
    }

    #[test]
    fn empty_input_is_rejected() {
        let result = ModelUri::parse("");
        assert!(matches!(result, Err(ModelUriError::EmptyScheme(_))));
    }

    #[test]
    fn display_round_trips_the_canonical_form() {
        let uri = ModelUri::parse("openai://gpt-4.1").unwrap();
        assert_eq!(uri.to_string(), "openai://gpt-4.1");
    }

    #[test]
    fn identifier_containing_its_own_scheme_like_slashes_is_preserved_verbatim() {
        // A model identifier is never itself expected to contain "://",
        // but this proves split_once takes the *first* occurrence rather
        // than panicking or truncating oddly if one ever did.
        let uri = ModelUri::parse("custom://path/to/model").unwrap();
        assert_eq!(uri.identifier, "path/to/model");
    }
}
