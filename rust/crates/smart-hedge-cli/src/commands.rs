use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

use serde_json::json;
use smart_hedge_config::{EnvOverrides, LoadedConfig};
use smart_hedge_engine::{ContractOverrides, SmartHedgeEngine};
use smart_hedge_guard_client::{build_trade_intent, GuardClient, TradeIntentParams, TradeSide};
use smart_hedge_intelligence_client::IntelligenceClient;

use crate::args::ContractOverrideArgs;
use crate::error::CliError;

/// Rust binaries have no equivalent of Python's `Path(__file__).parents[2]`
/// — the compiled executable can be copied anywhere. `smart-hedge.py`'s
/// CLI is always run from the repository root in practice; this matches
/// that by using the process's current directory, as documented on
/// `smart_hedge_config::load_config`.
pub fn project_root() -> Result<PathBuf, CliError> {
    Ok(std::env::current_dir()?)
}

pub fn cpp_source_path(project_root: &Path) -> PathBuf {
    project_root.join("cpp").join("smart_dynamic_hedge.cpp")
}

/// Port of `cli.py`'s `selected = path or os.getenv("SMART_HEDGE_CONFIG")`.
pub fn resolve_config_path(explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit.or_else(|| std::env::var("SMART_HEDGE_CONFIG").ok().map(PathBuf::from))
}

pub fn load_config(config_path: Option<PathBuf>, project_root: &Path) -> Result<LoadedConfig, CliError> {
    Ok(smart_hedge_config::load_config(config_path.as_deref(), &EnvOverrides::from_process_env(), project_root)?)
}

fn to_engine_overrides(o: ContractOverrideArgs) -> ContractOverrides {
    ContractOverrides {
        strike: o.strike,
        implied_volatility: o.vol,
        days_to_expiry: o.days,
        current_shares: o.current_shares,
        contracts: o.contracts,
    }
}

/// Builds a `SmartHedgeEngine`, optionally routing to a named model from
/// `config.model.models` (the `MODEL_URI` router) instead of the legacy
/// `model.kind`/`model.name` single adviser `SmartHedgeEngine::new` uses
/// by default. `model_name: None` is the exact previous behavior — this
/// function is a strict superset, not a replacement.
fn build_engine(loaded: LoadedConfig, root: PathBuf, cpp_source: PathBuf, model_name: Option<&str>) -> Result<SmartHedgeEngine, CliError> {
    match model_name {
        None => Ok(SmartHedgeEngine::new(loaded, root, cpp_source)?),
        Some(name) => {
            let provider = smart_hedge_engine::build_provider(&loaded)?;
            let advisor = smart_hedge_engine::build_advisor_by_name(&loaded, name)?;
            Ok(SmartHedgeEngine::with_components(loaded, root, cpp_source, provider, advisor)?)
        }
    }
}

pub fn cmd_build_core(config_path: Option<PathBuf>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let binary = smart_hedge_core_bridge::build_core(&loaded, &root, &cpp_source)?;
    println!("{}", binary.display());
    Ok(0)
}

pub fn cmd_once(config_path: Option<PathBuf>, symbol: &str, overrides: ContractOverrideArgs, model: Option<String>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = build_engine(loaded, root, cpp_source, model.as_deref())?;
    let decision = engine.recommendation(symbol, &to_engine_overrides(overrides))?;
    println!("{}", serde_json::to_string_pretty(&decision).expect("decision is always serializable"));
    Ok(0)
}

pub fn cmd_loop(
    config_path: Option<PathBuf>,
    symbol: &str,
    overrides: ContractOverrideArgs,
    interval: f64,
    model: Option<String>,
) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = build_engine(loaded, root, cpp_source, model.as_deref())?;
    let engine_overrides = to_engine_overrides(overrides);
    let sleep_for = Duration::from_secs_f64(interval.max(1.0));

    // Matches Python's `except KeyboardInterrupt: return 0` in spirit: this
    // loop runs until the process receives Ctrl+C, at which point the OS
    // terminates it directly (Rust has no built-in signal handling without
    // a dependency, and adding one is out of scope for this pass) rather
    // than unwinding back through this function.
    loop {
        let decision = engine.recommendation(symbol, &engine_overrides)?;
        println!("{}", format_loop_line(&decision));
        std::thread::sleep(sleep_for);
    }
}

