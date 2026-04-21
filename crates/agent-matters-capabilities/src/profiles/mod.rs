//! Profile use cases.

mod build_plan;
mod build_writer;
mod compile;
mod instructions;
mod list;
mod requirements;
mod resolve;
mod runtime;
mod scope;
mod scope_git;
mod show;
mod use_profile;

pub use build_plan::{
    BuildPlanContentInput, BuildPlanPaths, BuildProfilePlanRequest, BuildProfilePlanResult,
    ProfileBuildPlan, plan_profile_build,
};
pub use build_writer::{
    ProfileBuildWriteStatus, WriteProfileBuildRequest, WriteProfileBuildResult,
    WrittenProfileBuild, write_profile_build,
};
pub use compile::{CompileProfileBuildRequest, CompileProfileBuildResult, compile_profile_build};
pub use instructions::BuildPlanInstructionOutput;
pub(crate) use instructions::{
    AssembleProfileInstructionsRequest, AssembledProfileInstructions,
    assemble_profile_instructions, resolve_instruction_output,
};
pub use list::{ListProfilesRequest, ListProfilesResult, list_profiles};
pub(crate) use requirements::validate_resolved_capability_requirements;
pub use requirements::{
    CapabilityRequirementCheck, EnvRequirementCheck, ProfileRequirementValidationMode,
    ProfileRequirementValidationResult, RequirementPresence, validate_profile_requirements,
};
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
