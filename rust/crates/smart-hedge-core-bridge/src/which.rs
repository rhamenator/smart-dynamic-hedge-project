use std::path::PathBuf;

/// Minimal, dependency-free `shutil.which` equivalent: searches `PATH` for
/// an existing file named `name` (with a `.exe` suffix also tried on
/// Windows, since that's how `cmake`/`g++`/`clang++` are actually installed
/// there). Does not check the executable bit on Unix — a false positive
/// (an existing-but-not-executable file) would simply surface as a normal
/// spawn failure later, which is an acceptable trade-off against adding a
/// dependency or `unsafe` `libc` calls just for a permission-bit check.
pub fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(windows) {
            let with_exe = dir.join(format!("{name}.exe"));
            if with_exe.is_file() {
                return Some(with_exe);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_tool_known_to_exist_in_this_test_environment() {
        // `cargo` itself must be on PATH for this test to be running at all.
        assert!(which("cargo").is_some());
    }

    #[test]
    fn returns_none_for_a_name_that_should_never_exist() {
        assert!(which("definitely-not-a-real-executable-name-xyz").is_none());
    }
}
