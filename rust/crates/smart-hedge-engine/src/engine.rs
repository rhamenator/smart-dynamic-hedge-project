use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use smart_hedge_config::LoadedConfig;
use smart_hedge_core_bridge::resolve_binary;
use smart_hedge_data::MarketDataProvider;
use smart_hedge_model_advisor::{Advisor, HeuristicAdvisor};
use smart_hedge_models::{new_unique_id, TimestampUtc};
use smart_hedge_policy::{evaluate_policy, POLICY_VERSION};
use smart_hedge_store::DecisionStore;

use crate::contract::{resolve_contract, resolved_strike, ContractOverrides};
use crate::error::EngineError;
use crate::factory::{build_advisor, build_provider};
use crate::hashing::{canonical_hash, file_hash};

pub const ENGINE_VERSION: &str = "smart-orchestrator-rust-v0.1.0";

/// Port of `engine.SmartHedgeEngine`.
pub struct SmartHedgeEngine {
    loaded_config: LoadedConfig,
    project_root: PathBuf,
    cpp_source: PathBuf,
    provider: Box<dyn MarketDataProvider>,
    advisor: Box<dyn Advisor>,
    store: DecisionStore,
}

impl SmartHedgeEngine {
    pub fn new(loaded_config: LoadedConfig, project_root: PathBuf, cpp_source: PathBuf) -> Result<Self, EngineError> {
        let provider = build_provider(&loaded_config)?;
        let advisor = build_advisor(&loaded_config)?;
        Self::with_components(loaded_config, project_root, cpp_source, provider, advisor)
    }

    /// Constructs an engine with explicit provider/adviser implementations
    /// rather than ones built from config — mirrors Python's
    /// `SmartHedgeEngine(config, provider=..., advisor=...)` constructor
    /// arguments, and is how tests inject a deliberately-failing adviser
    /// stub to exercise the fallback path (SDH-LLR-057).
    pub fn with_components(
        loaded_config: LoadedConfig,
        project_root: PathBuf,
        cpp_source: PathBuf,
        provider: Box<dyn MarketDataProvider>,
        advisor: Box<dyn Advisor>,
    ) -> Result<Self, EngineError> {
        let db_path =
            smart_hedge_config::resolve_project_path(&loaded_config.config_dir, &loaded_config.config.storage.sqlite_path);
        let store = DecisionStore::new(&db_path)?;
        Ok(SmartHedgeEngine { loaded_config, project_root, cpp_source, provider, advisor, store })
    }

    /// Port of `SmartHedgeEngine.recommendation`. `now` is an explicit
    /// parameter (Python reads the clock internally); tests use
    /// `recommendation_at` directly for deterministic expiry-date behavior,
    /// `recommendation` is the convenience wrapper using the real clock.
    pub fn recommendation(&self, symbol: &str, overrides: &ContractOverrides) -> Result<Value, EngineError> {
        self.recommendation_at(symbol, overrides, TimestampUtc::now())
    }

