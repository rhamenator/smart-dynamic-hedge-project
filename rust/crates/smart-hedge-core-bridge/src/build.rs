use std::path::Path;
use std::process::Command;
use std::time::Duration;

use smart_hedge_config::LoadedConfig;

use crate::error::CoreError;
use crate::paths::{default_binary_path, resolve_binary, windows_multi_config_fallback};
use crate::run_with_timeout::run_command_with_timeout;
use crate::which::which;

const CMAKE_TIMEOUT: Duration = Duration::from_secs(120);
const COMPILE_TIMEOUT: Duration = Duration::from_secs(180);

/// Port of `smart_hedge.core_bridge.build_core`. Prefers CMake; falls back
/// to a direct `g++`/`clang++` invocation exactly like Python does, and
/// checks the same Windows multi-config-generator fallback path.
pub fn build_core(loaded: &LoadedConfig, project_root: &Path, cpp_source: &Path) -> Result<std::path::PathBuf, CoreError> {
    let build_dir = project_root.join("build");
    std::fs::create_dir_all(&build_dir)?;

    if let Some(cmake) = which("cmake") {
        run_command_with_timeout(
            Command::new(&cmake)
                .arg("-S")
                .arg(project_root)
                .arg("-B")
                .arg(&build_dir)
                .arg("-DCMAKE_BUILD_TYPE=Release"),
            CMAKE_TIMEOUT,
        )?;
        run_command_with_timeout(
            Command::new(&cmake)
                .arg("--build")
                .arg(&build_dir)
                .arg("--config")
                .arg("Release")
                .arg("-j"),
            COMPILE_TIMEOUT,
        )?;
    } else {
        let compiler = which("g++").or_else(|| which("clang++")).ok_or(CoreError::NoToolchainFound)?;
        let output_path = default_binary_path(project_root);
        run_command_with_timeout(
            Command::new(&compiler)
                .args(["-std=c++17", "-O2", "-Wall", "-Wextra", "-Wpedantic"])
                .arg(cpp_source)
                .arg("-o")
                .arg(&output_path),
            COMPILE_TIMEOUT,
        )?;
    }

    let mut binary = resolve_binary(loaded, project_root);
    // Multi-config Windows generators place Release binaries in a subdirectory.
    if !binary.exists() && cfg!(windows) {
        let candidate = windows_multi_config_fallback(project_root);
        if candidate.exists() {
            binary = candidate;
        }
    }
    if !binary.exists() {
        return Err(CoreError::BuildSucceededButBinaryMissing(binary));
    }
    Ok(binary)
}

/// Port of `smart_hedge.core_bridge.ensure_core`.
pub fn ensure_core(
    loaded: &LoadedConfig,
    project_root: &Path,
    cpp_source: &Path,
) -> Result<std::path::PathBuf, CoreError> {
    let binary = resolve_binary(loaded, project_root);
    if binary.exists() {
        return Ok(binary);
    }
    if loaded.config.core.auto_build {
        build_core(loaded, project_root, cpp_source)
    } else {
        Err(CoreError::BinaryNotFound(binary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn loaded_config_with_auto_build(auto_build: bool) -> (LoadedConfig, std::path::PathBuf) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-core-bridge-ensure-core-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"core": {{"auto_build": {auto_build}}}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        (loaded, dir)
    }

    /// SDH-LLR-034: `auto_build: false` with no binary present reports
    /// `BinaryNotFound` — and must do so *without ever attempting a
    /// build*. Verified by pointing `cpp_source` at a path that doesn't
    /// exist and isn't buildable: if `ensure_core` ever regressed to
    /// calling `build_core` regardless of `auto_build`, the result would
    /// be a different `CoreError` variant (a toolchain/compile failure),
    /// not `BinaryNotFound` — this test would then fail on the exact
    /// variant, not just "some error occurred".
    #[test]
    fn auto_build_false_reports_binary_not_found_without_attempting_a_build() {
        let (loaded, dir) = loaded_config_with_auto_build(false);
        let missing_cpp_source = dir.join("does-not-exist.cpp");

        let result = ensure_core(&loaded, &dir, &missing_cpp_source);
        assert!(matches!(result, Err(CoreError::BinaryNotFound(_))), "expected BinaryNotFound, got {result:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The mirror case: `auto_build: true` (the default) with no binary
    /// present and no cpp source does attempt a build — and since there's
    /// nothing to build, it fails with something other than
    /// `BinaryNotFound`, confirming the two configurations really do take
    /// different code paths (not just different error text for the same
    /// outcome).
    #[test]
    fn auto_build_true_attempts_a_build_and_fails_differently_than_auto_build_false() {
        let (loaded, dir) = loaded_config_with_auto_build(true);
        let missing_cpp_source = dir.join("does-not-exist.cpp");

        let result = ensure_core(&loaded, &dir, &missing_cpp_source);
        assert!(
            !matches!(result, Err(CoreError::BinaryNotFound(_))),
            "auto_build:true should not short-circuit to BinaryNotFound like auto_build:false does, got {result:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
