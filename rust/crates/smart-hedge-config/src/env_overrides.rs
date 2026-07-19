/// Explicit, injectable set of the environment-variable overrides Python's
/// `load_config` reads directly from `os.environ`. Reading real process
/// environment variables (`SMART_HEDGE_PROVIDER`, `SMART_HEDGE_MODEL_KIND`,
/// `OPENAI_MODEL`, `SMART_HEDGE_CORE`, `SMART_HEDGE_DB`) only happens in
/// `from_process_env`; the merge logic itself takes this plain struct, so
/// tests can exercise every combination without touching real environment
/// state — Rust 2024 makes `std::env::set_var`/`remove_var` `unsafe`, and
/// this workspace forbids `unsafe_code` outright, so that's not just nicer,
/// it's required.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvOverrides {
    pub provider_kind: Option<String>,
    pub model_kind: Option<String>,
    pub openai_model: Option<String>,
    pub core_binary: Option<String>,
    pub storage_sqlite_path: Option<String>,
}

impl EnvOverrides {
    pub fn from_process_env() -> Self {
        EnvOverrides {
            provider_kind: std::env::var("SMART_HEDGE_PROVIDER").ok(),
            model_kind: std::env::var("SMART_HEDGE_MODEL_KIND").ok(),
            openai_model: std::env::var("OPENAI_MODEL").ok(),
            core_binary: std::env::var("SMART_HEDGE_CORE").ok(),
            storage_sqlite_path: std::env::var("SMART_HEDGE_DB").ok(),
        }
    }
}
