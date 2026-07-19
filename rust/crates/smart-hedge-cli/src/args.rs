use std::fmt;
use std::path::PathBuf;

/// Port of the subset of `cli.py`'s `argparse` surface this binary
/// implements. `serve`/`mcp` are recognized (so the error message is
/// specific rather than "unknown command") but always fail — they need a
/// dependency decision (HTTP server, MCP-over-stdio) not yet made.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    BuildCore,
    Once { symbol: String, overrides: ContractOverrideArgs },
    Loop { symbol: String, overrides: ContractOverrideArgs, interval: f64 },
    Replay { decision_id: String },
    Recent { limit: i64, symbol: Option<String> },
    SelfTest { symbol: String },
    Serve,
    Mcp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ContractOverrideArgs {
    pub strike: Option<f64>,
    pub vol: Option<f64>,
    pub days: Option<f64>,
    pub current_shares: Option<f64>,
    pub contracts: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArgs {
    pub config: Option<PathBuf>,
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgsError {
    MissingCommand,
    UnknownCommand(String),
    UnknownFlag { command: String, flag: String },
    MissingValueFor(String),
    InvalidNumber { flag: String, value: String },
    MissingPositional { command: String, name: String },
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => write!(f, "a command is required (build-core, once, loop, replay, recent, serve, mcp, self-test)"),
            Self::UnknownCommand(c) => write!(f, "unknown command: {c}"),
            Self::UnknownFlag { command, flag } => write!(f, "unknown flag {flag} for command {command}"),
            Self::MissingValueFor(flag) => write!(f, "{flag} requires a value"),
            Self::InvalidNumber { flag, value } => write!(f, "{flag} expects a number, got {value}"),
            Self::MissingPositional { command, name } => write!(f, "{command} requires a {name} argument"),
        }
    }
}

impl std::error::Error for ArgsError {}

fn parse_f64(flag: &str, value: &str) -> Result<f64, ArgsError> {
    value.parse::<f64>().map_err(|_| ArgsError::InvalidNumber { flag: flag.to_string(), value: value.to_string() })
}

fn parse_i64(flag: &str, value: &str) -> Result<i64, ArgsError> {
    value.parse::<i64>().map_err(|_| ArgsError::InvalidNumber { flag: flag.to_string(), value: value.to_string() })
}

/// Consumes `--flag value` (or `--flag=value`) pairs from `tail`, returning
/// the remaining unconsumed tokens plus whatever flags it recognized. Kept
/// as one pass over the token list rather than one `.iter().position()`
/// scan per flag, so an unrecognized `--flag` is reported instead of
/// silently passing through as a positional argument.
struct FlagCursor<'a> {
    tokens: &'a [String],
    positionals: Vec<&'a str>,
}

impl<'a> FlagCursor<'a> {
    fn new(tokens: &'a [String]) -> Self {
        FlagCursor { tokens, positionals: Vec::new() }
    }

    /// Parses every token into `(flag_name, value)` pairs for `--name value`
    /// or `--name=value`, plus a leftover positional list, then hands each
    /// pair to `handle`. `handle` returns `Ok(true)` if it recognized the
    /// flag, `Ok(false)` if the caller should report it as unknown.
    fn run(mut self, command: &str, mut handle: impl FnMut(&str, &str) -> Result<bool, ArgsError>) -> Result<Vec<&'a str>, ArgsError> {
        let mut i = 0;
        while i < self.tokens.len() {
            let token = self.tokens[i].as_str();
            if let Some(rest) = token.strip_prefix("--") {
                if let Some((name, value)) = rest.split_once('=') {
                    if !handle(name, value)? {
                        return Err(ArgsError::UnknownFlag { command: command.to_string(), flag: format!("--{name}") });
                    }
                    i += 1;
                } else {
                    let value = self
                        .tokens
                        .get(i + 1)
                        .ok_or_else(|| ArgsError::MissingValueFor(format!("--{rest}")))?;
                    if !handle(rest, value)? {
                        return Err(ArgsError::UnknownFlag { command: command.to_string(), flag: format!("--{rest}") });
                    }
                    i += 2;
                }
            } else {
                self.positionals.push(token);
                i += 1;
            }
        }
        Ok(self.positionals)
    }
}

