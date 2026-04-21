//! Pure domain primitives: identifiers, kinds, and their validation rules.
//! This module depends on nothing outside the standard library, serde, and
//! `thiserror`, and is reused by every downstream subsystem.

pub mod capability;
pub mod id;
pub mod profile;
pub mod runtime;

pub use capability::{CapabilityId, CapabilityIdError, CapabilityKind};
pub use id::{IdError, validate_id_body};
pub use profile::{ProfileId, ProfileKind, ProfileKindError};
pub use runtime::RuntimeId;
