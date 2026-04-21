//! `agent-matters capabilities` subcommand surface.
//!
//! Implemented handlers delegate to `agent-matters-capabilities`; remaining
//! verbs return `not yet implemented` until their issue lands.

use agent_matters_capabilities::capabilities::{
    CapabilityDiffStatus, DiffCapabilityRequest, DiffCapabilityResult, ListCapabilitiesRequest,
    diff_capability, list_capabilities,
};
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

fn run_diff(capability: &str, json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = diff_capability(DiffCapabilityRequest {
        repo_root,
        user_state_dir,
        capability: capability.to_string(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        emit_diagnostics(&result.diagnostics);
        render_diff(&result);
    }

    Ok(if result.has_error_diagnostics() { 1 } else { 0 })
}

fn render_diff(result: &DiffCapabilityResult) {
    println!("Capability overlay diff: {}", result.capability);
    if let Some(path) = &result.base_path {
        println!("base: {path}");
    }
    if let Some(path) = &result.overlay_path {
        println!("overlay: {path}");
    }
    if let Some(path) = &result.vendor_path {
        println!("vendor: {path}");
    }

    if result.files.is_empty() {
        println!("No overlay file changes found.");
        return;
    }

    println!();
    for file in &result.files {
        let status = match file.status {
            CapabilityDiffStatus::Added => "added",
            CapabilityDiffStatus::Removed => "removed",
            CapabilityDiffStatus::Changed => "changed",
        };
        println!(
            "{}\t{}\t{} -> {}",
            status,
            file.path,
            render_bytes(file.base_bytes),
            render_bytes(file.overlay_bytes)
        );
        if let Some(note) = &file.note {
            println!("  {note}");
        }
    }
}

fn render_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "-".to_string(), |bytes| bytes.to_string())
}