fn parse_contract_overrides(tail: &[String], command: &str) -> Result<(String, ContractOverrideArgs), ArgsError> {
    let mut symbol = "SPY".to_string();
    let mut overrides = ContractOverrideArgs::default();
    FlagCursor::new(tail).run(command, |name, value| {
        match name {
            "symbol" => symbol = value.to_string(),
            "strike" => overrides.strike = Some(parse_f64("--strike", value)?),
            "vol" => overrides.vol = Some(parse_f64("--vol", value)?),
            "days" => overrides.days = Some(parse_f64("--days", value)?),
            "current-shares" => overrides.current_shares = Some(parse_f64("--current-shares", value)?),
            "contracts" => overrides.contracts = Some(parse_i64("--contracts", value)?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok((symbol.to_uppercase(), overrides))
}

pub fn parse_args(raw: &[String]) -> Result<ParsedArgs, ArgsError> {
    let mut config: Option<PathBuf> = None;
    let mut rest: Vec<String> = Vec::with_capacity(raw.len());

    let mut i = 0;
    while i < raw.len() {
        if raw[i] == "--config" {
            let value = raw.get(i + 1).ok_or_else(|| ArgsError::MissingValueFor("--config".to_string()))?;
            config = Some(PathBuf::from(value));
            i += 2;
        } else if let Some(value) = raw[i].strip_prefix("--config=") {
            config = Some(PathBuf::from(value));
            i += 1;
        } else {
            rest.push(raw[i].clone());
            i += 1;
        }
    }

    let Some((command_name, tail)) = rest.split_first() else {
        return Err(ArgsError::MissingCommand);
    };

    let command = match command_name.as_str() {
        "build-core" => Command::BuildCore,
        "once" => {
            let (symbol, overrides) = parse_contract_overrides(tail, "once")?;
            Command::Once { symbol, overrides }
        }
        "loop" => {
            let mut interval = 15.0;
            let mut symbol = "SPY".to_string();
            let mut overrides = ContractOverrideArgs::default();
            FlagCursor::new(tail).run("loop", |name, value| {
                match name {
                    "symbol" => symbol = value.to_string(),
                    "strike" => overrides.strike = Some(parse_f64("--strike", value)?),
                    "vol" => overrides.vol = Some(parse_f64("--vol", value)?),
                    "days" => overrides.days = Some(parse_f64("--days", value)?),
                    "current-shares" => overrides.current_shares = Some(parse_f64("--current-shares", value)?),
                    "contracts" => overrides.contracts = Some(parse_i64("--contracts", value)?),
                    "interval" => interval = parse_f64("--interval", value)?,
                    _ => return Ok(false),
                }
                Ok(true)
            })?;
            Command::Loop { symbol: symbol.to_uppercase(), overrides, interval }
        }
        "replay" => {
            let positionals = FlagCursor::new(tail).run("replay", |_, _| Ok(false))?;
            let decision_id = positionals
                .first()
                .ok_or_else(|| ArgsError::MissingPositional { command: "replay".to_string(), name: "decision_id".to_string() })?;
            Command::Replay { decision_id: decision_id.to_string() }
        }
        "recent" => {
            let mut limit = 20i64;
            let mut symbol: Option<String> = None;
            FlagCursor::new(tail).run("recent", |name, value| {
                match name {
                    "limit" => limit = parse_i64("--limit", value)?,
                    "symbol" => symbol = if value.is_empty() { None } else { Some(value.to_uppercase()) },
                    _ => return Ok(false),
                }
                Ok(true)
            })?;
            Command::Recent { limit, symbol }
        }
        "self-test" => {
            let mut symbol = "SPY".to_string();
            FlagCursor::new(tail).run("self-test", |name, value| {
                match name {
                    "symbol" => symbol = value.to_string(),
                    _ => return Ok(false),
                }
                Ok(true)
            })?;
            Command::SelfTest { symbol: symbol.to_uppercase() }
        }
        "serve" => Command::Serve,
        "mcp" => Command::Mcp,
        other => return Err(ArgsError::UnknownCommand(other.to_string())),
    };

    Ok(ParsedArgs { config, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_command_is_an_error() {
        assert_eq!(parse_args(&args(&[])), Err(ArgsError::MissingCommand));
    }

    #[test]
    fn unknown_command_is_reported_by_name() {
        assert_eq!(parse_args(&args(&["bogus"])), Err(ArgsError::UnknownCommand("bogus".to_string())));
    }

    #[test]
    fn build_core_takes_no_arguments() {
        let parsed = parse_args(&args(&["build-core"])).unwrap();
        assert_eq!(parsed.command, Command::BuildCore);
        assert_eq!(parsed.config, None);
    }

    #[test]
    fn config_flag_before_the_command_is_recognized() {
        let parsed = parse_args(&args(&["--config", "my.json", "build-core"])).unwrap();
        assert_eq!(parsed.config, Some(PathBuf::from("my.json")));
    }

    #[test]
    fn config_flag_with_equals_form_is_recognized() {
        let parsed = parse_args(&args(&["--config=my.json", "once"])).unwrap();
        assert_eq!(parsed.config, Some(PathBuf::from("my.json")));
    }

    #[test]
    fn once_defaults_to_spy_with_no_overrides() {
        let parsed = parse_args(&args(&["once"])).unwrap();
        assert_eq!(parsed.command, Command::Once { symbol: "SPY".to_string(), overrides: ContractOverrideArgs::default() });
    }

    #[test]
    fn once_symbol_is_uppercased() {
        let parsed = parse_args(&args(&["once", "--symbol", "qqq"])).unwrap();
        assert_eq!(parsed.command, Command::Once { symbol: "QQQ".to_string(), overrides: ContractOverrideArgs::default() });
    }

    #[test]
    fn once_collects_all_overrides() {
        let parsed = parse_args(&args(&[
            "once", "--strike", "150", "--vol", "0.3", "--days", "10", "--current-shares", "-5", "--contracts", "2",
        ]))
        .unwrap();
        let expected = ContractOverrideArgs {
            strike: Some(150.0),
            vol: Some(0.3),
            days: Some(10.0),
            current_shares: Some(-5.0),
            contracts: Some(2),
        };
        assert_eq!(parsed.command, Command::Once { symbol: "SPY".to_string(), overrides: expected });
    }

    #[test]
    fn once_rejects_a_non_numeric_strike() {
        let result = parse_args(&args(&["once", "--strike", "abc"]));
        assert_eq!(result, Err(ArgsError::InvalidNumber { flag: "--strike".to_string(), value: "abc".to_string() }));
    }

    #[test]
    fn once_rejects_an_unknown_flag() {
        let result = parse_args(&args(&["once", "--bogus", "1"]));
        assert_eq!(result, Err(ArgsError::UnknownFlag { command: "once".to_string(), flag: "--bogus".to_string() }));
    }

    #[test]
    fn loop_defaults_interval_to_fifteen_seconds() {
        let parsed = parse_args(&args(&["loop"])).unwrap();
        assert_eq!(
            parsed.command,
            Command::Loop { symbol: "SPY".to_string(), overrides: ContractOverrideArgs::default(), interval: 15.0 }
        );
    }

    #[test]
    fn loop_interval_can_be_overridden() {
        let parsed = parse_args(&args(&["loop", "--interval", "5"])).unwrap();
        assert_eq!(
            parsed.command,
            Command::Loop { symbol: "SPY".to_string(), overrides: ContractOverrideArgs::default(), interval: 5.0 }
        );
    }

    #[test]
    fn replay_requires_a_decision_id_positional() {
        let result = parse_args(&args(&["replay"]));
        assert_eq!(result, Err(ArgsError::MissingPositional { command: "replay".to_string(), name: "decision_id".to_string() }));
    }

    #[test]
    fn replay_captures_the_decision_id() {
        let parsed = parse_args(&args(&["replay", "abc-123"])).unwrap();
        assert_eq!(parsed.command, Command::Replay { decision_id: "abc-123".to_string() });
    }

    #[test]
    fn recent_defaults_to_limit_twenty_and_no_symbol_filter() {
        let parsed = parse_args(&args(&["recent"])).unwrap();
        assert_eq!(parsed.command, Command::Recent { limit: 20, symbol: None });
    }

    #[test]
    fn recent_symbol_filter_is_uppercased() {
        let parsed = parse_args(&args(&["recent", "--symbol", "qqq", "--limit", "5"])).unwrap();
        assert_eq!(parsed.command, Command::Recent { limit: 5, symbol: Some("QQQ".to_string()) });
    }

    #[test]
    fn recent_empty_symbol_is_no_filter_matching_python_or_none() {
        let parsed = parse_args(&args(&["recent", "--symbol", ""])).unwrap();
        assert_eq!(parsed.command, Command::Recent { limit: 20, symbol: None });
    }

    #[test]
    fn self_test_defaults_to_spy() {
        let parsed = parse_args(&args(&["self-test"])).unwrap();
        assert_eq!(parsed.command, Command::SelfTest { symbol: "SPY".to_string() });
    }

    #[test]
    fn serve_and_mcp_parse_but_are_handled_as_not_yet_ported_by_the_caller() {
        assert_eq!(parse_args(&args(&["serve"])).unwrap().command, Command::Serve);
        assert_eq!(parse_args(&args(&["mcp"])).unwrap().command, Command::Mcp);
    }

    #[test]
    fn missing_value_for_a_flag_is_reported() {
        let result = parse_args(&args(&["once", "--strike"]));
        assert_eq!(result, Err(ArgsError::MissingValueFor("--strike".to_string())));
    }
}
