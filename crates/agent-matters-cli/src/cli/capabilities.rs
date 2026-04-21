//! `agent-matters capabilities` subcommand surface.
//!
//! Handler stubs return `not yet implemented` until the relevant use case
//! ships. Concrete behaviors land in ALP-1944 (list, show) and ALP-1929
//! (diff).

use clap::Subcommand;

use super::{generated_help, help_text};

/// Verbs for `agent-matters capabilities`.
#[derive(Debug, Subcommand)]
pub enum CapabilitiesCmd {
    /// List capabilities discovered in the catalog.
    #[command(
        long_about = generated_help::CAPABILITIES_LIST_ABOUT,
        after_help = help_text::CAPABILITIES_LIST_AFTER_HELP
    )]
    List {
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::CAPABILITIES_LIST_JSON_HELP)]
        json: bool,
    },
    /// Show a single capability and its metadata.
    #[command(
        long_about = generated_help::CAPABILITIES_SHOW_ABOUT,
        after_help = help_text::CAPABILITIES_SHOW_AFTER_HELP
    )]
    Show {
        /// Capability identifier.
        #[arg(help = generated_help::CAPABILITIES_SHOW_CAPABILITY_HELP)]
        capability: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::CAPABILITIES_SHOW_JSON_HELP)]
        json: bool,
    },
    /// Diff a capability overlay against its vendor record.
    #[command(
        long_about = generated_help::CAPABILITIES_DIFF_ABOUT,
        after_help = help_text::CAPABILITIES_DIFF_AFTER_HELP
    )]
    Diff {
        /// Capability identifier.
        #[arg(help = generated_help::CAPABILITIES_DIFF_CAPABILITY_HELP)]
        capability: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long, help = generated_help::CAPABILITIES_DIFF_JSON_HELP)]
        json: bool,
    },
}

/// Dispatch a parsed `capabilities` subcommand to its handler.
pub fn dispatch(cmd: CapabilitiesCmd) -> anyhow::Result<i32> {
    match cmd {
        CapabilitiesCmd::List { json } => run_list(json),
        CapabilitiesCmd::Show { capability, json } => run_show(&capability, json),
        CapabilitiesCmd::Diff { capability, json } => run_diff(&capability, json),
    }
}

fn run_list(_json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("capabilities list: not yet implemented (ALP-1944)")
}

fn run_show(_capability: &str, _json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("capabilities show: not yet implemented (ALP-1944)")
}

fn run_diff(_capability: &str, _json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("capabilities diff: not yet implemented (ALP-1929)")
}
