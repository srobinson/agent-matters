//! Filesystem writer for immutable profile builds and stable runtime pointers.

mod diagnostics;
mod existing;
mod immutable;
mod paths;
mod runtime_pointer;

use std::path::PathBuf;

use agent_matters_core::domain::Diagnostic;
use agent_matters_core::runtime::RuntimeCredentialSymlink;
use serde::Serialize;

use diagnostics::{has_error_diagnostics, write_diagnostic};
use immutable::write_immutable_build;
use paths::AbsoluteBuildPaths;
use runtime_pointer::update_runtime_pointer;

use super::{
    AssembleProfileInstructionsRequest, ProfileBuildPlan, RuntimeHomeRenderRequest,
    adapter_for_runtime, assemble_profile_instructions,
    credential_symlinks::credential_symlinks_for_adapter, unknown_runtime_adapter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteProfileBuildRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub native_home_dir: Option<PathBuf>,
    pub plan: ProfileBuildPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriteProfileBuildResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<WrittenProfileBuild>,
    pub diagnostics: Vec<Diagnostic>,
}

impl WriteProfileBuildResult {
    pub fn has_error_diagnostics(&self) -> bool {
        has_error_diagnostics(&self.diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WrittenProfileBuild {
    pub profile: String,
    pub runtime: String,
    pub fingerprint: String,
    pub build_id: String,
    pub status: ProfileBuildWriteStatus,
    pub build_dir: PathBuf,
    pub home_dir: PathBuf,
    pub runtime_pointer: PathBuf,
    pub pointer_target: PathBuf,
    pub build_plan_path: PathBuf,
    pub credential_symlinks: Vec<RuntimeCredentialSymlink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileBuildWriteStatus {
    Created,
    Reused,
}

pub fn write_profile_build(request: WriteProfileBuildRequest) -> WriteProfileBuildResult {
    let paths = AbsoluteBuildPaths::new(&request.user_state_dir, &request.plan);
    let mut result = WriteProfileBuildResult {
        build: None,
        diagnostics: Vec::new(),
    };
    let assembled = assemble_profile_instructions(AssembleProfileInstructionsRequest {
        repo_root: &request.repo_root,
        profile: &request.plan.profile,
        fragments: &request.plan.instruction_fragments,
        output: &request.plan.instruction_output,
    });
    if has_error_diagnostics(&assembled.diagnostics) {
        result.diagnostics = assembled.diagnostics;
        return result;
    }
    let Some(instructions) = assembled.instructions else {
        result.diagnostics = assembled.diagnostics;
        return result;
    };
    let Some(adapter) = adapter_for_runtime(&request.plan.runtime) else {
        result
            .diagnostics
            .push(unknown_runtime_adapter(&request.plan.runtime));
        return result;
    };
    let credential_symlinks = credential_symlinks_for_adapter(
        adapter,
        request.native_home_dir.as_deref(),
        &mut result.diagnostics,
    );
    let home = adapter.render_home(RuntimeHomeRenderRequest {
        repo_root: &request.repo_root,
        plan: &request.plan,
        instructions: &instructions,
    });
    result.diagnostics.extend(home.diagnostics.clone());
    if has_error_diagnostics(&result.diagnostics) {
        return result;
    }

    let status =
        match write_immutable_build(&paths, &request.plan, adapter, &home, &credential_symlinks) {
            Ok(status) => status,
            Err(source) => {
                result.diagnostics.push(write_diagnostic(
                    "write immutable build",
                    &paths.build_dir,
                    &source,
                ));
                return result;
            }
        };

    if let Err(source) = update_runtime_pointer(&paths.runtime_pointer, &paths.pointer_target) {
        result.diagnostics.push(write_diagnostic(
            "update runtime pointer",
            &paths.runtime_pointer,
            &source,
        ));
        return result;
    }

    result.build = Some(WrittenProfileBuild {
        profile: request.plan.profile,
        runtime: request.plan.runtime,
        fingerprint: request.plan.fingerprint,
        build_id: request.plan.build_id,
        status,
        build_dir: paths.build_dir,
        home_dir: paths.home_dir,
        runtime_pointer: paths.runtime_pointer,
        pointer_target: paths.pointer_target,
        build_plan_path: paths.build_plan_path,
        credential_symlinks,
    });
    result
}
