//! Compile profiles into managed runtime homes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_matters_core::catalog::{CapabilityIndexRecord, MANIFEST_FILE_NAME};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::Serialize;

use crate::catalog::CatalogIndexError;

use super::{
    BuildProfilePlanRequest, ProfileRequirementValidationMode, WriteProfileBuildRequest,
    WrittenProfileBuild, plan_profile_build, validate_resolved_capability_requirements,
    write_profile_build,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileProfileBuildRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
    pub runtime: Option<String>,
    pub env: BTreeMap<String, String>,
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
        repo_root: request.repo_root.clone(),
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

    result.diagnostics.extend(validate_runtime_compatibility(
        &plan.runtime,
        &plan.effective_capabilities,
    ));
    if result.has_error_diagnostics() {
        return Ok(result);
    }

    let requirement_result = validate_resolved_capability_requirements(
        &plan.effective_capabilities,
        &request.env,
        ProfileRequirementValidationMode::Compile,
    );
    result.diagnostics.extend(requirement_result.diagnostics);
    if result.has_error_diagnostics() {
        return Ok(result);
    }

    let write_result = write_profile_build(WriteProfileBuildRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
        plan,
    });
    result.diagnostics.extend(write_result.diagnostics);
    result.build = write_result.build;
    Ok(result)
}

pub(super) fn validate_runtime_compatibility(
    runtime: &str,
    capabilities: &[CapabilityIndexRecord],
) -> Vec<Diagnostic> {
    capabilities
        .iter()
        .filter(|record| !capability_supports_runtime(record, runtime))
        .map(|record| unsupported_capability_runtime(record, runtime))
        .collect()
}

fn capability_supports_runtime(record: &CapabilityIndexRecord, runtime: &str) -> bool {
    record
        .runtimes
        .get(runtime)
        .is_some_and(|support| support.supported)
}

fn unsupported_capability_runtime(record: &CapabilityIndexRecord, runtime: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.runtime.capability-unsupported",
        format!(
            "capability `{}` does not support runtime `{runtime}`",
            record.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        PathBuf::from(&record.source_path).join(MANIFEST_FILE_NAME),
        format!("runtimes.{runtime}"),
    ))
    .with_recovery_hint("remove the capability or choose a runtime it supports")
}
