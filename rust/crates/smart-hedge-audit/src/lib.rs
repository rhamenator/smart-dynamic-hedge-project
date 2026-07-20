//! Repo-wide static check: no source file outside an explicit allowlist
//! constructs (or even names) an order-placement request. Verifies
//! `SDH-LLR-080`'s previously-open structural gap — "if an order endpoint
//! were ever added elsewhere, nothing here would automatically flip
//! `broker_order_endpoint_present` to `true`." This crate is that
//! automatic flip: a `cargo test` failure the moment such code appears,
//! rather than a promise nobody re-checks.
//!
//! Scope: this workspace's Rust source, which is (as of this pass) the
//! only actively-growing implementation — Python is scheduled for
//! cutover, and the C++ core has no networking capability at all
//! (`SDH-LLR-100`: no third-party includes, so no HTTP library is even
//! reachable). `python/` and `cpp/` were manually re-checked for the same
//! patterns as part of writing this crate, but are not covered by an
//! automated, repeatable check the way this crate covers `rust/`.

use std::fs;
use std::path::{Path, PathBuf};

/// Substrings that would only plausibly appear in order-placement code —
/// naming an order tool, or a broker's real (non-data) trading endpoint.
/// Checked case-insensitively.
const FORBIDDEN_SUBSTRINGS: &[&str] = &["place_order", "submit_order", "cancel_order", "/v2/orders", "orders/place"];

/// HTTP verbs capable of mutating remote state. `ureq::post` is allowed
/// in exactly one file (the OpenAI Responses API call, which never talks
/// to a broker); `put`/`patch`/`delete` are never used anywhere in this
/// codebase.
const MUTATING_VERBS: &[&str] = &["ureq::put(", "ureq::patch(", "ureq::delete("];

const ALLOWED_POST_FILE_SUFFIX: &str = "smart-hedge-model-advisor/src/openai.rs";

#[derive(Debug)]
pub struct Violation {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
    pub reason: String,
}

/// Scans one file's *production* code (everything before its `mod tests`
/// block, if any) for forbidden patterns. This codebase's consistent
/// convention is exactly one `mod tests { ... }` block per file, always
/// near the end, always containing every test that file defines — so
/// stopping at the first such line is a reliable (if approximate)
/// production/test boundary without needing a real Rust parser. A test
/// asserting an order tool *doesn't* exist is expected to mention its
/// name; that's not a violation.
pub fn scan_file(path: &Path) -> Vec<Violation> {
    let Ok(text) = fs::read_to_string(path) else { return vec![] };
    let mut violations = Vec::new();
    let normalized_path = path.to_string_lossy().replace('\\', "/");
    let is_allowed_post_file = normalized_path.ends_with(ALLOWED_POST_FILE_SUFFIX);

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("mod tests") || trimmed.starts_with("pub mod tests") {
            break;
        }

        let lower = line.to_ascii_lowercase();
        for needle in FORBIDDEN_SUBSTRINGS {
            if lower.contains(needle) {
                violations.push(Violation {
                    path: path.to_path_buf(),
                    line_number: index + 1,
                    line: line.to_string(),
                    reason: format!("contains forbidden order-related substring {needle:?}"),
                });
            }
        }
        if !is_allowed_post_file && line.contains("ureq::post(") {
            violations.push(Violation {
                path: path.to_path_buf(),
                line_number: index + 1,
                line: line.to_string(),
                reason: "ureq::post( used outside the one allowed file (OpenAI Responses API)".to_string(),
            });
        }
        for verb in MUTATING_VERBS {
            if line.contains(verb) {
                violations.push(Violation {
                    path: path.to_path_buf(),
                    line_number: index + 1,
                    line: line.to_string(),
                    reason: format!("mutating HTTP verb {verb} is never expected anywhere in this codebase"),
                });
            }
        }
    }
    violations
}

/// Recursively collects every `.rs` file under `root`, skipping this
/// crate's own source (which legitimately contains every forbidden
/// substring, as string literals to search for) and any `target/` build
/// output directory.
pub fn collect_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == "smart-hedge-audit" {
                continue;
            }
            out.extend(collect_rust_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_rust_dir() -> PathBuf {
        // rust/crates/smart-hedge-audit -> rust/ is 2 levels up.
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
    }

    /// SDH-LLR-080: no production Rust source in this workspace constructs
    /// (or even names) an order-placement request. This is a repo-wide
    /// structural guarantee, not a single function's behavior — the
    /// closest thing to a runtime test this property can have.
    #[test]
    fn no_production_rust_source_names_or_constructs_an_order_placement_request() {
        let files = collect_rust_files(&repo_rust_dir());
        assert!(files.len() > 50, "sanity check: expected to find many .rs files, found {}", files.len());

        let mut all_violations = Vec::new();
        for file in &files {
            all_violations.extend(scan_file(file));
        }

        assert!(
            all_violations.is_empty(),
            "found {} order-placement-shaped code path(s):\n{}",
            all_violations.len(),
            all_violations
                .iter()
                .map(|v| format!("  {}:{}: {} | {}", v.path.display(), v.line_number, v.reason, v.line.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Proves the checker itself actually detects violations rather than
    /// being vacuously true — writes a fixture file containing exactly
    /// the kind of code this check exists to catch, and confirms it's
    /// flagged.
    #[test]
    fn the_checker_itself_flags_a_deliberately_planted_violation() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-audit-selftest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.rs");
        std::fs::write(&path, "pub fn place_order(qty: f64) {}\n").unwrap();

        let violations = scan_file(&path);
        assert!(!violations.is_empty(), "checker failed to flag a planted place_order fixture");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_checker_ignores_a_planted_mention_inside_a_test_module() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-audit-selftest-testmod-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.rs");
        std::fs::write(&path, "pub fn real_code() {}\n\nmod tests {\n    fn asserts_no_place_order() {}\n}\n").unwrap();

        let violations = scan_file(&path);
        assert!(violations.is_empty(), "checker should not flag mentions inside a test module");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_checker_flags_a_mutating_http_verb_outside_the_allowed_file() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-audit-selftest-verb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.rs");
        std::fs::write(&path, "let r = ureq::put(\"http://example.com\");\n").unwrap();

        let violations = scan_file(&path);
        assert!(!violations.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_checker_allows_post_specifically_in_the_openai_file() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-audit-selftest-openai-{}", std::process::id()));
        let nested = dir.join("smart-hedge-model-advisor").join("src");
        std::fs::create_dir_all(&nested).unwrap();
        let path = nested.join("openai.rs");
        std::fs::write(&path, "let r = ureq::post(\"https://api.openai.com/v1/responses\");\n").unwrap();

        let violations = scan_file(&path);
        assert!(violations.is_empty(), "expected no violations for the allowed OpenAI POST file, got {violations:?}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
