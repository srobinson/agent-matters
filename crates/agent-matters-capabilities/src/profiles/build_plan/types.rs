use std::path::PathBuf;

use agent_matters_core::catalog::{CapabilityIndexRecord, ProfileIndexRecord};
use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use serde::Serialize;

use super::super::{
    BuildPlanInstructionOutput, ResolveProfileResult, ResolvedInstructionFragment,
    ResolvedRuntimeConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProfilePlanRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedProfileBuildPlanRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub runtime: Option<String>,
    pub resolved: ResolveProfileResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildProfilePlanResult {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<ProfileBuildPlan>,
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildProfilePlanResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileBuildPlan {
    pub schema_version: u16,
    pub profile: String,
    pub runtime: String,
    pub adapter_version: String,
    pub fingerprint: String,
    pub build_id: String,
    pub paths: BuildPlanPaths,
    pub profile_record: ProfileIndexRecord,
    pub effective_capabilities: Vec<CapabilityIndexRecord>,
    pub instruction_fragments: Vec<ResolvedInstructionFragment>,
    pub instruction_output: BuildPlanInstructionOutput,
    pub runtime_config: ResolvedRuntimeConfig,
    pub content_inputs: Vec<BuildPlanContentInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildPlanPaths {
    pub build_dir: PathBuf,
    pub home_dir: PathBuf,
    pub runtime_pointer: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildPlanContentInput {
    pub role: String,
    pub path: String,
    pub content_fingerprint: String,
}
