//! Build plan generation for resolved profiles.

mod diagnostics;
mod fingerprint;
mod inputs;
mod runtime_selection;
mod types;

use agent_matters_core::runtime::{
    BUILD_PLAN_SCHEMA_VERSION, runtime_build_dir, runtime_home_dir, runtime_pointer_dir,
};

use crate::catalog::CatalogIndexError;

use self::diagnostics::{has_error_diagnostics, remove_default_runtime_diagnostics};
use self::fingerprint::build_fingerprint;
use self::inputs::{content_input_candidates, read_content_inputs};
use self::runtime_selection::select_runtime_config;
pub(crate) use self::types::ResolvedProfileBuildPlanRequest;
pub use self::types::{
    BuildPlanContentInput, BuildPlanPaths, BuildProfilePlanRequest, BuildProfilePlanResult,
    ProfileBuildPlan,
};

use super::{
    ResolveProfileRequest, ResolveProfileResult, adapter_for_runtime, resolve_instruction_output,
    resolve_profile, unknown_runtime_adapter,
};

pub fn plan_profile_build(
    request: BuildProfilePlanRequest,
) -> Result<BuildProfilePlanResult, CatalogIndexError> {
    let resolved = resolve_profile(ResolveProfileRequest {
        repo_root: request.repo_root.clone(),
        user_state_dir: request.user_state_dir.clone(),
        profile: request.profile.clone(),
    })?;
    Ok(plan_resolved_profile_build(
        ResolvedProfileBuildPlanRequest {
            repo_root: request.repo_root,
            user_state_dir: request.user_state_dir,
            runtime: request.runtime,
            resolved,
        },
    ))
}

pub(crate) fn plan_resolved_profile_build(
    request: ResolvedProfileBuildPlanRequest,
) -> BuildProfilePlanResult {
    let ResolvedProfileBuildPlanRequest {
        repo_root,
        user_state_dir,
        runtime,
        resolved,
    } = request;
    let ResolveProfileResult {
        profile,
        record,
        effective_capabilities,
        instruction_fragments,
        runtime_configs,
        selected_runtime,
        diagnostics,
    } = resolved;
    let mut diagnostics = diagnostics;
    if runtime.is_some() {
        remove_default_runtime_diagnostics(&mut diagnostics);
    }
    let mut result = BuildProfilePlanResult {
        profile,
        plan: None,
        diagnostics: Vec::new(),
    };

    let Some(profile_record) = record else {
        result.diagnostics = diagnostics;
        return result;
    };

    let runtime_config = select_runtime_config(
        runtime.as_deref(),
        selected_runtime.as_deref(),
        &runtime_configs,
        &profile_record,
        &mut diagnostics,
    );
    if has_error_diagnostics(&diagnostics) {
        result.diagnostics = diagnostics;
        return result;
    }
    let Some(runtime_config) = runtime_config else {
        result.diagnostics = diagnostics;
        return result;
    };
    let Some(adapter) = adapter_for_runtime(&runtime_config.id) else {
        diagnostics.push(unknown_runtime_adapter(&runtime_config.id));
        result.diagnostics = diagnostics;
        return result;
    };
    diagnostics.extend(adapter.validate_config(&runtime_config));
    if has_error_diagnostics(&diagnostics) {
        result.diagnostics = diagnostics;
        return result;
    }

    let instruction_output = resolve_instruction_output(
        &repo_root,
        &user_state_dir,
        &profile_record,
        &mut diagnostics,
    );
    if has_error_diagnostics(&diagnostics) {
        result.diagnostics = diagnostics;
        return result;
    }

    let candidates = content_input_candidates(
        &repo_root,
        &user_state_dir,
        &profile_record,
        &effective_capabilities,
    );
    let read_inputs = read_content_inputs(candidates, &mut diagnostics);
    if has_error_diagnostics(&diagnostics) {
        result.diagnostics = diagnostics;
        return result;
    }

    let adapter_version = adapter.version().to_string();
    let fingerprint = build_fingerprint(
        &profile_record,
        &effective_capabilities,
        &instruction_fragments,
        &instruction_output,
        &runtime_config,
        &adapter_version,
        &read_inputs,
    );
    let build_id = fingerprint
        .split_once(':')
        .map_or_else(|| fingerprint.clone(), |(_, digest)| digest.to_string());
    let paths = BuildPlanPaths {
        build_dir: runtime_build_dir(&runtime_config.id, &profile_record.id, &build_id),
        home_dir: runtime_home_dir(&runtime_config.id, &profile_record.id, &build_id),
        runtime_pointer: runtime_pointer_dir(&profile_record.id, &runtime_config.id),
    };

    result.plan = Some(ProfileBuildPlan {
        schema_version: BUILD_PLAN_SCHEMA_VERSION,
        profile: profile_record.id.clone(),
        runtime: runtime_config.id.clone(),
        adapter_version,
        fingerprint,
        build_id,
        paths,
        profile_record,
        effective_capabilities,
        instruction_fragments,
        instruction_output,
        runtime_config,
        content_inputs: read_inputs
            .into_iter()
            .map(|input| input.content_input)
            .collect(),
    });
    result.diagnostics = diagnostics;
    result
}