fn format_loop_line(decision: &serde_json::Value) -> String {
    let p = &decision["policy"];
    let q = &decision["snapshot"]["quote"];
    let m = &decision["model_assessment"];
    let bid = q["bid"].as_f64().unwrap_or(f64::NAN);
    let ask = q["ask"].as_f64().unwrap_or(f64::NAN);
    format!(
        "{} {} mid={:.4} regime={} action={} preview={:.3} blockers={}",
        decision["created_at"].as_str().unwrap_or(""),
        decision["symbol"].as_str().unwrap_or(""),
        (bid + ask) / 2.0,
        m["regime"].as_str().unwrap_or(""),
        p["action"].as_str().unwrap_or(""),
        p["paper_trade_preview_shares"].as_f64().unwrap_or(f64::NAN),
        p["blocking_reasons"],
    )
}

pub fn cmd_replay(config_path: Option<PathBuf>, decision_id: &str) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let value = engine.replay(decision_id)?;
    println!("{}", serde_json::to_string_pretty(&value).expect("replayed decision is always serializable"));
    Ok(0)
}

pub fn cmd_recent(config_path: Option<PathBuf>, limit: i64, symbol: Option<&str>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let values = engine.recent(limit, symbol)?;
    println!("{}", serde_json::to_string_pretty(&values).expect("recent decisions are always serializable"));
    Ok(0)
}

pub fn cmd_self_test(config_path: Option<PathBuf>, symbol: &str) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);

    let binary = smart_hedge_core_bridge::ensure_core(&loaded, &root, &cpp_source)?;
    let status = ProcessCommand::new(&binary)
        .arg("--self-test")
        .status()
        .map_err(|e| CliError::SelfTestFailed(format!("failed to run {}: {e}", binary.display())))?;
    if !status.success() {
        return Err(CliError::SelfTestFailed(format!("{} --self-test exited with {status}", binary.display())));
    }

    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let value = engine.recommendation(symbol, &ContractOverrides::default())?;
    if value["mode"] != json!("paper") {
        return Err(CliError::SelfTestFailed("mode was not paper".to_string()));
    }
    if value["policy"]["live_execution_allowed"] != json!(false) {
        return Err(CliError::SelfTestFailed("live_execution_allowed was not false".to_string()));
    }
    if value["audit"]["broker_order_endpoint_present"] != json!(false) {
        return Err(CliError::SelfTestFailed("broker_order_endpoint_present was not false".to_string()));
    }

    let decision_id = value["decision_id"].as_str().expect("decision_id is always a string");
    let replay = engine.replay(decision_id)?;
    if replay["audit"]["stored_content_hash_valid"] != json!(true) {
        return Err(CliError::SelfTestFailed("stored_content_hash_valid was not true on replay".to_string()));
    }

    println!("rust integration self-test: PASS");
    Ok(0)
}

pub fn cmd_serve(config_path: Option<PathBuf>, host: Option<String>, port: Option<u16>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    smart_hedge_dashboard::serve(loaded, root, cpp_source, host.as_deref(), port)?;
    Ok(0)
}

pub fn cmd_mcp(config_path: Option<PathBuf>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    smart_hedge_mcp::run_stdio(&engine)?;
    Ok(0)
}

/// Same env-var-fallback pattern `resolve_config_path` uses for
/// `SMART_HEDGE_CONFIG`, applied to the two sibling MCP server binaries
/// this command needs but this repository does not build.
fn resolve_sibling_binary(explicit: Option<PathBuf>, env_var: &str) -> Option<PathBuf> {
    explicit.or_else(|| std::env::var(env_var).ok().map(PathBuf::from))
}

/// The one fixture this demo actually has data for —
/// `market-intelligence-mcp`'s own demo binary hardcodes the same
/// `us-house-periodic-transaction-report` source and
/// `fixture-political-disclosure-1` record, reviewed under a
/// `research-only` `SourceUseDecision`. This command reuses that exact
/// fixture rather than inventing a different one, so what it proves is
/// "the real `build-evidence-bundle` tool, called from a different
/// process, over real subprocess/stdio boundaries" — not new fixture
/// content.
const DEMO_SOURCE_ID: &str = "us-house-periodic-transaction-report";
const DEMO_SOURCE_RECORD_ID: &str = "fixture-political-disclosure-1";

fn demo_source_use_decision() -> serde_json::Value {
    json!({
        "source-id": DEMO_SOURCE_ID,
        "policy-version": "2026-07-19.1",
        "public-access": true,
        "automated-retrieval": "legal-review-required",
        "commercial-use": "prohibited-or-unclear",
        "trading-use": "research-only",
        "redistribution": "prohibited-or-unclear",
        "attribution-required": true,
        "reviewed-at": "1970-01-01T00:00:00Z",
        "reviewed-by": "operator",
        "reason-codes": ["statutory-commercial-use-restriction"],
    })
}

