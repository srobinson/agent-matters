//! Config schemas and filesystem path conventions for the runtime profile
//! system. This module is pure: it defines the TOML backed structs that
//! `agent-matters-capabilities::config` loads, plus the constant path
//! fragments and tilde helper that downstream code uses to locate those
//! files on disk. No I/O happens here.

pub mod paths;
pub mod schemas;

pub use paths::{
    MARKERS_FILE_NAME, REPO_DEFAULTS_DIR_NAME, RUNTIMES_FILE_NAME, USER_CONFIG_DIR_NAME,
    USER_CONFIG_FILE_NAME, expand_tilde,
};
pub use schemas::{Markers, RuntimeDefaults, RuntimeSettings, UserConfig};
