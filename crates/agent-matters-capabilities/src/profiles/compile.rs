//! Compile profiles into managed runtime homes.

use std::path::PathBuf;

use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use serde::Serialize;

use crate::catalog::CatalogIndexError;

use super::{
    BuildProfilePlanRequest, WriteProfileBuildRequest, WrittenProfileBuild, plan_profile_build,
    write_profile_build,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileProfileBuildRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompileProfileBuildResult {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<WrittenProfileBuild>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CompileProfileBuildResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

pub fn compile_profile_build(
    request: CompileProfileBuildRequest,
) -> Result<CompileProfileBuildResult, CatalogIndexError> {
    let plan_result = plan_profile_build(BuildProfilePlanRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir.clone(),
        profile: request.profile,
        runtime: request.runtime,
    })?;

    let mut result = CompileProfileBuildResult {
        profile: plan_result.profile,
        build: None,
        diagnostics: plan_result.diagnostics,
    };

    if result.has_error_diagnostics() {
        return Ok(result);
    }

    let Some(plan) = plan_result.plan else {
        return Ok(result);
    };

    let write_result = write_profile_build(WriteProfileBuildRequest {
        user_state_dir: request.user_state_dir,
        plan,
    });
    result.diagnostics.extend(write_result.diagnostics);
    result.build = write_result.build;
    Ok(result)
}
