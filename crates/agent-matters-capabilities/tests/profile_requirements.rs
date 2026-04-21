mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    ProfileRequirementValidationMode, RequirementPresence, ResolveProfileRequest,
    validate_profile_requirements,
};
use agent_matters_core::domain::{DiagnosticSeverity, EnvVarPresence};
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

fn resolve_and_validate(
    repo: &TempDir,
    env: BTreeMap<String, String>,
    mode: ProfileRequirementValidationMode,
) -> agent_matters_capabilities::profiles::ProfileRequirementValidationResult {
    let state = TempDir::new().unwrap();
    let resolved = agent_matters_capabilities::profiles::resolve_profile(ResolveProfileRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
    })
    .unwrap();

    validate_profile_requirements(&resolved, &env, mode)
}

fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

fn append_requires(repo: &TempDir, manifest: &str, body: &str) {
    let path = repo.path().join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
}

#[test]
fn required_capability_present_reports_present_check() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "capabilities = [\"skill:playwright\"]\n",
    );

    let result = resolve_and_validate(
        &repo,
        BTreeMap::new(),
        ProfileRequirementValidationMode::Compile,
    );

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.capability_checks.len(), 1);
    assert_eq!(result.capability_checks[0].capability, "skill:playwright");
    assert_eq!(result.capability_checks[0].required_by, "mcp:linear");
    assert_eq!(
        result.capability_checks[0].status,
        RequirementPresence::Present
    );
}

#[test]
fn required_capability_missing_is_error_with_requiring_capability() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "capabilities = [\"mcp:context-matters\"]\n",
    );

    let result = resolve_and_validate(
        &repo,
        BTreeMap::new(),
        ProfileRequirementValidationMode::Compile,
    );
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "profile.required-capability-missing")
        .unwrap();

    assert!(result.has_error_diagnostics());
    assert_eq!(
        result.capability_checks[0].status,
        RequirementPresence::Missing
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(diagnostic.message.contains("mcp:linear"));
    assert!(diagnostic.message.contains("mcp:context-matters"));
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("requires.capabilities")
    );
}

#[test]
fn required_env_present_reports_status_without_value() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );
    let env = BTreeMap::from([(
        "LINEAR_API_KEY".to_string(),
        "secret-value-never-rendered".to_string(),
    )]);

    let result = resolve_and_validate(&repo, env, ProfileRequirementValidationMode::Use);
    let encoded = serde_json::to_string(&result).unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.env_checks.len(), 1);
    assert_eq!(result.env_checks[0].name, "LINEAR_API_KEY");
    assert_eq!(result.env_checks[0].required_by, "mcp:linear");
    assert_eq!(result.env_checks[0].status, EnvVarPresence::Present);
    assert!(encoded.contains("LINEAR_API_KEY"));
    assert!(!encoded.contains("secret-value-never-rendered"));
}

#[test]
fn required_env_missing_during_compile_is_warning() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );

    let result = resolve_and_validate(
        &repo,
        BTreeMap::new(),
        ProfileRequirementValidationMode::Compile,
    );

    assert!(!result.has_error_diagnostics());
    assert_eq!(result.env_checks[0].status, EnvVarPresence::Missing);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "profile.required-env-missing");
}

#[test]
fn required_env_missing_during_use_is_error() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );

    let result = resolve_and_validate(
        &repo,
        BTreeMap::new(),
        ProfileRequirementValidationMode::Use,
    );

    assert!(result.has_error_diagnostics());
    assert_eq!(result.env_checks[0].status, EnvVarPresence::Missing);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "profile.required-env-missing");
}

#[test]
fn dependency_validation_uses_overlaid_effective_capability() {
    let repo = valid_repo();
    let overlay_dir = repo.path().join("overlays/skills/playwright");
    fs::create_dir_all(&overlay_dir).unwrap();
    let base = fs::read_to_string(
        repo.path()
            .join("catalog/skills/renamed-skill-dir/manifest.toml"),
    )
    .unwrap();
    fs::write(
        overlay_dir.join("manifest.toml"),
        format!("{base}\n[requires]\ncapabilities = [\"mcp:linear\"]\n"),
    )
    .unwrap();

    let result = resolve_and_validate(
        &repo,
        BTreeMap::new(),
        ProfileRequirementValidationMode::Compile,
    );

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.capability_checks.len(), 1);
    assert_eq!(result.capability_checks[0].capability, "mcp:linear");
    assert_eq!(result.capability_checks[0].required_by, "skill:playwright");
    assert_eq!(
        result.capability_checks[0].status,
        RequirementPresence::Present
    );
}
