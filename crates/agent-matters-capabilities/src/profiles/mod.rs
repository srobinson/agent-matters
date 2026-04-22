//! Profile use cases.

mod adapter;
mod adapter_capabilities;
mod build_plan;
mod build_writer;
mod claude_adapter;
mod codex_adapter;
mod compile;
mod credential_symlinks;
mod instructions;
mod list;
mod requirements;
mod resolve;
mod runtime;
mod scope;
mod scope_git;
mod show;
mod use_profile;

pub(crate) use adapter::unknown_runtime_adapter;
pub use adapter::{
    CLAUDE_RUNTIME_ID, CODEX_RUNTIME_ID, RuntimeAdapter, RuntimeHomeRenderRequest,
    RuntimeHomeRenderResult, RuntimeLaunchRequest, adapter_for_runtime, runtime_adapter_ids,
    runtime_adapters,
};
pub use build_plan::{
    BuildPlanContentInput, BuildPlanPaths, BuildProfilePlanRequest, BuildProfilePlanResult,
    ProfileBuildPlan, plan_profile_build,
};
pub(crate) use build_plan::{ResolvedProfileBuildPlanRequest, plan_resolved_profile_build};
pub use build_writer::{
    ProfileBuildWriteStatus, WriteProfileBuildRequest, WriteProfileBuildResult,
    WrittenProfileBuild, write_profile_build,
};
pub use compile::{CompileProfileBuildRequest, CompileProfileBuildResult, compile_profile_build};
pub use instructions::AssembledProfileInstructions;
pub use instructions::BuildPlanInstructionOutput;
pub(crate) use instructions::{
    AssembleProfileInstructionsRequest, assemble_profile_instructions, resolve_instruction_output,
};
pub use list::{ListProfilesRequest, ListProfilesResult, list_profiles};
pub(crate) use requirements::validate_resolved_capability_requirements;
pub use requirements::{
    CapabilityRequirementCheck, EnvRequirementCheck, ProfileRequirementValidationMode,
    ProfileRequirementValidationResult, RequirementPresence, validate_profile_requirements,
};
pub(crate) use resolve::resolve_profile_record;
pub use resolve::{
    ResolveProfileRequest, ResolveProfileResult, ResolvedInstructionFragment, resolve_profile,
};
pub use runtime::ResolvedRuntimeConfig;
pub use scope::{
    MatchedScope, ProfileScopeValidationRequest, ProfileScopeValidationResult,
    ProfileScopeValidationStatus, ProfileUseScopeValidationRequest, validate_profile_scope,
    validate_profile_use_scope,
};
pub use show::{ShowProfileRequest, ShowProfileResult, show_profile};
pub use use_profile::{
    ProfileLaunchInstructions, UseProfileRequest, UseProfileResult, use_profile,
};
