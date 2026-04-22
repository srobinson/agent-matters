//! Noun first command surface integration tests (ALP-1920).
//!
//! Keeps the command surface grouped by CLI noun while preserving one
//! integration test crate for the shared binary and fixture helpers.

#[path = "commands/capabilities.rs"]
mod capabilities;
#[path = "support/mod.rs"]
mod common;
#[path = "commands/completions.rs"]
mod completions;
#[path = "commands/doctor.rs"]
mod doctor;
#[path = "commands/help.rs"]
mod help;
#[path = "commands/profiles.rs"]
mod profiles;
#[path = "commands/sources.rs"]
mod sources;
