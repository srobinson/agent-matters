//! Profile use cases.

mod build_plan;
mod list;
mod requirements;
mod resolve;
mod runtime;
mod scope;
mod scope_git;
mod show;

pub use build_plan::{
    BuildPlanContentInput, BuildPlanPaths, BuildProfilePlanRequest, BuildProfilePlanResult,
    ProfileBuildPlan, plan_profile_build,
};
pub use list::{ListProfilesRequest, ListProfilesResult, list_profiles};
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
