//! TOML manifest schemas for authored profiles and catalog capabilities.
//!
//! These structs model the file contract only. They perform no I/O and do not
//! apply runtime defaults, resolve capability references, or validate scope
//! policy. Later capability workflows load these schemas and layer diagnostics
//! on top of Serde's field specific TOML errors.

pub mod capability;
pub mod profile;

pub use capability::{
    CapabilityFilesManifest, CapabilityManifest, CapabilityRequirementsManifest,
    CapabilityRuntimeManifest, CapabilityRuntimesManifest, OriginManifest,
};
pub use profile::{
    InstructionMarkers, InstructionsOutputManifest, ProfileManifest, ProfileRuntimeManifest,
    ProfileRuntimesManifest, ProfileScopeManifest, ScopeEnforcement,
};
