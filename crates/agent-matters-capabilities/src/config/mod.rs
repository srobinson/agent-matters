//! Filesystem loaders for the three MVP config surfaces. Pure schemas and
//! path conventions live in `agent-matters-core::config`; this module
//! performs the I/O and converts parse errors into actionable diagnostics.

mod loader;

pub use loader::{
    ConfigError, load_markers, load_runtime_defaults, load_runtime_settings, load_user_config,
    load_user_config_from_state_dir,
};
