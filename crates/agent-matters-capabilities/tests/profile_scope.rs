mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    ProfileScopeValidationRequest, ProfileScopeValidationStatus, ProfileUseScopeValidationRequest,
    ResolveProfileRequest, validate_profile_scope, validate_profile_use_scope,
};
use agent_matters_core::domain::DiagnosticSeverity;
use serde_json::json;
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

fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

fn set_scope(repo: &TempDir, scope: &str) {
    let path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let mut body = fs::read_to_string(&path).unwrap();
    body.push_str("\n[scope]\n");
    body.push_str(scope);
    fs::write(path, body).unwrap();
}

fn resolve(repo: &TempDir) -> agent_matters_capabilities::profiles::ResolveProfileResult {
    let state = TempDir::new().unwrap();
    agent_matters_capabilities::profiles::resolve_profile(ResolveProfileRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
    })
    .unwrap()
}

fn validate(
    repo: &TempDir,
    workspace: &Path,
) -> agent_matters_capabilities::profiles::ProfileScopeValidationResult {
    let resolved = resolve(repo);
    validate_profile_scope(ProfileScopeValidationRequest {
        resolved: &resolved,
        repo_root: repo.path().to_path_buf(),
        workspace_path: workspace.to_path_buf(),
    })
}

#[test]
fn path_in_scope_passes() {
    let repo = valid_repo();
    let allowed = repo.path().join("allowed");
    let workspace = allowed.join("nested");
    fs::create_dir_all(&workspace).unwrap();
    set_scope(
        &repo,
        r#"paths = ["allowed"]
enforcement = "fail"
"#,
    );

    let result = validate(&repo, &workspace);

    assert_eq!(result.status, ProfileScopeValidationStatus::InScope);
    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.matched_scope.unwrap().value, "allowed");
}

#[test]
fn path_out_of_scope_warns() {
    let repo = valid_repo();
    fs::create_dir_all(repo.path().join("allowed")).unwrap();
    let workspace = repo.path().join("outside");
    fs::create_dir_all(&workspace).unwrap();
    set_scope(
        &repo,
        r#"paths = ["allowed"]
enforcement = "warn"
"#,
    );

    let result = validate(&repo, &workspace);
    let diagnostic = result.diagnostics.first().unwrap();

    assert_eq!(result.status, ProfileScopeValidationStatus::OutOfScope);
    assert!(!result.has_error_diagnostics());
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.code, "profile.scope.out-of-scope");
    assert!(diagnostic.message.contains("github-researcher"));
    assert!(diagnostic.message.contains(workspace.to_str().unwrap()));
    assert!(diagnostic.message.contains("allowed"));
}

#[test]
fn path_out_of_scope_fails() {
    let repo = valid_repo();
    fs::create_dir_all(repo.path().join("allowed")).unwrap();
    let workspace = repo.path().join("outside");
    fs::create_dir_all(&workspace).unwrap();
    set_scope(
        &repo,
        r#"paths = ["allowed"]
enforcement = "fail"
"#,
    );

    let result = validate(&repo, &workspace);

    assert_eq!(result.status, ProfileScopeValidationStatus::OutOfScope);
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
}

#[test]
fn scope_enforcement_none_records_no_warning_or_failure() {
    let repo = valid_repo();
    fs::create_dir_all(repo.path().join("allowed")).unwrap();
    let workspace = repo.path().join("outside");
    fs::create_dir_all(&workspace).unwrap();
    set_scope(
        &repo,
        r#"paths = ["allowed"]
enforcement = "none"
"#,
    );

    let result = validate(&repo, &workspace);

    assert_eq!(result.status, ProfileScopeValidationStatus::NotEnforced);
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn github_repo_scope_passes_when_origin_matches() {
    let repo = valid_repo();
    let workspace = repo.path().join("project");
    let git_dir = workspace.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        r#"
[remote "origin"]
    url = git@github.com:srobinson/helioy.git
"#,
    )
    .unwrap();
    set_scope(
        &repo,
        r#"github_repos = ["srobinson/helioy"]
enforcement = "fail"
"#,
    );

    let result = validate(&repo, &workspace);

    assert_eq!(result.status, ProfileScopeValidationStatus::InScope);
    assert_eq!(
        result.detected_github_repo.as_deref(),
        Some("srobinson/helioy")
    );
    let matched = result.matched_scope.unwrap();
    assert_eq!(matched.kind, "github_repo");
    assert_eq!(matched.value, "srobinson/helioy");
}

