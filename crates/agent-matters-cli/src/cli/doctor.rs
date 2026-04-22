//! `agent-matters doctor` subcommand.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_matters_capabilities::doctor::{DoctorRequest, DoctorResult, run_doctor};
use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};

use super::{default_catalog_paths, diagnostic_severity};

/// Run all registered doctor checks.
pub fn run(json: bool) -> anyhow::Result<i32> {
    let (repo_root, user_state_dir) = default_catalog_paths()?;
    let result = run_doctor(DoctorRequest {
        repo_root,
        user_state_dir,
        native_home_dir: native_home_dir(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        render_doctor(&result);
    }

    Ok(if result.has_error_diagnostics() { 1 } else { 0 })
}

fn render_doctor(result: &DoctorResult) {
    println!("Doctor: catalog integrity");
    println!(
        "Catalog: {} capabilities, {} profiles",
        result.catalog.capability_count, result.catalog.profile_count
    );
    println!(
        "Index: {:?} ({})",
        result.index.status,
        result.index.path.display()
    );

    if result.diagnostics.is_empty() {
        println!("No issues found");
        return;
    }

    for severity in [
        DiagnosticSeverity::Error,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Info,
    ] {
        let grouped = diagnostics_by_source(&result.diagnostics, severity);
        if grouped.is_empty() {
            continue;
        }
        println!();
        println!("{}:", severity_heading(severity));
        for (source, diagnostics) in grouped {
            println!("  {source}");
            for diagnostic in diagnostics {
                println!("    {}: {}", diagnostic.code, diagnostic.message);
                if let Some(location) = &diagnostic.location
                    && let Some(field) = &location.field
                {
                    println!("      field: {field}");
                }
                if let Some(hint) = &diagnostic.recovery_hint {
                    println!("      hint: {hint}");
                }
            }
        }
    }
}

fn diagnostics_by_source(
    diagnostics: &[Diagnostic],
    severity: DiagnosticSeverity,
) -> BTreeMap<String, Vec<&Diagnostic>> {
    let mut grouped = BTreeMap::<String, Vec<&Diagnostic>>::new();
    for diagnostic in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == severity)
    {
        let source = diagnostic
            .location
            .as_ref()
            .and_then(|location| location.manifest_path.as_ref())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<general>".to_string());
        grouped.entry(source).or_default().push(diagnostic);
    }
    grouped
}

fn severity_heading(severity: DiagnosticSeverity) -> &'static str {
    match diagnostic_severity(severity) {
        "error" => "Errors",
        "warning" => "Warnings",
        "info" => "Info",
        _ => "Diagnostics",
    }
}

fn native_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
