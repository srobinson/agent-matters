//! `agent-matters capabilities` subcommand surface.
//!
//! Implemented handlers delegate to `agent-matters-capabilities`; remaining
//! verbs return `not yet implemented` until their issue lands.

use agent_matters_capabilities::capabilities::{ListCapabilitiesRequest, list_capabilities};
use clap::Subcommand;

use super::{
    default_catalog_paths, emit_diagnostics, generated_help, help_text, render_runtime_names,
};

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

fn run_list(json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = list_capabilities(ListCapabilitiesRequest {
        repo_root,
        user_state_dir,
    })?;
    emit_diagnostics(&result.diagnostics);

    if json {
        println!("{}", serde_json::to_string_pretty(&result.capabilities)?);
    } else if result.capabilities.is_empty() {
        println!("No capabilities found.");
    } else {
        for capability in result.capabilities {
            println!(
                "{}\t{}\t{}\t{}",
                capability.id,
                capability.kind,
                render_runtime_names(&capability.runtimes),
                capability.source_path
            );
        }
    }

    Ok(0)
}

fn run_show(_capability: &str, _json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("capabilities show: not yet implemented (ALP-1944)")
}

fn run_diff(_capability: &str, _json: bool) -> anyhow::Result<i32> {
    anyhow::bail!("capabilities diff: not yet implemented (ALP-1929)")
}
