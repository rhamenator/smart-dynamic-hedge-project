use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A configured contract's `strike`: either a fixed number, or the
/// literal (case-insensitive) string `"ATM"`, resolved dynamically from
/// the live quote at recommendation time — see
/// `smart_hedge_engine::contract` (SDH-LLR-131). Python represents this
/// with no schema at all (`contract.get("strike")` is `str | float`,
/// checked with `isinstance` at read time); this is the typed
/// equivalent, so a config that will actually work in Python
/// deserializes here too, instead of failing at config-load time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrikeSpec {
    Atm,
    Fixed(f64),
}

impl<'de> Deserialize<'de> for StrikeSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match &value {
            Value::String(s) if s.eq_ignore_ascii_case("atm") => Ok(StrikeSpec::Atm),
            Value::Number(n) => n
                .as_f64()
                .map(StrikeSpec::Fixed)
                .ok_or_else(|| de::Error::custom("strike number is out of range for f64")),
            other => Err(de::Error::custom(format!(
                "strike must be a number or the string \"ATM\", got {other}"
            ))),
        }
    }
}

impl Serialize for StrikeSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            StrikeSpec::Atm => serializer.serialize_str("ATM"),
            StrikeSpec::Fixed(v) => serializer.serialize_f64(*v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_a_plain_number() {
        let spec: StrikeSpec = serde_json::from_str("100.0").unwrap();
        assert_eq!(spec, StrikeSpec::Fixed(100.0));
    }

    #[test]
    fn deserializes_the_atm_literal_case_insensitively() {
        for literal in ["\"ATM\"", "\"atm\"", "\"Atm\""] {
            let spec: StrikeSpec = serde_json::from_str(literal).unwrap();
            assert_eq!(spec, StrikeSpec::Atm);
        }
    }

    #[test]
    fn rejects_an_arbitrary_string() {
        let result: Result<StrikeSpec, _> = serde_json::from_str("\"not-atm\"");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_a_boolean() {
        let result: Result<StrikeSpec, _> = serde_json::from_str("true");
        assert!(result.is_err());
    }

    #[test]
    fn round_trips_through_serialization() {
        let fixed = StrikeSpec::Fixed(123.45);
        let json = serde_json::to_string(&fixed).unwrap();
        assert_eq!(json, "123.45");
        assert_eq!(serde_json::from_str::<StrikeSpec>(&json).unwrap(), fixed);

        let atm = StrikeSpec::Atm;
        let json = serde_json::to_string(&atm).unwrap();
        assert_eq!(json, "\"ATM\"");
        assert_eq!(serde_json::from_str::<StrikeSpec>(&json).unwrap(), atm);
    }
}
