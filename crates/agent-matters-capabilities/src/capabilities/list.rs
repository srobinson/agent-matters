//! List capabilities through the generated catalog index.

use std::path::PathBuf;

use agent_matters_core::catalog::CapabilityIndexRecord;
use agent_matters_core::domain::Diagnostic;
use serde::Serialize;

use crate::catalog::{
    CatalogIndexError, CatalogIndexStatus, LoadCatalogIndexRequest, load_or_refresh_catalog_index,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCapabilitiesRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListCapabilitiesResult {
    pub capabilities: Vec<CapabilityIndexRecord>,
    pub index_path: PathBuf,
    pub index_status: CatalogIndexStatus,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn list_capabilities(
    request: ListCapabilitiesRequest,
) -> Result<ListCapabilitiesResult, CatalogIndexError> {
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
    })?;

    Ok(ListCapabilitiesResult {
        capabilities: loaded.index.capabilities.values().cloned().collect(),
        index_path: loaded.index_path,
        index_status: loaded.status,
        diagnostics: loaded.diagnostics,
    })
}
