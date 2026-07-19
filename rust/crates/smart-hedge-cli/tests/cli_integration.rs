//! End-to-end tests that shell out to the actual compiled `smart-hedge`
//! binary, exercising it the way a user would from a terminal. Skips
//! (passes trivially) when no prebuilt C++ core binary is available,
//! matching the pattern already used by `smart-hedge-core-bridge` and
//! `smart-hedge-engine`'s own integration tests — only these tests have
//! that dependency.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    // rust/crates/smart-hedge-cli -> repo root is 3 levels up.
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

/// Locates an already-built core binary without ever invoking cmake itself
/// — parallel test threads each shelling out to `cmake --build` against the
/// same `build/` directory would race. If nothing is prebuilt, tests skip.
fn prebuilt_core_binary(root: &Path) -> Option<PathBuf> {
    let direct = root.join("build").join(if cfg!(windows) { "smart_dynamic_hedge.exe" } else { "smart_dynamic_hedge" });
    if direct.is_file() {
        return Some(direct);
    }
    let windows_fallback = root.join("build").join("Release").join("smart_dynamic_hedge.exe");
    if windows_fallback.is_file() {
        return Some(windows_fallback);
    }
    None
}

struct Harness {
    root: PathBuf,
    core_binary: PathBuf,
    db_path: PathBuf,
}

fn harness_or_skip(name: &str) -> Option<Harness> {
    let root = repo_root();
    let Some(core_binary) = prebuilt_core_binary(&root) else {
        eprintln!("skipping {name}: no prebuilt core binary found under {}/build", root.display());
        return None;
    };
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("smart-hedge-cli-itest-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    Some(Harness { root, core_binary, db_path: dir.join("decisions.sqlite3") })
}

impl Harness {
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_smart-hedge"));
        cmd.args(args)
            .current_dir(&self.root)
            .env("SMART_HEDGE_CORE", &self.core_binary)
            .env("SMART_HEDGE_DB", &self.db_path)
            .env_remove("SMART_HEDGE_CONFIG");
        cmd
    }

    fn cleanup(&self) {
        if let Some(parent) = self.db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }
}

#[test]
fn no_command_exits_2_with_a_helpful_stderr_message() {
    let root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_smart-hedge")).current_dir(&root).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("a command is required"), "stderr was: {stderr}");
}

#[test]
fn unknown_command_exits_2() {
    let root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_smart-hedge")).arg("bogus").current_dir(&root).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown command: bogus"), "stderr was: {stderr}");
}

#[test]
fn serve_starts_a_real_http_server_and_answers_health() {
    let Some(h) = harness_or_skip("serve_starts_a_real_http_server_and_answers_health") else { return };

    let mut child = h
        .command(&["serve", "--host", "127.0.0.1", "--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // The server prints "...listening on http://host:port" (flushed
    // explicitly) as soon as it's bound and before it blocks accepting
    // connections — read that one line to learn the OS-assigned port
    // (`--port 0`) rather than guessing or sleeping.
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("listening on http://127.0.0.1:"), "unexpected first line: {line:?}");
    let port: u16 = line.trim().rsplit(':').next().unwrap().parse().expect("port should parse");

    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "response was: {response}");
    assert!(response.contains("\"broker_order_endpoint_present\":false"), "response was: {response}");

    let _ = child.kill();
    let _ = child.wait();
    h.cleanup();
}

#[test]
fn mcp_answers_initialize_and_tools_list_over_stdio() {
    let Some(h) = harness_or_skip("mcp_answers_initialize_and_tools_list_over_stdio") else { return };

    let mut child = h.command(&["mcp"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2024-11-05"}}}}"#).unwrap();
    stdin.flush().unwrap();
    let mut init_line = String::new();
    reader.read_line(&mut init_line).unwrap();
    let init_response: serde_json::Value = serde_json::from_str(&init_line).expect("stdout line should be JSON");
    assert_eq!(init_response["result"]["serverInfo"]["name"], "smart-dynamic-hedge");

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    stdin.flush().unwrap();
    let mut tools_line = String::new();
    reader.read_line(&mut tools_line).unwrap();
    let tools_response: serde_json::Value = serde_json::from_str(&tools_line).expect("stdout line should be JSON");
    let names: Vec<&str> =
        tools_response["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"health"));
    assert!(names.contains(&"get_market_recommendation"));
    assert!(!names.iter().any(|n| n.contains("order")));

    drop(stdin); // closes the child's stdin, which ends its read loop
    let _ = child.wait();
    h.cleanup();
}

#[test]
fn once_produces_a_paper_only_decision_as_pretty_json() {
    let Some(h) = harness_or_skip("once_produces_a_paper_only_decision_as_pretty_json") else { return };

    let output = h.command(&["once", "--symbol", "spy"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let decision: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(decision["symbol"], "SPY");
    assert_eq!(decision["mode"], "paper");
    assert_eq!(decision["policy"]["live_execution_allowed"], false);

    h.cleanup();
}

#[test]
fn recent_and_replay_see_a_decision_persisted_by_a_prior_process() {
    let Some(h) = harness_or_skip("recent_and_replay_see_a_decision_persisted_by_a_prior_process") else { return };

    let once_output = h.command(&["once", "--symbol", "SPY"]).output().unwrap();
    assert!(once_output.status.success(), "stderr: {}", String::from_utf8_lossy(&once_output.stderr));
    let decision: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&once_output.stdout)).unwrap();
    let decision_id = decision["decision_id"].as_str().unwrap().to_string();

    let recent_output = h.command(&["recent", "--symbol", "SPY"]).output().unwrap();
    assert!(recent_output.status.success());
    let recent: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&recent_output.stdout)).unwrap();
    assert!(recent.as_array().unwrap().iter().any(|d| d["decision_id"] == decision_id));

    let replay_output = h.command(&["replay", &decision_id]).output().unwrap();
    assert!(replay_output.status.success(), "stderr: {}", String::from_utf8_lossy(&replay_output.stderr));
    let replayed: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&replay_output.stdout)).unwrap();
    assert_eq!(replayed["decision_id"], decision_id);
    assert_eq!(replayed["audit"]["replay_mode"], "stored_inputs_and_outputs_no_network");

    h.cleanup();
}

#[test]
fn replay_of_an_unknown_decision_id_exits_2() {
    let Some(h) = harness_or_skip("replay_of_an_unknown_decision_id_exits_2") else { return };
    let output = h.command(&["replay", "does-not-exist"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("decision not found"), "stderr was: {stderr}");
    h.cleanup();
}

#[test]
fn self_test_passes_against_the_synthetic_heuristic_path() {
    let Some(h) = harness_or_skip("self_test_passes_against_the_synthetic_heuristic_path") else { return };
    let output = h.command(&["self-test"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("self-test: PASS"), "stdout was: {stdout}");
    h.cleanup();
}

#[test]
fn an_unrecognized_flag_exits_2_before_touching_the_network_or_store() {
    let root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_smart-hedge"))
        .args(["once", "--not-a-real-flag", "1"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