/// Outcome of one `run_guard_cycle`, shared by the one-shot `guard-demo`
/// command and the continuous `autonomous` loop below. A policy
/// rejection from `trade-guard-mcp` (insufficient buying power, evidence
/// ineligible) is `Rejected` — a legitimate, expected outcome, not a
/// `CliError`. Only a hard failure (a sibling process failed to spawn or
/// spoke a broken protocol) surfaces as `run_guard_cycle`'s `Err`.
enum GuardCycleOutcome {
    NoTradeProposed { action: String, approved: bool },
    Rejected(String),
    Filled(serde_json::Value),
}

struct GuardCycleReport {
    decision: serde_json::Value,
    evidence_bundle: Option<serde_json::Value>,
    outcome: GuardCycleOutcome,
}

/// Runs one recommendation → evidence → guard-authorization cycle. See
/// `GuardCycleOutcome`'s doc comment for what counts as this function's
/// `Err` versus a legitimate `Ok(GuardCycleOutcome::Rejected(_))`.
fn run_guard_cycle(
    engine: &SmartHedgeEngine,
    symbol: &str,
    overrides: &ContractOverrides,
    intelligence_binary: &Path,
    guard_binary: &Path,
) -> Result<GuardCycleReport, CliError> {
    let decision = engine.recommendation(symbol, overrides)?;

    let action = decision["policy"]["action"].as_str().unwrap_or("").to_string();
    let approved = decision["policy"]["paper_preview_approved"].as_bool().unwrap_or(false);
    let preview_shares = decision["policy"]["paper_trade_preview_shares"].as_f64().unwrap_or(0.0);
    if action != "paper_rebalance_preview" || !approved || preview_shares == 0.0 {
        return Ok(GuardCycleReport { decision, evidence_bundle: None, outcome: GuardCycleOutcome::NoTradeProposed { action, approved } });
    }

    let mut intelligence = IntelligenceClient::spawn(intelligence_binary)
        .map_err(|e| CliError::GuardDemo(format!("failed to start market-intelligence-mcp ({}): {e}", intelligence_binary.display())))?;
    intelligence
        .ingest_source_records(DEMO_SOURCE_ID)
        .map_err(|e| CliError::GuardDemo(format!("market-intelligence-mcp ingest-source-records failed: {e}")))?;
    let history = intelligence
        .get_source_record_history(DEMO_SOURCE_RECORD_ID)
        .map_err(|e| CliError::GuardDemo(format!("market-intelligence-mcp get-source-record-history failed: {e}")))?;
    let record = history
        .as_array()
        .and_then(|arr| arr.first())
        .cloned()
        .ok_or_else(|| CliError::GuardDemo("market-intelligence-mcp returned no history for the demo fixture record".to_string()))?;

    let decision_id = decision["decision_id"].as_str().unwrap_or("unknown-decision").to_string();
    let created_at = decision["created_at"].as_str().unwrap_or("1970-01-01T00:00:00Z").to_string();
    let evidence_bundle_id = format!("bundle-{decision_id}");
    let evidence_bundle = intelligence
        .build_evidence_bundle(
            &evidence_bundle_id,
            vec![json!({"record": record, "decision": demo_source_use_decision()})],
            "research",
            &created_at,
        )
        .map_err(|e| CliError::GuardDemo(format!("market-intelligence-mcp build-evidence-bundle failed: {e}")))?;

    // Deliberately a fresh timestamp, not `created_at` from the
    // recommendation above: `TradeIntent.decision-time` means "when was
    // the decision to submit this trade finalized," which is now — after
    // evidence was gathered — not backdated to when the underlying
    // deterministic recommendation happened to be computed. Reusing
    // `created_at` here would make `check-evidence-eligibility` correctly
    // reject the intent with `evidence-bundle-created-after-decision`,
    // since the evidence bundle's own `created-at` (also "now", set by
    // `market-intelligence-mcp`) would then postdate it.
    let decision_time = smart_hedge_models::TimestampUtc::now().to_iso_string();
    let confidence = decision["model_assessment"]["confidence"].as_f64().unwrap_or(0.0);
    let instrument_id = format!("us-equity-{}", symbol.to_lowercase());
    let side = if preview_shares > 0.0 { TradeSide::Buy } else { TradeSide::Sell };
    let intent = build_trade_intent(&TradeIntentParams {
        intent_id: &decision_id,
        strategy_id: "smart-dynamic-hedge",
        decision_id: &decision_id,
        account_alias: "paper-default",
        instrument_id: &instrument_id,
        symbol,
        side,
        quantity: preview_shares.abs(),
        decision_time: &decision_time,
        confidence,
        idempotency_key: &decision_id,
        evidence_bundle_id: Some(&evidence_bundle_id),
    });

    let mut guard = GuardClient::spawn(guard_binary)
        .map_err(|e| CliError::GuardDemo(format!("failed to start trade-guard-mcp ({}): {e}", guard_binary.display())))?;
    let outcome = match guard.authorize_and_submit_paper_order(intent, Some(evidence_bundle.clone())) {
        Ok(value) => GuardCycleOutcome::Filled(value),
        // A rejection (insufficient buying power, evidence ineligible,
        // etc.) is a legitimate, informative outcome, not a hard failure.
        Err(smart_hedge_mcp_client::ClientError::Tool(text)) => GuardCycleOutcome::Rejected(text),
        Err(e) => return Err(CliError::GuardDemo(format!("trade-guard-mcp authorize-and-submit-paper-order failed: {e}"))),
    };

    Ok(GuardCycleReport { decision, evidence_bundle: Some(evidence_bundle), outcome })
}

