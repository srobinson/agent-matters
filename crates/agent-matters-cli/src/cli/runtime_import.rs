//! Top level `agent-matters import` command.
//!
//! Runtime home import is a catalog mutation workflow, so this module keeps
//! the CLI surface separate from source adapter import.

use std::path::{Path, PathBuf};

use agent_matters_capabilities::sources::{
    ImportRuntimeHomeRequest, RuntimeHomeImportResult, RuntimeHomeImportStatus, import_runtime_home,
};
use agent_matters_core::domain::{DiagnosticReport, DiagnosticSeverity};

use super::{Runtime, default_catalog_paths, emit_diagnostics};

pub fn run(
    path: PathBuf,
    profile: Option<String>,
    runtime: Option<Runtime>,
    write: bool,
    json: bool,
) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    match import_runtime_home(ImportRuntimeHomeRequest {
        repo_root: repo_root.clone(),
        user_state_dir,
        runtime: runtime.map(|runtime| runtime.as_str().to_string()),
        source_home: path,
        profile,
        write,
    }) {
        Ok(result) => {
            let exit_code = if import_result_has_errors(&result) {
                1
            } else {
                0
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                emit_diagnostics(&result.diagnostics);
                if exit_code == 0 {
                    render_import_runtime(&repo_root, &result);
                }
            }
            Ok(exit_code)
        }
        Err(err) => {
            let diagnostic = err.to_diagnostic();
            if json {
                let diagnostics = vec![diagnostic];
                println!(
                    "{}",
                    serde_json::to_string_pretty(&DiagnosticReport::new(&diagnostics))?
                );
            } else {
                emit_diagnostics(&[diagnostic]);
            }
            Ok(1)
        }
    }
}

fn import_result_has_errors(result: &RuntimeHomeImportResult) -> bool {
    result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn render_import_runtime(repo_root: &Path, result: &RuntimeHomeImportResult) {
    let verb = match result.status {
        RuntimeHomeImportStatus::DryRun => "Dry run",
        RuntimeHomeImportStatus::Imported => "Imported",
    };
    println!("{verb} runtime profile {}", result.profile_id);
    println!("runtime\t{}", result.runtime);
    println!("source\t{}", result.source_home.display());
    println!(
        "profile\t{}",
        display_path(repo_root, &result.profile_manifest_path)
    );
    for capability in &result.capabilities {
        println!(
            "capability\t{}\t{}",
            capability.id,
            display_path(repo_root, &capability.manifest_path)
        );
    }
    for skipped in &result.skipped_files {
        println!("skipped\t{}\t{}", skipped.path.display(), skipped.reason);
    }
    if let Some(index_path) = &result.index_path {
        println!("index\t{}", index_path.display());
    }
    if result.status == RuntimeHomeImportStatus::DryRun {
        println!("next\tRun again with --write to create these files");
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
