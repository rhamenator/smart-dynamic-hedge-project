use std::path::{Component, Path, PathBuf};

/// Cross-platform home-directory lookup without a `dirs`/`home` crate
/// dependency: `USERPROFILE` on Windows, `HOME` elsewhere. Good enough for
/// expanding a leading `~` in a user-supplied config path; this is not a
/// general-purpose home-directory API.
fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from)
}

/// Minimal `~`/`~/...` expansion, matching Python's `Path(raw).expanduser()`
/// for the cases this codebase actually produces (a bare `~` or a
/// `~/relative/path`). Does not support `~otheruser` — Python's
/// `expanduser` does on Unix, but nothing in this codebase relies on that.
pub fn expand_user(raw: &str) -> PathBuf {
    if raw == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(raw));
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

/// Collapses `.`/`..` components purely lexically — no filesystem access,
/// no symlink resolution, and no requirement that the path exist. This is a
/// deliberate, documented deviation from Python's `Path.resolve()` (which
/// does touch the filesystem to resolve symlinks it can find) since the
/// paths resolved here are user-configured file locations, not something
/// where symlink transparency matters, and requiring filesystem access to
/// resolve a config path is its own footgun (fails confusingly on a path
/// that doesn't exist yet, e.g. `storage.sqlite_path` before its first run).
pub fn lexically_normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// Port of `smart_hedge.config.resolve_project_path`: expands `~`, returns
/// absolute paths unchanged, and otherwise joins the relative path onto
/// `config_dir` and normalizes lexically.
pub fn resolve_project_path(config_dir: &Path, raw: &str) -> PathBuf {
    let expanded = expand_user(raw);
    if expanded.is_absolute() {
        return expanded;
    }
    lexically_normalize(&config_dir.join(expanded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_path_is_returned_unchanged() {
        let abs = if cfg!(windows) { r"C:\abs\path" } else { "/abs/path" };
        let result = resolve_project_path(Path::new("/config/dir"), abs);
        assert_eq!(result, PathBuf::from(abs));
    }

    #[test]
    fn relative_path_joins_onto_config_dir() {
        let result = resolve_project_path(Path::new("/config/dir"), "data/evidence.json");
        assert_eq!(result, PathBuf::from("/config/dir/data/evidence.json"));
    }

    #[test]
    fn parent_dir_components_are_collapsed() {
        let result = resolve_project_path(Path::new("/config/dir"), "../sibling/file.json");
        assert_eq!(result, PathBuf::from("/config/sibling/file.json"));
    }

    #[test]
    fn current_dir_components_are_dropped() {
        let result = resolve_project_path(Path::new("/config/dir"), "./file.json");
        assert_eq!(result, PathBuf::from("/config/dir/file.json"));
    }

    #[test]
    fn lexically_normalize_does_not_touch_the_filesystem_for_nonexistent_paths() {
        // Must not panic or error even though this path does not exist.
        let result = lexically_normalize(Path::new("/nonexistent/../also-nonexistent/./x"));
        assert_eq!(result, PathBuf::from("/also-nonexistent/x"));
    }
}