#[test]
fn non_origin_github_remote_does_not_satisfy_repo_scope() {
    let repo = valid_repo();
    let workspace = repo.path().join("project");
    let git_dir = workspace.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        r#"
[remote "origin"]
    url = git@gitlab.com:srobinson/helioy.git
[remote "upstream"]
    url = git@github.com:srobinson/helioy.git
"#,
    )
    .unwrap();
    set_scope(
        &repo,
        r#"github_repos = ["srobinson/helioy"]
enforcement = "fail"
"#,
    );

    let result = validate(&repo, &workspace);

    assert_eq!(result.status, ProfileScopeValidationStatus::OutOfScope);
    assert_eq!(result.detected_github_repo, None);
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
}

#[test]
fn omitted_use_path_defaults_to_current_working_directory() {
    let repo = valid_repo();
    let cwd = std::env::current_dir().unwrap();
    set_scope(
        &repo,
        &format!(
            "paths = [\"{}\"]\nenforcement = \"fail\"\n",
            cwd.to_string_lossy()
        ),
    );
    let resolved = resolve(&repo);

    let result = validate_profile_use_scope(ProfileUseScopeValidationRequest {
        resolved: &resolved,
        repo_root: repo.path().to_path_buf(),
        workspace_path: None,
    });

    assert_eq!(result.status, ProfileScopeValidationStatus::InScope);
    let canonical_cwd = cwd.canonicalize().unwrap();
    assert_eq!(
        result.canonical_path.as_deref(),
        Some(canonical_cwd.to_str().unwrap())
    );
}

#[test]
fn nonexistent_path_reports_diagnostic() {
    let repo = valid_repo();
    fs::create_dir_all(repo.path().join("allowed")).unwrap();
    let workspace = repo.path().join("missing");
    set_scope(
        &repo,
        r#"paths = ["allowed"]
enforcement = "fail"
"#,
    );

    let result = validate(&repo, &workspace);

    assert_eq!(result.status, ProfileScopeValidationStatus::PathMissing);
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics[0].code, "profile.scope.path-not-found");
    assert!(result.diagnostics[0].message.contains("github-researcher"));
    assert!(result.diagnostics[0].message.contains("missing"));
}

#[test]
fn scope_validation_json_includes_status_path_and_allowed_scopes() {
    let repo = valid_repo();
    fs::create_dir_all(repo.path().join("allowed")).unwrap();
    let workspace = repo.path().join("outside");
    fs::create_dir_all(&workspace).unwrap();
    set_scope(
        &repo,
        r#"paths = ["allowed"]
github_repos = ["srobinson/helioy"]
enforcement = "warn"
"#,
    );

    let result = validate(&repo, &workspace);
    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(
        encoded,
        json!({
            "profile": "github-researcher",
            "requested_path": workspace.to_string_lossy(),
            "canonical_path": workspace.canonicalize().unwrap().to_string_lossy(),
            "scope": {
                "paths": ["allowed"],
                "github_repos": ["srobinson/helioy"],
                "enforcement": "warn"
            },
            "status": "out-of-scope",
            "diagnostics": [{
                "severity": "warning",
                "code": "profile.scope.out-of-scope",
                "message": format!(
                    "profile `github-researcher` is not scoped for path `{}`; allowed scopes: paths [allowed], github_repos [srobinson/helioy]",
                    workspace.to_string_lossy()
                ),
                "location": {
                    "manifest_path": "profiles/renamed-profile-dir/manifest.toml",
                    "field": "scope"
                },
                "recovery_hint": "use a path inside the allowed scope or update the profile manifest"
            }]
        })
    );
}
