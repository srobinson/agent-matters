//! Prepare a generated runtime home for manual profile launch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use agent_matters_core::runtime::RuntimeLaunchInstructions;
use serde::Serialize;

use crate::catalog::CatalogIndexError;

use super::compile::validate_runtime_compatibility;
use super::{
    BuildProfilePlanRequest, ProfileRequirementValidationMode, ProfileScopeValidationResult,
    ProfileUseScopeValidationRequest, ResolveProfileResult, RuntimeLaunchRequest,
    WriteProfileBuildRequest, WrittenProfileBuild, adapter_for_runtime, plan_profile_build,
    validate_profile_use_scope, validate_resolved_capability_requirements, write_profile_build,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseProfileRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
    pub runtime: Option<String>,
    pub workspace_path: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
}

pub type ProfileLaunchInstructions = RuntimeLaunchInstructions;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UseProfileResult {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<WrittenProfileBuild>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ProfileScopeValidationResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<ProfileLaunchInstructions>,
    pub diagnostics: Vec<Diagnostic>,
}

impl UseProfileResult {
    pub fn has_error_diagnostics(&self) -> bool {
        has_error_diagnostics(&self.diagnostics)
    }
}

pub fn use_profile(request: UseProfileRequest) -> Result<UseProfileResult, CatalogIndexError> {
    let plan_result = plan_profile_build(BuildProfilePlanRequest {
        repo_root: request.repo_root.clone(),
        user_state_dir: request.user_state_dir.clone(),
        profile: request.profile,
        runtime: request.runtime,
    })?;
    let mut result = UseProfileResult {
        profile: plan_result.profile,
        build: None,
        scope: None,
        launch: None,
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

    let scope = validate_profile_use_scope(ProfileUseScopeValidationRequest {
        resolved: &resolved_for_scope(&plan),
        repo_root: request.repo_root.clone(),
        workspace_path: request.workspace_path,
    });
    result.diagnostics.extend(scope.diagnostics.clone());
    let workspace_path = scope
        .canonical_path
        .clone()
        .unwrap_or_else(|| scope.requested_path.clone());
    result.scope = Some(scope);
    if result.has_error_diagnostics() {
        return Ok(result);
    }

    let requirement_result = validate_resolved_capability_requirements(
        &plan.effective_capabilities,
        &request.env,
        ProfileRequirementValidationMode::Use,
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
    if result.has_error_diagnostics() {
        return Ok(result);
    }

    if let Some(build) = &result.build {
        result.launch = launch_instructions(build, &workspace_path);
    }
    Ok(result)
}

fn resolved_for_scope(plan: &super::ProfileBuildPlan) -> ResolveProfileResult {
    ResolveProfileResult {
        profile: plan.profile.clone(),
        record: Some(plan.profile_record.clone()),
        effective_capabilities: plan.effective_capabilities.clone(),
        instruction_fragments: plan.instruction_fragments.clone(),
        runtime_configs: vec![plan.runtime_config.clone()],
        selected_runtime: Some(plan.runtime.clone()),
        diagnostics: Vec::new(),
    }
}

fn launch_instructions(
    build: &WrittenProfileBuild,
    workspace_path: &str,
) -> Option<ProfileLaunchInstructions> {
    adapter_for_runtime(&build.runtime).map(|adapter| {
        adapter.launch_instructions(RuntimeLaunchRequest {
            runtime_home: &build.runtime_pointer,
            workspace_path,
        })
    })
}

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}
