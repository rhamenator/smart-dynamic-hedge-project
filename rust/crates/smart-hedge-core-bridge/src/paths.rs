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
}
