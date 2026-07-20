//! A large-scale randomized "workout" test: many synthetic scenarios
//! (random symbols — including unconfigured ones, random contract
//! overrides at and beyond realistic boundaries, and a flaky adviser that
//! fails unpredictably) run through the full engine pipeline, asserting
//! the safety invariants that must hold no matter how strange the input
//! is: no panic, always `mode: "paper"`, always
//! `live_execution_allowed: false`, always replayable with a valid hash.
//! This is fake data, not live credentials — the point is to build
//! confidence *before* ever pointing this at a real feed, the same spirit
//! as a pre-flight soak test.
//!
//! Skips (passes trivially) under the same conditions as
//! `integration_tests` — no prebuilt C++ toolchain available.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use smart_hedge_config::EnvOverrides;
use smart_hedge_model_advisor::{Advisor, AdvisorError, HeuristicAdvisor};
use smart_hedge_models::{CoreResponse, FeatureSet, MarketSnapshot, ModelAssessment};

use crate::contract::ContractOverrides;
use crate::engine::SmartHedgeEngine;
use crate::error::EngineError;

/// Each iteration is a *real* subprocess spawn of the C++ core plus two
/// real SQLite connections (append + replay), so this isn't free — on
/// this project's development machine (noted elsewhere as having
/// antivirus real-time-scanning overhead on both process spawns and file
/// I/O — see `.cargo/config.toml`), 25 iterations takes roughly a
/// minute. That's a deliberate trade-off: a real workout needs real
/// subprocess/file-system round trips, not a mocked-out fast path: a
/// machine without that overhead can raise this freely.
const ITERATIONS: u32 = 25;

/// Same xorshift64 construction already used elsewhere in this workspace
/// (e.g. `smart_hedge_models::time_util`'s fuzz-smoke test) — a fixed
/// seed, so a failure is exactly reproducible, not a one-time flake.
struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 // [0, 1)
    }

    fn next_f64(&mut self, low: f64, high: f64) -> f64 {
        low + self.next_unit() * (high - low)
    }

    fn next_i64(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() % ((high - low + 1) as u64)) as i64
    }

    fn choice<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() as usize) % items.len()]
    }

    fn bool_with_probability(&mut self, p: f64) -> bool {
        self.next_unit() < p
    }
}

/// An adviser that fails a configurable fraction of calls (deterministic
/// per-call pseudo-randomness derived from an atomic counter, so the
/// whole chaos run is still reproducible from one fixed outer seed) and
/// otherwise delegates to the real `HeuristicAdvisor` — exercises the
/// fallback path repeatedly under randomized conditions, not just the
/// hand-picked always-succeeds/always-fails cases in `integration_tests`.
struct FlakyAdvisor {
    failure_probability: f64,
    counter: AtomicU64,
}

impl Advisor for FlakyAdvisor {
    fn assess(
        &self,
        snapshot: &MarketSnapshot,
        features: &FeatureSet,
        core: &CoreResponse,
        contract: &smart_hedge_config::ContractConfig,
    ) -> Result<ModelAssessment, AdvisorError> {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut local_rng = XorShift64(n.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1));
        if local_rng.next_unit() < self.failure_probability {
            return Err(AdvisorError("flaky adviser: simulated failure".to_string()));
        }
        HeuristicAdvisor.assess(snapshot, features, core, contract)
    }

    fn name(&self) -> &'static str {
        "FlakyAdvisor"
    }
}

fn find_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

