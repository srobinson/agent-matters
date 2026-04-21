//! Top level clap surface and dispatch entry point.
//!
//! The command tree is noun first: each top level variant of [`Command`]
//! carries a nested enum listing the verbs for that noun. Keeping the nouns
//! in their own modules lets each file stay single responsibility.
//!
//! Handlers are thin adapters that delegate to `agent-matters-capabilities`.
//! Until use cases land in their own issues they return a clear `not yet
//! implemented` error so the CLI surface is discoverable and testable.

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

pub mod capabilities;
pub mod doctor;
pub mod profiles;
pub mod sources;

/// Top level `agent-matters` CLI.
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
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage runtime profiles.
    Profiles {
        #[command(subcommand)]
        command: profiles::ProfilesCmd,
    },
    /// Manage capabilities.
    Capabilities {
        #[command(subcommand)]
        command: capabilities::CapabilitiesCmd,
    },
    /// Manage catalog sources.
    Sources {
        #[command(subcommand)]
        command: sources::SourcesCmd,
    },
    /// Diagnose catalog, runtime, and auth setup.
    Doctor {
        /// Emit JSON instead of human readable diagnostics.
        #[arg(short = 'j', long)]
        json: bool,
    },
    /// Generate shell completion script for the `agent-matters` binary.
    Completions {
        /// Target shell: bash, zsh, fish, powershell, elvish.
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Target runtime for compile and use commands.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum Runtime {
    /// OpenAI Codex CLI.
    Codex,
    /// Anthropic Claude Code CLI.
    Claude,
}

/// Parse the top level CLI and dispatch to the appropriate handler.
///
/// Returns the intended process exit code. A bare invocation with no
/// subcommand prints long help and exits zero; all other paths rely on
/// handler return values.
pub fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        None => {
            Cli::command().print_long_help()?;
            println!();
            Ok(0)
        }
        Some(Command::Profiles { command }) => profiles::dispatch(command),
        Some(Command::Capabilities { command }) => capabilities::dispatch(command),
        Some(Command::Sources { command }) => sources::dispatch(command),
        Some(Command::Doctor { json }) => doctor::run(json),
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "agent-matters", &mut std::io::stdout());
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
