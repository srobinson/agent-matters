//! `agent-matters-cli` owns the clap command surface, stdout/stderr
//! rendering, and exit code mapping for the `agent-matters` binary.
//!
//! This crate is a thin adapter. It parses user input, delegates to
//! `agent-matters-capabilities`, and projects results into human or JSON
//! output. No domain logic or orchestration lives here.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

/// Crate version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Top level `agent-matters` CLI.
///
/// Command groups land in sub issues of ALP-1910. Until then the binary
/// exposes a useful top level help screen and a smoke level `--version`
/// output so downstream scaffolding can depend on a stable entry point.
#[derive(Debug, Parser)]
#[command(
    name = "agent-matters",
    version,
    about = "Local first runtime profile system for Codex, Claude, and future CLI runtimes",
    long_about = "agent-matters compiles selected capabilities, instructions, hooks, \
                  MCP servers, runtime settings, and launch material into focused \
                  runtime homes for Codex, Claude, and future CLI runtimes. \
                  Runtime homes (`.codex`, `.claude`) are generated rather than hand \
                  maintained source of truth."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top level command groups.
///
/// The concrete surface for each group (`profiles`, `capabilities`,
/// `sources`, `doctor`) is implemented in the sub issues of ALP-1910 and
/// ALP-1912 through ALP-1917. The scaffold only declares the enum so
/// `--help` produces the right shape.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage runtime profiles.
    Profiles,
    /// Manage capabilities.
    Capabilities,
    /// Manage catalog sources.
    Sources,
    /// Diagnose catalog, runtime, and auth setup.
    Doctor,
}

/// Dispatch a parsed CLI to the capabilities layer.
///
/// Returns the intended process exit code. The scaffold only reports that
/// no command was supplied; concrete handlers arrive with their issues.
pub fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        None => {
            eprintln!("agent-matters: no command supplied. Run `agent-matters --help`.");
            Ok(2)
        }
        Some(Command::Profiles | Command::Capabilities | Command::Sources | Command::Doctor) => {
            eprintln!("agent-matters: command not yet implemented in this scaffold.");
            Ok(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn help_mentions_runtime_profiles() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("runtime"));
        assert!(help.contains("profile"));
    }
}