/// Port of Phase 4 in `06-implementation-order-and-acceptance.md`'s
/// minimal slice: run the real deterministic recommendation, fetch real
/// evidence from `market-intelligence-mcp`, build a typed `TradeIntent`
/// from the recommendation's own paper-trade preview, and submit it to
/// `trade-guard-mcp`'s real paper simulator — the first time this
/// repository has actually exercised the intended
/// `TradeIntent -> trade-guard-mcp` cross-repository flow end to end,
/// rather than only documenting it. See
/// `smart_hedge_guard_client`'s module doc comment for why calling a
/// tool named `authorize-and-submit-paper-order` from this repository
/// does not conflict with `smart_hedge_audit`'s no-order-placement
/// invariant.
pub fn cmd_guard_demo(
    config_path: Option<PathBuf>,
    symbol: &str,
    overrides: ContractOverrideArgs,
    intelligence_binary: Option<PathBuf>,
    guard_binary: Option<PathBuf>,
) -> Result<i32, CliError> {
    let intelligence_binary = resolve_sibling_binary(intelligence_binary, "MARKET_INTELLIGENCE_MCP_BIN").ok_or_else(|| {
        CliError::GuardDemo(
            "guard-demo needs market-intelligence-mcp's server binary: pass --intelligence-binary or set MARKET_INTELLIGENCE_MCP_BIN"
                .to_string(),
        )
    })?;
    let guard_binary = resolve_sibling_binary(guard_binary, "TRADE_GUARD_MCP_BIN").ok_or_else(|| {
        CliError::GuardDemo("guard-demo needs trade-guard-mcp's server binary: pass --guard-binary or set TRADE_GUARD_MCP_BIN".to_string())
    })?;

    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let engine = SmartHedgeEngine::new(loaded, root, cpp_source)?;
    let engine_overrides = to_engine_overrides(overrides);

    let report = run_guard_cycle(&engine, symbol, &engine_overrides, &intelligence_binary, &guard_binary)?;
    println!("=== smart-dynamic-hedge recommendation ===");
    println!("{}", serde_json::to_string_pretty(&report.decision).expect("decision is always serializable"));

    match report.outcome {
        GuardCycleOutcome::NoTradeProposed { action, approved } => {
            println!("\n=== no trade proposed (action={action:?}, approved={approved}) — trade-guard-mcp not called ===");
        }
        GuardCycleOutcome::Rejected(text) => {
            print_evidence_bundle(&report.evidence_bundle);
            println!("\n=== trade-guard-mcp paper-order result ===");
            println!("{text}");
        }
        GuardCycleOutcome::Filled(value) => {
            print_evidence_bundle(&report.evidence_bundle);
            println!("\n=== trade-guard-mcp paper-order result ===");
            println!("{}", serde_json::to_string_pretty(&value).expect("guard result is always serializable"));
        }
    }
    Ok(0)
}

fn print_evidence_bundle(evidence_bundle: &Option<serde_json::Value>) {
    if let Some(bundle) = evidence_bundle {
        println!("\n=== market-intelligence-mcp evidence bundle ===");
        println!("{}", serde_json::to_string_pretty(bundle).expect("evidence bundle is always serializable"));
    }
}

