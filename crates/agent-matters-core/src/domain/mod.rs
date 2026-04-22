//! Pure domain primitives: identifiers, kinds, and their validation rules.
//! This module depends on nothing outside the standard library, serde, and
//! `thiserror`, and is reused by every downstream subsystem.

pub mod capability;
pub mod diagnostic;
pub mod id;
pub mod profile;
pub mod provenance;
pub mod requirement;
pub mod runtime;
pub mod scope;

pub use capability::{CapabilityId, CapabilityIdError, CapabilityKind};
pub use diagnostic::{
    Diagnostic, DiagnosticLocation, DiagnosticReport, DiagnosticSeverity, render_diagnostics_human,
};
pub use id::{IdError, validate_id_body, validate_path_segment_id_body};
pub use profile::{ProfileId, ProfileKind, ProfileKindError};
pub use provenance::Provenance;
pub use requirement::{
    EnvVarCheck, EnvVarPresence, EnvVarRequirement, EnvVarRequirementError, Requirements,
};
pub use runtime::RuntimeId;
pub use scope::{ScopeConstraints, ScopeEnforcement, ScopeEnforcementError};
