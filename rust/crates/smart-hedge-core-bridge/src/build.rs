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