/// The "autonomous (non-manual) paper operation" gap `docs/ROADMAP.md`
/// Phase 4 named: `guard-demo` proves the full recommendation → evidence
/// → guard-authorization chain works once, but a human has to re-invoke
/// it every cycle. `autonomous` runs that same chain on a timer without a
/// human re-invoking anything each iteration — still paper-only (nothing
/// in this repository's dependency graph can place a live order; see
/// `smart_hedge_audit`), and still explicitly started by a human once,
/// not scheduled or self-initiating.
///
/// Safety gates on top of everything `evaluate_policy` and
/// `trade-guard-mcp`'s own paper simulator already enforce (including the
/// `STALE_QUOTE` check that already pauses trading on stale data — see
/// `rust/README.md` "Point-in-time backtester" for the bug that check
/// used to have):
///  - **stop file**: checked at the top of every iteration; if present,
///    the loop halts cleanly. The kill switch — an operator drops a file,
///    the loop notices within one interval, no signal handling needed.
///  - **`--max-iterations`**: an optional hard cap, for bounded runs
///    (testing, a supervised session) instead of "forever" by default.
///  - **consecutive-error circuit breaker**: a *hard* error (a sibling
///    process failed to spawn, or spoke a broken protocol) halts the loop
///    after `max_consecutive_errors` in a row, instead of hammering a
///    broken dependency forever. A policy rejection is not an error here
///    — it is the guard doing its job — and resets the counter to zero,
///    the same as a fill or a "no trade proposed" cycle.
#[allow(clippy::too_many_arguments)]
pub fn cmd_autonomous(
    config_path: Option<PathBuf>,
    symbol: &str,
    overrides: ContractOverrideArgs,
    interval: f64,
    model: Option<String>,
    intelligence_binary: Option<PathBuf>,
    guard_binary: Option<PathBuf>,
    max_iterations: Option<u32>,
    max_consecutive_errors: u32,
    stop_file: Option<PathBuf>,
) -> Result<i32, CliError> {
    let intelligence_binary = resolve_sibling_binary(intelligence_binary, "MARKET_INTELLIGENCE_MCP_BIN").ok_or_else(|| {
        CliError::Autonomous(
            "autonomous needs market-intelligence-mcp's server binary: pass --intelligence-binary or set MARKET_INTELLIGENCE_MCP_BIN"
                .to_string(),
        )
    })?;
    let guard_binary = resolve_sibling_binary(guard_binary, "TRADE_GUARD_MCP_BIN").ok_or_else(|| {
        CliError::Autonomous("autonomous needs trade-guard-mcp's server binary: pass --guard-binary or set TRADE_GUARD_MCP_BIN".to_string())
    })?;

    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);
    let stop_file = stop_file.unwrap_or_else(|| root.join(".smart-hedge-stop"));
    let engine = build_engine(loaded, root, cpp_source, model.as_deref())?;
    let engine_overrides = to_engine_overrides(overrides);
    let sleep_for = Duration::from_secs_f64(interval.max(1.0));
    let max_consecutive_errors = max_consecutive_errors.max(1);

    let mut iteration: u32 = 0;
    let mut consecutive_errors: u32 = 0;
    loop {
        if stop_file.exists() {
            println!("stop file {} present -- halting after {iteration} iteration(s)", stop_file.display());
            return Ok(0);
        }
        if max_iterations.is_some_and(|max| iteration >= max) {
            println!("reached --max-iterations={} -- halting", max_iterations.expect("is_some_and checked above"));
            return Ok(0);
        }

        match run_guard_cycle(&engine, symbol, &engine_overrides, &intelligence_binary, &guard_binary) {
            Ok(report) => {
                consecutive_errors = 0;
                println!("{}", autonomous_cycle_line(iteration, symbol, &report));
            }
            Err(e) => {
                consecutive_errors += 1;
                eprintln!("iteration {iteration}: error ({consecutive_errors}/{max_consecutive_errors} consecutive): {e}");
                if consecutive_errors >= max_consecutive_errors {
                    return Err(CliError::Autonomous(format!(
                        "halting after {consecutive_errors} consecutive errors (max_consecutive_errors={max_consecutive_errors}); last error: {e}"
                    )));
                }
            }
        }

        iteration += 1;
        std::thread::sleep(sleep_for);
    }
}

