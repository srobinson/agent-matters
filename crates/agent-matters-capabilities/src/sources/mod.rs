//! Source adapter contracts and import storage for external capability sources.
//!
//! Concrete adapters live behind this boundary. They normalize source specific
//! records into capability manifests while keeping raw upstream records in the
//! vendor tree for audit and future diff workflows.

mod contract;
mod import;
pub mod mcp_registry_spec;
mod runtime_import;
mod search;
mod skills_sh;
mod skills_sh_parse;
mod storage;

pub use contract::{
    SourceAdapter, SourceAdapterError, SourceImportFile, SourceImportRequest, SourceImportResult,
    SourceSearchEntry, SourceSearchRequest, SourceSearchResult,
};
pub use import::{
    ImportSourceAdapterRequest, ImportSourceError, ImportSourceRequest, ImportSourceResult,
    ImportSourceStatus, import_source, import_source_from_adapter,
    import_source_from_adapter_with_policy,
};
pub use runtime_import::{
    ImportRuntimeHomeRequest, RuntimeHomeImportCapability, RuntimeHomeImportError,
    RuntimeHomeImportResult, RuntimeHomeImportSkippedFile, RuntimeHomeImportStatus,
    import_runtime_home,
};
pub use search::{SearchSourceRequest, search_source};
pub use skills_sh::{CommandOutput, NpxSkillsCommand, SkillsShAdapter, SkillsShCommand};
pub use storage::{
    SourceImportStorageError, WriteSourceImportRequest, WriteSourceImportResult,
    WriteSourceImportStatus, write_source_import,
};
