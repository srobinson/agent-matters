//! `agent-matters-core` owns the pure domain of the runtime profile system.
//!
//! This crate holds manifest schemas, typed identifiers, validation rules,
//! diagnostics, and filesystem path conventions. It performs no I/O and
//! depends on no runtime or CLI machinery. Downstream crates
//! (`agent-matters-capabilities`, `agent-matters-cli`) build use cases and
//! surfaces on top of these types.
//!
//! See `/Users/alphab/.mdx/products/agent-matters-runtime-profiles.md` for
//! the product and architecture spec.

#![forbid(unsafe_code)]

pub mod catalog;
pub mod config;
pub mod domain;
pub mod manifest;
pub mod runtime;

/// Crate version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty());
    }
}
