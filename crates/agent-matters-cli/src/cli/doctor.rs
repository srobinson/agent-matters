//! `agent-matters doctor` subcommand.
//!
//! Doctor is flat (no verbs) because the MVP runs a single integrity sweep.
//! Concrete checks arrive in ALP-1949, ALP-1951, ALP-1952, and ALP-1953.

/// Run all registered doctor checks.
pub fn run(_json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("doctor: not yet implemented (ALP-1949)")
}