#[test]
fn chaos_workout_many_randomized_scenarios_never_panic_and_stay_paper_only() {
    let root = find_repo_root();
    let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");
    if !cpp_source.exists() {
        eprintln!("skipping: {} not found", cpp_source.display());
        return;
    }
    if smart_hedge_core_bridge::which_tool("cmake").is_none()
        && smart_hedge_core_bridge::which_tool("g++").is_none()
        && smart_hedge_core_bridge::which_tool("clang++").is_none()
    {
        eprintln!("skipping: no cmake/g++/clang++ toolchain found on PATH");
        return;
    }

    let dir = std::env::temp_dir().join(format!("smart-hedge-engine-chaos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("config.json");
    let sqlite_path = dir.join("decisions.sqlite3");
    std::fs::write(
        &config_path,
        format!(
            r#"{{
                "model": {{"fallback_to_heuristic": true}},
                "storage": {{"sqlite_path": "{}"}},
                "contracts": {{
                    "SPY": {{"option_type": "put", "exercise_style": "american", "strike": 100.0, "implied_volatility": 0.20, "days_to_expiry": 30.0}},
                    "QQQ": {{"option_type": "call", "exercise_style": "european", "strike": "ATM", "implied_volatility": 0.25, "days_to_expiry": 45.0}},
                    "IWM": {{"option_type": "put", "exercise_style": "american", "strike": 50.0, "implied_volatility": 0.35, "days_to_expiry": 7.0}}
                }}
            }}"#,
            sqlite_path.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .unwrap();

    let loaded = smart_hedge_config::load_config(Some(&config_path), &EnvOverrides::default(), &root).unwrap();
    let provider = smart_hedge_data::SyntheticProvider::new(loaded.clone());
    let advisor = FlakyAdvisor { failure_probability: 0.25, counter: AtomicU64::new(0) };
    let engine =
        SmartHedgeEngine::with_components(loaded, root, cpp_source, Box::new(provider), Box::new(advisor)).unwrap();

    // Deliberately includes a symbol with no `contracts` entry at all —
    // every iteration that picks it must hit `UnknownSymbol`, never a panic.
    let symbols = ["SPY", "QQQ", "IWM", "ZZZZ_NOT_CONFIGURED"];
    let mut rng = XorShift64(0x2545_F491_4F6C_DD1D);
    let mut ok_count = 0usize;
    let mut expected_err_count = 0usize;

    for i in 0..ITERATIONS {
        let symbol = *rng.choice(&symbols);
        let overrides = ContractOverrides {
            strike: rng.bool_with_probability(0.6).then(|| rng.next_f64(0.01, 10_000.0)),
            implied_volatility: rng.bool_with_probability(0.6).then(|| rng.next_f64(0.001, 5.0)),
            days_to_expiry: rng.bool_with_probability(0.6).then(|| rng.next_f64(0.0, 3650.0)),
            current_shares: rng.bool_with_probability(0.5).then(|| rng.next_f64(-100_000.0, 100_000.0)),
            contracts: rng.bool_with_probability(0.5).then(|| rng.next_i64(0, 10_000)),
        };

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine.recommendation(symbol, &overrides)));
        let result = match outcome {
            Ok(r) => r,
            Err(_) => panic!("iteration {i} (symbol={symbol:?}, overrides={overrides:?}) PANICKED instead of returning a Result"),
        };

        match result {
            Ok(decision) => {
                ok_count += 1;
                assert_eq!(decision["mode"], "paper", "iteration {i}");
                assert_eq!(decision["policy"]["live_execution_allowed"], false, "iteration {i}");
                assert_eq!(decision["audit"]["broker_order_endpoint_present"], false, "iteration {i}");
                assert_eq!(decision["audit"]["secrets_sent_to_model"], false, "iteration {i}");
                let decision_id = decision["decision_id"].as_str().expect("decision_id present");
                let replayed = engine.replay(decision_id).unwrap_or_else(|e| panic!("iteration {i}: replay failed: {e}"));
                assert_eq!(
                    replayed["audit"]["stored_content_hash_valid"], true,
                    "iteration {i}: tamper check failed on a decision that was never tampered with"
                );
            }
            Err(e) => {
                expected_err_count += 1;
                // Exhaustive on purpose: an error variant this match
                // doesn't list is treated as a failure, not silently
                // accepted — the point of a chaos test is to notice when
                // "weird input" starts taking an unexpected path, not
                // just "didn't crash".
                match e {
                    EngineError::UnknownSymbol(_) | EngineError::InvalidStrike(_) | EngineError::Core(_) => {}
                    other => panic!("iteration {i} (symbol={symbol:?}, overrides={overrides:?}): unexpected error variant: {other}"),
                }
            }
        }
    }

    assert!(ok_count > 0, "expected at least some of {ITERATIONS} iterations to succeed");
    assert!(expected_err_count > 0, "expected at least the unconfigured-symbol case to produce an error");
    eprintln!("chaos workout: {ITERATIONS} iterations, {ok_count} succeeded, {expected_err_count} failed with an expected error variant");

    std::fs::remove_dir_all(&dir).ok();
}
