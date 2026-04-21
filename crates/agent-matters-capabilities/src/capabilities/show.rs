//! Show one capability through the generated catalog index.

use std::path::PathBuf;

use agent_matters_core::catalog::CapabilityIndexRecord;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::Serialize;

use crate::catalog::{CatalogIndexError, LoadCatalogIndexRequest, load_or_refresh_catalog_index};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowCapabilityRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShowCapabilityResult {
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<CapabilityIndexRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ShowCapabilityResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

pub fn show_capability(
    request: ShowCapabilityRequest,
) -> Result<ShowCapabilityResult, CatalogIndexError> {
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
    })?;
    let record = loaded.index.capability(&request.capability).cloned();
    let mut result = ShowCapabilityResult {
        capability: request.capability,
        record,
        diagnostics: loaded.diagnostics,
    };

    if result.record.is_none() {
        result.diagnostics.push(not_found(&result.capability));
    }

    Ok(result)
}

fn not_found(capability: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.show-not-found",
        format!("capability `{capability}` was not found in the catalog index"),
    )
    .with_location(DiagnosticLocation::field("capability"))
    .with_recovery_hint("run `agent-matters capabilities list` to inspect exact capability ids")
}
