use std::io::{self, BufRead, Write};

use smart_hedge_engine::SmartHedgeEngine;

use crate::protocol::handle_line;

/// Port of `mcp_server.main`: reads newline-delimited JSON-RPC 2.0
/// messages from stdin and writes responses to stdout, one line each —
/// the MCP stdio transport. Stdio is the least-exposed default and works
/// with local MCP clients (matches `python/smart_hedge/mcp_server.py`'s
/// own comment); a network transport is out of scope here, same as
/// Python's.
pub fn run_stdio(engine: &SmartHedgeEngine) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if let Some(response) = handle_line(engine, &line) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
