use std::path::{Path, PathBuf};

use smart_hedge_config::{resolve_project_path, LoadedConfig};

/// Port of `smart_hedge.core_bridge.default_binary_path`: `.exe` suffix on
/// Windows, none elsewhere — mirrors `"smart_dynamic_hedge.exe" if
/// os.name == "nt" else "smart_dynamic_hedge"` exactly.
pub fn default_binary_path(project_root: &Path) -> PathBuf {
    let name = if cfg!(windows) { "smart_dynamic_hedge.exe" } else { "smart_dynamic_hedge" };
    project_root.join("build").join(name)
}

/// Port of `smart_hedge.core_bridge.resolve_binary`.
pub fn resolve_binary(loaded: &LoadedConfig, project_root: &Path) -> PathBuf {
    let raw = loaded.config.core.binary.trim();
    if raw.is_empty() {
        default_binary_path(project_root)
    } else {
        resolve_project_path(&loaded.config_dir, raw)
    }
}

/// The multi-config-generator fallback location Python checks when the
/// primary candidate doesn't exist on Windows (Visual Studio/Ninja
/// Multi-Config generators put Release binaries in a `Release/`
/// subdirectory instead of directly under `build/`).
pub fn windows_multi_config_fallback(project_root: &Path) -> PathBuf {
    project_root.join("build").join("Release").join("smart_dynamic_hedge.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_binary_path_has_platform_correct_suffix() {
        let path = default_binary_path(Path::new("/root"));
        if cfg!(windows) {
            assert_eq!(path, PathBuf::from("/root/build/smart_dynamic_hedge.exe"));
        } else {
            assert_eq!(path, PathBuf::from("/root/build/smart_dynamic_hedge"));
        }
    }

    #[test]
    fn windows_fallback_path_is_under_release_subdirectory() {
        let path = windows_multi_config_fallback(Path::new("/root"));
        assert_eq!(path, PathBuf::from("/root/build/Release/smart_dynamic_hedge.exe"));
    }

    fn loaded_config_with_core_binary(core_json: &str) -> LoadedConfig {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-core-bridge-resolve-binary-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"core": {core_json}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    /// SDH-LLR-031: an empty `core.binary` (the default) falls back to the
    /// platform-default path under `project_root/build/`, not the
    /// config-relative resolution an explicit value would get.
    #[test]
    fn resolve_binary_with_no_override_falls_back_to_the_platform_default() {
        let loaded = loaded_config_with_core_binary(r#"{"binary": ""}"#);
        let resolved = resolve_binary(&loaded, Path::new("/some/other/project/root"));
        assert_eq!(resolved, default_binary_path(Path::new("/some/other/project/root")));
    }

    /// SDH-LLR-031: an explicit, absolute `core.binary` is used verbatim —
    /// it must win over the platform default, and it must not be
    /// re-resolved against `project_root` (only `resolve_project_path`'s
    /// own config-relative rule applies, via `config_dir`).
    #[test]
    fn resolve_binary_with_an_explicit_absolute_path_uses_it_verbatim() {
        // A leading `/` alone isn't "absolute" by Rust's/Windows's
        // definition on Windows (no drive letter) — matching this file's
        // own `default_binary_path_has_platform_correct_suffix` test,
        // pick a genuinely platform-absolute literal.
        let absolute = if cfg!(windows) { r"C:\opt\custom\smart_dynamic_hedge" } else { "/opt/custom/smart_dynamic_hedge" };
        let loaded = loaded_config_with_core_binary(&format!(r#"{{"binary": "{}"}}"#, absolute.replace('\\', "\\\\")));
        let resolved = resolve_binary(&loaded, Path::new("/some/other/project/root"));
        assert_eq!(resolved, PathBuf::from(absolute));
    }

    /// SDH-LLR-031/SDH-LLR-024: an explicit *relative* `core.binary`
    /// resolves against the config file's own directory (`config_dir`),
    /// not `project_root` — the two can legitimately differ (e.g. a
    /// config file kept outside the repository).
    #[test]
    fn resolve_binary_with_an_explicit_relative_path_resolves_against_config_dir_not_project_root() {
        let loaded = loaded_config_with_core_binary(r#"{"binary": "bin/smart_dynamic_hedge"}"#);
        let resolved = resolve_binary(&loaded, Path::new("/unrelated/project/root"));
        assert_eq!(resolved, loaded.config_dir.join("bin").join("smart_dynamic_hedge"));
        assert!(!resolved.starts_with("/unrelated/project/root"));
    }

    /// A whitespace-only `core.binary` is treated the same as empty
    /// (`.trim()` in `resolve_binary`), not as a literal path made of
    /// spaces.
    #[test]
    fn resolve_binary_with_a_whitespace_only_override_falls_back_to_the_platform_default() {
        let loaded = loaded_config_with_core_binary(r#"{"binary": "   "}"#);
        let resolved = resolve_binary(&loaded, Path::new("/root"));
        assert_eq!(resolved, default_binary_path(Path::new("/root")));
    }
}
