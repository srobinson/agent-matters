//! `agent-matters doctor` subcommand.

use std::path::PathBuf;

use agent_matters_capabilities::doctor::{DoctorRequest, DoctorResult, run_doctor};
use agent_matters_core::domain::render_diagnostics_human;

use super::default_catalog_paths;

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

    println!();
    print!("{}", render_diagnostics_human(&result.diagnostics));
}

fn native_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
