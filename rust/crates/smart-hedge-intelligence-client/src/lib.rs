//! A typed client for `market-intelligence-mcp`'s stdio MCP server, built
//! on `smart_hedge_mcp_client::McpClient`. Read-only: every method here
//! maps to one of that repository's 11 read-only tools
//! (`market_intelligence_mcp_transport::tools`) — there is no
//! order-entry capability to wrap, because that repository has none.
//!
//! This crate does not know how to build `market-intelligence-mcp`'s
//! server binary; the caller supplies its path (typically via
//! `MARKET_INTELLIGENCE_MCP_BIN`, matching the `SMART_HEDGE_CORE`
//! env-var convention this repository already uses for the C++ core
//! binary path — see `smart-hedge-cli`'s `commands.rs`).

use std::path::Path;

use serde_json::{json, Value};
use smart_hedge_mcp_client::{ClientError, McpClient};

pub struct IntelligenceClient {
    inner: McpClient,
}

impl IntelligenceClient {
    /// Spawns `binary mcp` — the same subcommand
    /// `market-intelligence-server`'s own README documents.
    pub fn spawn(binary: &Path) -> Result<Self, ClientError> {
        let inner = McpClient::spawn(binary, &["mcp"])?;
        Ok(IntelligenceClient { inner })
    }

    fn call_json(&mut self, tool: &str, arguments: Value) -> Result<Value, ClientError> {
        let text = self.inner.call_tool(tool, arguments)?;
        serde_json::from_str(&text).map_err(|e| ClientError::Parse(e.to_string()))
    }

    pub fn health(&mut self) -> Result<Value, ClientError> {
        self.call_json("health", json!({}))
    }

    pub fn list_configured_sources(&mut self) -> Result<Vec<String>, ClientError> {
        let result = self.call_json("list-configured-sources", json!({}))?;
        Ok(result
            .get("source_ids")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default())
    }

    pub fn ingest_source_records(&mut self, source_id: &str) -> Result<Value, ClientError> {
        self.call_json("ingest-source-records", json!({"source_id": source_id}))
    }

    pub fn get_source_record_history(&mut self, source_record_id: &str) -> Result<Value, ClientError> {
        self.call_json("get-source-record-history", json!({"source_record_id": source_record_id}))
    }

    /// `items` is a list of `{"record": <SourceRecord>, "decision": <SourceUseDecision>}`
    /// objects — the caller is responsible for constructing well-typed
    /// records/decisions (or reusing ones read back from
    /// `get_source_record_history`); this client does not invent evidence
    /// content on its own.
    pub fn build_evidence_bundle(
        &mut self,
        evidence_bundle_id: &str,
        items: Vec<Value>,
        purpose: &str,
        decision_time: &str,
    ) -> Result<Value, ClientError> {
        self.call_json(
            "build-evidence-bundle",
            json!({
                "evidence_bundle_id": evidence_bundle_id,
                "items": items,
                "purpose": purpose,
                "decision_time": decision_time,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawning_a_nonexistent_binary_is_a_spawn_error() {
        let bogus = std::env::temp_dir().join("this-market-intelligence-binary-does-not-exist-12345.exe");
        let result = IntelligenceClient::spawn(&bogus);
        assert!(matches!(result, Err(ClientError::Spawn(_))));
    }
}