    pub fn recommendation_at(
        &self,
        symbol: &str,
        overrides: &ContractOverrides,
        now: TimestampUtc,
    ) -> Result<Value, EngineError> {
        let normalized = symbol.to_uppercase();
        let snapshot = self.provider.snapshot(&normalized)?;
        let midpoint = snapshot.quote.midpoint();

        let contract = resolve_contract(&self.loaded_config.config, &normalized, overrides, midpoint, now)?;
        let features = smart_hedge_features::build_features(&snapshot, &self.loaded_config.config.features);
        let strike = resolved_strike(&contract);
        let core = smart_hedge_core_bridge::run_core(
            &self.loaded_config,
            &self.project_root,
            &self.cpp_source,
            &contract,
            midpoint,
            strike,
        )?;

        let mut fallback_reason = String::new();
        let assessment = match self.advisor.assess(&snapshot, &features, &core, &contract) {
            Ok(a) => a,
            Err(e) => {
                if !self.loaded_config.config.model.fallback_to_heuristic {
                    return Err(EngineError::AdvisorFailedAndFallbackDisabled(e));
                }
                fallback_reason = e.to_string();
                let mut fallback_assessment = HeuristicAdvisor
                    .assess(&snapshot, &features, &core, &contract)
                    .expect("HeuristicAdvisor::assess is infallible");
                fallback_assessment.fallback_reason = fallback_reason.clone();
                fallback_assessment
            }
        };

        let policy = evaluate_policy(&self.loaded_config.config, &snapshot, &features, &core, &assessment);
        let decision_id = new_unique_id();
        let created_at = now.to_iso_string();

        let contract_value = serde_json::to_value(&contract).expect("ContractConfig serialization is infallible");
        let core_value = serde_json::to_value(&core).expect("CoreResponse serialization is infallible");
        let snapshot_value = serde_json::to_value(&snapshot).expect("MarketSnapshot serialization is infallible");
        let features_value = serde_json::to_value(&features).expect("FeatureSet serialization is infallible");
        let assessment_value = serde_json::to_value(&assessment).expect("ModelAssessment serialization is infallible");
        let policy_value = serde_json::to_value(&policy).expect("PolicyDecision serialization is infallible");

        let core_binary = resolve_binary(&self.loaded_config, &self.project_root);
        let input_hash = canonical_hash(&json!({
            "contract": contract_value,
            "snapshot": snapshot_value,
            "features": features_value,
            "core": core_value,
        }));
        let model_output_hash = canonical_hash(&assessment_value);

        let audit = json!({
            "engine_version": ENGINE_VERSION,
            "policy_version": POLICY_VERSION,
            "input_hash": input_hash,
            "model_output_hash": model_output_hash,
            "core_binary_path": core_binary.to_string_lossy(),
            "core_binary_sha256": file_hash(&core_binary),
            "fallback_used": !fallback_reason.is_empty(),
            "fallback_reason": fallback_reason,
            "secrets_sent_to_model": false,
            "broker_order_endpoint_present": false,
        });

        let mut payload = json!({
            "decision_id": decision_id,
            "created_at": created_at,
            "mode": "paper",
            "symbol": normalized,
            "contract": contract_value,
            "snapshot": snapshot_value,
            "features": features_value,
            "deterministic_core": core_value,
            "model_assessment": assessment_value,
            "policy": policy_value,
            "audit": audit,
        });

        let content_hash = self.store.append(&payload)?;
        payload["audit"]["decision_store_content_hash"] = Value::String(content_hash);
        Ok(payload)
    }

    /// Port of `SmartHedgeEngine.replay`. Verifies: SDH-LLR-135.
    pub fn replay(&self, decision_id: &str) -> Result<Value, EngineError> {
        let mut payload =
            self.store.get(decision_id)?.ok_or_else(|| EngineError::DecisionNotFound(decision_id.to_string()))?;
        let root = payload
            .as_object_mut()
            .expect("stored decision payload root is always an object");
        let audit = root.entry("audit").or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Value::Object(audit_map) = audit {
            audit_map.insert(
                "replay_mode".to_string(),
                Value::String("stored_inputs_and_outputs_no_network".to_string()),
            );
        }
        Ok(payload)
    }

    pub fn recent(&self, limit: i64, symbol: Option<&str>) -> Result<Vec<Value>, EngineError> {
        Ok(self.store.recent(limit, symbol)?)
    }

    /// Port of `SmartHedgeEngine.health`. Verifies: SDH-LLR-136.
    pub fn health(&self) -> Value {
        json!({
            "status": "ok",
            "mode": "paper",
            "engine_version": ENGINE_VERSION,
            "provider": self.provider.name(),
            "advisor": self.advisor.name(),
            "database": self.store.path().to_string_lossy(),
            "broker_order_endpoint_present": false,
        })
    }

    pub fn store_path(&self) -> &Path {
        self.store.path()
    }
}