fn autonomous_cycle_line(iteration: u32, symbol: &str, report: &GuardCycleReport) -> String {
    let created_at = report.decision["created_at"].as_str().unwrap_or("");
    match &report.outcome {
        GuardCycleOutcome::NoTradeProposed { action, approved } => {
            format!("[{iteration}] {created_at} {symbol} no trade proposed (action={action}, approved={approved})")
        }
        GuardCycleOutcome::Rejected(text) => {
            format!("[{iteration}] {created_at} {symbol} rejected by trade-guard-mcp: {text}")
        }
        GuardCycleOutcome::Filled(value) => {
            let state = value["order"]["state"].as_str().unwrap_or("?");
            let quantity = value["order"]["filled-quantity"].as_str().unwrap_or("?");
            format!("[{iteration}] {created_at} {symbol} filled: state={state} quantity={quantity}")
        }
    }
}

/// Port of Phase 4's "C++ portfolio pricing/Greeks/hedging expansion":
/// runs the (unchanged) C++ core once per symbol and prints both the
/// per-position detail and the aggregated dollar-denominated portfolio
/// summary — see `smart_hedge_portfolio`'s module doc comment for why the
/// aggregates are dollar-denominated rather than raw share counts.
pub fn cmd_portfolio(config_path: Option<PathBuf>, symbols: Vec<String>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);

    let symbols = if symbols.is_empty() { loaded.config.contracts.keys().cloned().collect() } else { symbols };
    if symbols.is_empty() {
        return Err(CliError::Engine(smart_hedge_engine::EngineError::UnknownSymbol(
            "no symbols given and no contracts configured".to_string(),
        )));
    }

    let positions = smart_hedge_portfolio::build_portfolio(&loaded, &root, &cpp_source, &symbols)?;
    let summary = smart_hedge_portfolio::summarize(&positions);

    let output = json!({
        "positions": positions,
        "summary": summary,
    });
    println!("{}", serde_json::to_string_pretty(&output).expect("portfolio output is always serializable"));
    Ok(0)
}

/// Port of Phase 4's "point-in-time backtester": steps a deterministic
/// synthetic price path day by day through the same real pipeline `once`
/// uses, threading `current_shares` and `days_to_expiry` forward across
/// days. See `smart_hedge_backtest`'s module doc comment for exactly what
/// this proves (a real multi-day pipeline run) and doesn't (real
/// historical market data, which doesn't exist anywhere in this system).
pub fn cmd_backtest(config_path: Option<PathBuf>, symbol: &str, days: u32, start: Option<String>) -> Result<i32, CliError> {
    let root = project_root()?;
    let loaded = load_config(resolve_config_path(config_path), &root)?;
    let cpp_source = cpp_source_path(&root);

    let start = match start {
        Some(s) => smart_hedge_models::TimestampUtc::parse_flexible(&s)
            .ok_or_else(|| CliError::Backtest(format!("invalid --start timestamp: {s:?}")))?,
        None => smart_hedge_models::TimestampUtc::now(),
    };

    let config = smart_hedge_backtest::BacktestConfig { symbol: symbol.to_string(), num_days: days, start };
    let report = smart_hedge_backtest::run_backtest(&loaded, &root, &cpp_source, &config)?;
    println!("{}", serde_json::to_string_pretty(&report).expect("backtest report is always serializable"));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_loop_line_renders_the_expected_shape() {
        let decision = json!({
            "created_at": "2026-07-19T00:00:00Z",
            "symbol": "SPY",
            "snapshot": {"quote": {"bid": 99.0, "ask": 101.0}},
            "model_assessment": {"regime": "calm"},
            "policy": {"action": "hold", "paper_trade_preview_shares": 1.5, "blocking_reasons": []},
        });
        let line = format_loop_line(&decision);
        assert_eq!(line, "2026-07-19T00:00:00Z SPY mid=100.0000 regime=calm action=hold preview=1.500 blockers=[]");
    }

    #[test]
    fn resolve_config_path_prefers_the_explicit_flag_over_the_env_var() {
        // No env var set in this process by default; explicit always wins when present.
        let resolved = resolve_config_path(Some(PathBuf::from("explicit.json")));
        assert_eq!(resolved, Some(PathBuf::from("explicit.json")));
    }

    #[test]
    fn resolve_config_path_is_none_with_neither_flag_nor_env_var() {
        // SMART_HEDGE_CONFIG is not expected to be set in the test environment.
        if std::env::var("SMART_HEDGE_CONFIG").is_err() {
            assert_eq!(resolve_config_path(None), None);
        }
    }
}
