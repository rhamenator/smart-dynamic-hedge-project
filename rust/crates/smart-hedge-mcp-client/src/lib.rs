//! A generic, dependency-free MCP stdio JSON-RPC 2.0 client: spawns a
//! server binary as a child process, writes one newline-delimited request
//! per call, and reads exactly one newline-delimited response — the
//! client-side counterpart to the hand-rolled stdio *servers* every
//! repository in this system already implements
//! (`smart_hedge_mcp::mcp`/`trade_guard_core::mcp`/
//! `market_intelligence_mcp_transport::mcp`), which is what makes this
//! client's protocol assumptions safe: every server it talks to processes
//! exactly one request per line and never sends an unsolicited
//! notification, so a strict request-then-read-one-line pairing is
//! correct, not merely convenient.
//!
//! No MCP SDK dependency — matching every other MCP implementation in
//! this three-repository system.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

#[derive(Debug)]
pub enum ClientError {
    Spawn(std::io::Error),
    Io(std::io::Error),
    Parse(String),
    /// A JSON-RPC protocol-level error (`response.error`) — an unknown
    /// method, malformed request, etc. Distinct from `Tool`, which is a
    /// tool-level failure reported through the normal `isError` result
    /// shape.
    Protocol { code: i64, message: String },
    /// The tool ran but reported failure (`isError: true`); `String` is
    /// the tool's own error text.
    Tool(String),
    /// The child process closed its stdout (exited or crashed) before
    /// answering a request.
    UnexpectedEof,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Spawn(e) => write!(f, "failed to spawn MCP server process: {e}"),
            ClientError::Io(e) => write!(f, "I/O error talking to MCP server process: {e}"),
            ClientError::Parse(e) => write!(f, "failed to parse MCP server response as JSON: {e}"),
            ClientError::Protocol { code, message } => write!(f, "MCP protocol error {code}: {message}"),
            ClientError::Tool(message) => write!(f, "MCP tool reported failure: {message}"),
            ClientError::UnexpectedEof => write!(f, "MCP server process closed its output before responding"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        ClientError::Io(e)
    }
}

pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpClient {
    /// Spawns `binary` with `args`, wires up piped stdin/stdout (stderr is
    /// inherited so a spawned server's diagnostic output is visible to
    /// this process's own stderr rather than silently discarded), and
    /// sends the MCP `initialize` handshake.
    pub fn spawn(binary: &Path, args: &[&str]) -> Result<Self, ClientError> {
        let mut child = Command::new(binary)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(ClientError::Spawn)?;
        let stdin = child.stdin.take().expect("spawned with Stdio::piped() stdin");
        let stdout = BufReader::new(child.stdout.take().expect("spawned with Stdio::piped() stdout"));
        let mut client = McpClient { child, stdin, stdout, next_id: 1 };
        client.call("initialize", json!({"protocolVersion": "2024-11-05"}))?;
        Ok(client)
    }

    fn next_request_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Sends one JSON-RPC request and reads exactly one response line.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_request_id();
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let line = serde_json::to_string(&request).expect("Value serialization is infallible");
        writeln!(self.stdin, "{line}")?;
        self.stdin.flush()?;

        let mut response_line = String::new();
        let bytes_read = self.stdout.read_line(&mut response_line)?;
        if bytes_read == 0 {
            return Err(ClientError::UnexpectedEof);
        }
        let response: Value = serde_json::from_str(response_line.trim()).map_err(|e| ClientError::Parse(e.to_string()))?;

        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = error.get("message").and_then(Value::as_str).unwrap_or("").to_string();
            return Err(ClientError::Protocol { code, message });
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Calls `tools/call` for `name`, returning the tool's text content on
    /// success. A tool-level failure (`isError: true`) becomes
    /// `Err(ClientError::Tool(text))`, matching the convention every
    /// server in this system's own `mcp.rs` uses for the same
    /// distinction on the server side.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String, ClientError> {
        let result = self.call("tools/call", json!({"name": name, "arguments": arguments}))?;
        let text = result
            .get("content")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);
        if is_error {
            Err(ClientError::Tool(text))
        } else {
            Ok(text)
        }
    }

    pub fn tools_list(&mut self) -> Result<Value, ClientError> {
        self.call("tools/list", Value::Null)
    }
}

impl Drop for McpClient {
    /// A hard kill, not a graceful stdin-close-then-wait: every server
    /// this client talks to is a local, stateless-per-invocation demo
    /// process whose durable state (SQLite audit stores) is flushed
    /// synchronously on each write, so there is nothing a graceful
    /// shutdown would protect that a kill risks losing. Simpler and more
    /// robust than depending on the child noticing EOF and exiting
    /// promptly, which a hung or misbehaving child could stall on.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses this very repository's own `smart-hedge` binary as the "MCP
    /// server under test" — it already implements a real, working `mcp`
    /// subcommand (`smart_hedge_mcp`), so this exercises the client
    /// against a real process without depending on either sibling
    /// repository being built, keeping this crate's own test suite
    /// self-contained.
    fn smart_hedge_binary() -> std::path::PathBuf {
        // target/debug/deps/<test-binary> -> target/debug/smart-hedge(.exe)
        let mut path = std::env::current_exe().unwrap();
        path.pop(); // deps/
        path.pop(); // debug/
        let name = if cfg!(windows) { "smart-hedge.exe" } else { "smart-hedge" };
        path.push(name);
        path
    }

    fn scratch_config() -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-mcp-client-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, "{}").unwrap();
        config_path
    }

    #[test]
    fn spawn_and_health_round_trip_against_a_real_process() {
        let binary = smart_hedge_binary();
        if !binary.exists() {
            eprintln!("skipping: {binary:?} not built (run `cargo build` first)");
            return;
        }
        let config = scratch_config();
        let config_str = config.to_string_lossy().to_string();
        let mut client = McpClient::spawn(&binary, &["--config", &config_str, "mcp"]).unwrap();
        let text = client.call_tool("health", json!({})).unwrap();
        assert!(text.contains("broker_order_endpoint_present"));
    }

    #[test]
    fn tools_list_returns_a_nonempty_array_against_a_real_process() {
        let binary = smart_hedge_binary();
        if !binary.exists() {
            eprintln!("skipping: {binary:?} not built (run `cargo build` first)");
            return;
        }
        let config = scratch_config();
        let config_str = config.to_string_lossy().to_string();
        let mut client = McpClient::spawn(&binary, &["--config", &config_str, "mcp"]).unwrap();
        let result = client.tools_list().unwrap();
        assert!(result["tools"].as_array().unwrap().len() > 3);
    }

    #[test]
    fn calling_an_unknown_tool_is_a_tool_error_not_a_panic() {
        let binary = smart_hedge_binary();
        if !binary.exists() {
            eprintln!("skipping: {binary:?} not built (run `cargo build` first)");
            return;
        }
        let config = scratch_config();
        let config_str = config.to_string_lossy().to_string();
        let mut client = McpClient::spawn(&binary, &["--config", &config_str, "mcp"]).unwrap();
        let result = client.call_tool("definitely-not-a-real-tool", json!({}));
        assert!(matches!(result, Err(ClientError::Tool(_))));
    }

    #[test]
    fn spawning_a_nonexistent_binary_is_a_spawn_error() {
        let bogus = std::env::temp_dir().join("this-binary-does-not-exist-12345.exe");
        let result = McpClient::spawn(&bogus, &["mcp"]);
        assert!(matches!(result, Err(ClientError::Spawn(_))));
    }
}
