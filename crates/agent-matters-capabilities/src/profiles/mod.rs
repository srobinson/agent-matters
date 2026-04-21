//! Profile use cases.

mod list;
mod resolve;

pub use list::{ListProfilesRequest, ListProfilesResult, list_profiles};
pub use resolve::{
    ResolveProfileRequest, ResolveProfileResult, ResolvedInstructionFragment, resolve_profile,
};
