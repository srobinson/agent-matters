//! Profile use cases.

mod list;
mod requirements;
mod resolve;
mod runtime;
mod scope;
mod scope_git;

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
