//! Shared helpers for integration tests. Cargo treats files directly
//! under `tests/` as test binaries; nesting helpers in a subdirectory
//! (here `support/mod.rs`) keeps them out of that contract so they can
//! be shared by `mod support;` in any integration test file.

#![allow(dead_code)]

pub(crate) mod fixtures;
pub(crate) mod manifests;
pub(crate) mod native_home;
