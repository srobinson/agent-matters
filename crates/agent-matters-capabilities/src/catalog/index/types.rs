use std::io;
use std::path::PathBuf;

use agent_matters_core::catalog::CatalogIndex;
use agent_matters_core::domain::Diagnostic;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadCatalogIndexRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadCatalogIndexResult {
    pub index: CatalogIndex,
    pub index_path: PathBuf,
    pub status: CatalogIndexStatus,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogIndexStatus {
    Fresh,
    RebuiltMissing,
    RebuiltStale,
    RecoveredCorrupt,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogIndexError {
    #[error("failed to read generated index `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write generated index `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to encode generated index `{path}`: {source}")]
    Encode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}
