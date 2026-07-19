//! Rust port of `python/smart_hedge/mcp_server.py`: an MCP (Model Context
//! Protocol) stdio server. The Python original uses the `mcp` package's
//! `FastMCP` framework; this hand-rolls the minimal JSON-RPC 2.0 subset
//! MCP's stdio transport actually needs (`initialize`, `ping`,
//! `tools/list`, `tools/call`, newline-delimited messages) — no HTTP, no
//! TLS, and no third-party dependency at all beyond `serde_json`, since
//! this is a much narrower surface than the dashboard's HTTP server or
//! the data providers' HTTPS clients.

pub mod protocol;
pub mod server;
pub mod tools;

pub use protocol::{call_tool, handle_line, tool_definitions};
pub use server::run_stdio;
pub use tools::PriceOptionArgs;
