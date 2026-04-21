//! `agent-matters capabilities` subcommand surface.
//!
//! Handler stubs return `not yet implemented` until the relevant use case
//! ships. Concrete behaviors land in ALP-1944 (list, show) and ALP-1929
//! (diff).

use clap::Subcommand;

/// Verbs for `agent-matters capabilities`.
#[derive(Debug, Subcommand)]
pub enum CapabilitiesCmd {
    /// List capabilities discovered in the catalog.
    List {
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long)]
        json: bool,
    },
    /// Show a single capability and its metadata.
    Show {
        /// Capability identifier.
        capability: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long)]
        json: bool,
    },
    /// Diff a capability overlay against its vendor record.
    Diff {
        /// Capability identifier.
        capability: String,
        /// Emit JSON instead of human readable output.
        #[arg(short = 'j', long)]
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
