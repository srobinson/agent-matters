//! `agent-matters capabilities` subcommand surface.
//!
//! Handlers delegate to `agent-matters-capabilities`.

use agent_matters_capabilities::capabilities::{
    CapabilityDiffStatus, DiffCapabilityRequest, DiffCapabilityResult, ListCapabilitiesRequest,
    ShowCapabilityRequest, ShowCapabilityResult, diff_capability, list_capabilities,
    show_capability,
};
use agent_matters_core::catalog::{CapabilityIndexRecord, ProvenanceSummary};
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
                "{}\t{}\t{}\t{}\t{}",
                capability.id,
                capability.kind,
                render_runtime_names(&capability.runtimes),
                render_provenance_state(&capability),
                capability.summary
            );
        }
    }

    Ok(0)
}

fn run_show(capability: &str, json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = show_capability(ShowCapabilityRequest {
        repo_root,
        user_state_dir,
        capability: capability.to_string(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        emit_diagnostics(&result.diagnostics);
        render_show(&result);
    }

    Ok(if result.has_error_diagnostics() { 1 } else { 0 })
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

fn render_show(result: &ShowCapabilityResult) {
    let Some(record) = &result.record else {
        return;
    };

    println!("Capability: {}", record.id);
    println!("kind: {}", record.kind);
    println!("summary: {}", record.summary);
    println!("source path: {}", record.source_path);
    println!("overlay state: {}", record.source.kind);
    if let Some(path) = &record.source.normalized_path {
        println!("normalized: {path}");
    }
    if let Some(path) = &record.source.overlay_path {
        println!("overlay: {path}");
    }
    if let Some(path) = &record.source.vendor_path {
        println!("vendor: {path}");
    }

    println!();
    println!("files:");
    if record.files.is_empty() {
        println!("none");
    } else {
        for (name, path) in &record.files {
            println!("{name}\t{path}");
        }
    }

    println!();
    println!("runtimes:");
    if record.runtimes.is_empty() {
        println!("none");
    } else {
        for (runtime, support) in &record.runtimes {
            let state = if support.supported {
                "supported"
            } else {
                "unsupported"
            };
            if let Some(model) = &support.model {
                println!("{runtime}\t{state}\tmodel={model}");
            } else {
                println!("{runtime}\t{state}");
            }
        }
    }

    println!();
    println!("requirements:");
    render_requirement_list("capabilities", &record.requirements.capabilities);
    render_requirement_list("env", &record.requirements.env);

    println!();
    println!("provenance:");
    println!("kind: {}", record.provenance.kind);
    if let Some(source) = &record.provenance.source {
        println!("source: {source}");
    }
    if let Some(locator) = &record.provenance.locator {
        println!("locator: {locator}");
    }
    if let Some(version) = &record.provenance.version {
        println!("version: {version}");
    }
}

fn render_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "-".to_string(), |bytes| bytes.to_string())
}

fn render_provenance_state(record: &CapabilityIndexRecord) -> String {
    let mut state = record.source.kind.clone();
    if record.provenance.kind != "local" {
        state.push(' ');
        state.push_str(&render_provenance(&record.provenance));
    }
    state
}

fn render_provenance(provenance: &ProvenanceSummary) -> String {
    let mut rendered = provenance.kind.clone();
    if let Some(source) = &provenance.source {
        rendered.push(':');
        rendered.push_str(source);
    }
    if let Some(locator) = &provenance.locator {
        rendered.push('/');
        rendered.push_str(locator);
    }
    if let Some(version) = &provenance.version {
        rendered.push('@');
        rendered.push_str(version);
    }
    rendered
}

fn render_requirement_list(label: &str, values: &[String]) {
    if values.is_empty() {
        println!("{label}: none");
    } else {
        println!("{label}: {}", values.join(","));
    }
}
