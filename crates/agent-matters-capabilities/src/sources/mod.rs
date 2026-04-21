//! Source adapter contracts and import storage for external capability sources.
//!
//! Concrete adapters live behind this boundary. They normalize source specific
//! records into capability manifests while keeping raw upstream records in the
//! vendor tree for audit and future diff workflows.

mod contract;
mod storage;

pub use contract::{
    SourceAdapter, SourceAdapterError, SourceImportFile, SourceImportRequest, SourceImportResult,
    SourceSearchEntry, SourceSearchRequest, SourceSearchResult,
};
pub use storage::{
    SourceImportStorageError, WriteSourceImportRequest, WriteSourceImportResult,
    write_source_import,
};
