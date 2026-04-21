mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    ResolveProfileRequest, ResolveProfileResult, resolve_profile,
};
use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use tempfile::TempDir;

use support::fixture_path;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn resolve_fixture(relative: &str, profile: &str) -> ResolveProfileResult {
    let state = TempDir::new().unwrap();
    resolve_profile(ResolveProfileRequest {
        repo_root: fixture_path(relative),
        user_state_dir: state.path().to_path_buf(),
        profile: profile.to_string(),
    })
    .unwrap()
}

fn resolve_temp(repo: &TempDir, profile: &str) -> ResolveProfileResult {
    let state = TempDir::new().unwrap();
    resolve_profile(ResolveProfileRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: profile.to_string(),
    })
    .unwrap()
}

fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn profile_resolves_capabilities_and_ordered_instruction_fragments() {
    let result = resolve_fixture("catalogs/valid", "github-researcher");

    let capability_ids = result
        .effective_capabilities
        .iter()
        .map(|record| record.id.as_str())
        .collect::<Vec<_>>();
    let instruction_ids = result
        .instruction_fragments
        .iter()
        .map(|fragment| fragment.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        capability_ids,
        vec![
            "skill:playwright",
            "mcp:linear",
            "hook:session-logger",
            "runtime-setting:codex-defaults",
            "instruction:helioy-core",
            "agent:github-researcher",
        ]
    );
    assert_eq!(
        instruction_ids,
        vec!["instruction:helioy-core", "agent:github-researcher"]
    );
    assert_eq!(
        result.instruction_fragments[0].source_path,
        "catalog/instructions/helioy-core"
    );
    assert_eq!(
        result.instruction_fragments[1]
            .files
            .get("instructions")
            .map(String::as_str),
        Some("AGENT.md")
    );
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn instruction_entries_are_effective_without_capability_repetition() {
    let result = resolve_fixture("catalogs/valid", "github-researcher");

    assert!(
        result
            .effective_capabilities
            .iter()
            .any(|record| record.id == "instruction:helioy-core")
    );
    assert!(
        result
            .effective_capabilities
            .iter()
            .any(|record| record.id == "agent:github-researcher")
    );
}

#[test]
fn missing_instruction_capability_is_structured_error() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("agent:github-researcher", "agent:missing");
    fs::write(&manifest_path, updated).unwrap();

    let result = resolve_temp(&repo, "github-researcher");

    assert!(result.has_error_diagnostics());
    assert!(has_code(
        &result.diagnostics,
        "profile.instruction-not-found"
    ));
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "profile.instruction-not-found")
            .count(),
        1
    );
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "profile.instruction-not-found")
        .unwrap();
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("instructions")
    );
}

#[test]
fn duplicate_capability_references_are_reported_and_deduplicated() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path).unwrap().replace(
        "\"skill:playwright\",\n  \"mcp:linear\",",
        "\"skill:playwright\",\n  \"skill:playwright\",\n  \"mcp:linear\",",
    );
    fs::write(&manifest_path, updated).unwrap();

    let result = resolve_temp(&repo, "github-researcher");
    let playwright_count = result
        .effective_capabilities
        .iter()
        .filter(|record| record.id == "skill:playwright")
        .count();

    assert_eq!(playwright_count, 1);
    assert!(has_code(
        &result.diagnostics,
        "profile.duplicate-capability-reference"
    ));
}

#[test]
fn duplicate_instruction_references_are_error_diagnostics() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace(
            "\"instruction:helioy-core\",\n  \"agent:github-researcher\",",
            "\"instruction:helioy-core\",\n  \"instruction:helioy-core\",\n  \"agent:github-researcher\",",
        );
    fs::write(&manifest_path, updated).unwrap();

    let result = resolve_temp(&repo, "github-researcher");
    let instruction_count = result
        .instruction_fragments
        .iter()
        .filter(|fragment| fragment.id == "instruction:helioy-core")
        .count();

    assert_eq!(instruction_count, 1);
    assert!(result.has_error_diagnostics());
    assert!(has_code(
        &result.diagnostics,
        "profile.duplicate-instruction-reference"
    ));
}
