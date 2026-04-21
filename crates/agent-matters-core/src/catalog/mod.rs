//! Pure catalog filesystem conventions.
//!
//! The catalog module defines path fragments for authored source state. It
//! performs no I/O; downstream use case crates decide how to read, validate,
//! and render files at these paths.

pub mod index;
pub mod paths;

pub use index::{
    CATALOG_INDEX_FILE_NAME, CATALOG_INDEX_SCHEMA_VERSION, CapabilityIndexRecord, CatalogIndex,
    INDEXES_DIR_NAME, ProfileIndexRecord, ProvenanceSummary, RequirementSummary,
    RuntimeCompatibilitySummary,
};
pub use paths::{
    CAPABILITY_AGENTS_DIR_NAME, CAPABILITY_HOOKS_DIR_NAME, CAPABILITY_INSTRUCTIONS_DIR_NAME,
    CAPABILITY_MCP_DIR_NAME, CAPABILITY_RUNTIME_SETTINGS_DIR_NAME, CAPABILITY_SKILLS_DIR_NAME,
    CATALOG_DIR_NAME, DEFAULTS_DIR_NAME, MANIFEST_FILE_NAME, OVERLAYS_DIR_NAME, PROFILES_DIR_NAME,
    VENDOR_DIR_NAME, capability_kind_dir_name, known_capability_dir_names,
};
