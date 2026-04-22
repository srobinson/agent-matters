//! `agent-matters-cli` owns the clap command surface, stdout/stderr
//! rendering, and exit code mapping for the `agent-matters` binary.
//!
//! This crate is a thin adapter. It parses user input, delegates to
//! `agent-matters-capabilities`, and projects results into human or JSON
//! output. No domain logic or orchestration lives here.

#![forbid(unsafe_code)]

pub mod cli;

pub use cli::{Cli, Command, Runtime, dispatch};

/// Crate version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("AGENT_MATTERS_VERSION");
