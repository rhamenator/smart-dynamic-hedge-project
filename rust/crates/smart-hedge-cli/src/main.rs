mod args;
mod commands;
mod error;

use args::{parse_args, Command};
use error::CliError;

fn run(raw_args: &[String]) -> Result<i32, CliError> {
    let parsed = parse_args(raw_args)?;
    match parsed.command {
        Command::BuildCore => commands::cmd_build_core(parsed.config),
        Command::Once { symbol, overrides, model } => commands::cmd_once(parsed.config, &symbol, overrides, model),
        Command::Loop { symbol, overrides, interval, model } => commands::cmd_loop(parsed.config, &symbol, overrides, interval, model),
        Command::Replay { decision_id } => commands::cmd_replay(parsed.config, &decision_id),
        Command::Recent { limit, symbol } => commands::cmd_recent(parsed.config, limit, symbol.as_deref()),
        Command::SelfTest { symbol } => commands::cmd_self_test(parsed.config, &symbol),
        Command::Serve { host, port } => commands::cmd_serve(parsed.config, host, port),
        Command::Mcp => commands::cmd_mcp(parsed.config),
        Command::GuardDemo { symbol, overrides, intelligence_binary, guard_binary } => {
            commands::cmd_guard_demo(parsed.config, &symbol, overrides, intelligence_binary, guard_binary)
        }
        Command::Portfolio { symbols } => commands::cmd_portfolio(parsed.config, symbols),
        Command::Backtest { symbol, days, start } => commands::cmd_backtest(parsed.config, &symbol, days, start),
    }
}

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    match run(&raw_args) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    }
}
