//! Pure runtime build contracts and generated path conventions.

pub mod adapter;
pub mod build;
pub mod fingerprint;

pub use adapter::{
    CredentialSymlinkAllowlistEntry, RUNTIME_ADAPTER_CONTRACT_VERSION, RuntimeAdapterMetadata,
    RuntimeHomeFile, RuntimeLaunchInstructions,
};
pub use build::{
    BUILD_PLAN_FILE_NAME, BUILD_PLAN_SCHEMA_VERSION, BUILDS_DIR_NAME, RUNTIME_HOME_DIR_NAME,
    RUNTIME_INSTRUCTIONS_FILE_NAME, RUNTIMES_DIR_NAME, runtime_build_dir, runtime_build_plan_file,
    runtime_home_dir, runtime_pointer_dir, runtime_pointer_target,
};
pub use fingerprint::{FINGERPRINT_ALGORITHM, FingerprintBuilder};
