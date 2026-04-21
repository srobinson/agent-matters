//! Pure runtime build contracts and generated path conventions.

pub mod build;
pub mod fingerprint;

pub use build::{
    BUILD_PLAN_SCHEMA_VERSION, BUILDS_DIR_NAME, RUNTIME_HOME_DIR_NAME, RUNTIMES_DIR_NAME,
    runtime_build_dir, runtime_home_dir, runtime_pointer_dir,
};
pub use fingerprint::{FINGERPRINT_ALGORITHM, FingerprintBuilder};
