//! Rust port of `python/smart_hedge/config.py`. See `smart_hedge_models`
//! (the sibling crate) and `docs/ROADMAP.md` "Language and dependency
//! policy" for the migration this is part of.

pub mod defaults;
pub mod env_overrides;
pub mod error;
pub mod loader;
pub mod merge;
pub mod paths;
pub mod strike_spec;
pub mod types;

pub use defaults::default_config_json;
pub use env_overrides::EnvOverrides;
pub use error::ConfigError;
pub use loader::{load_config, LoadedConfig};
pub use merge::deep_merge;
pub use paths::resolve_project_path;
pub use strike_spec::StrikeSpec;
pub use types::{
    AlpacaConfig, Config, ContractConfig, CoreConfig, DashboardConfig, FeaturesConfig, FredConfig,
    ModelConfig, PolicyConfig, ProviderConfig, RssConfig, StorageConfig,
};
