//! `agent-matters-capabilities` owns the use case layer for the runtime
//! profile system.
//!
//! Each capability accepts a structured request, performs the orchestration
//! required to satisfy it against the filesystem and domain types in
//! `agent-matters-core`, and returns a structured result. The CLI crate is
//! a thin adapter over these use cases. Later subsystems (a future GUI or
//! automation layer) would reuse the same capability surface.
//!
//! Concrete capabilities land in the sub issues of ALP-1960.

#![forbid(unsafe_code)]

pub mod capabilities;
pub mod catalog;
pub mod config;
pub mod profiles;
pub mod sources;

/// Crate version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexports_core_version_independently() {
        assert!(!VERSION.is_empty());
        assert!(!agent_matters_core::VERSION.is_empty());
    }
}
