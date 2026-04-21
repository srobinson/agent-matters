use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    ResolveProfileRequest, ResolveProfileResult, resolve_profile,
};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

mod support;

fn copy_dir(src: &Path, dest: &Path) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dest.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, body).unwrap();
}

fn valid_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    copy_dir(&support::fixture_path("catalogs/valid"), tmp.path());
    tmp
}

fn set_profile_runtimes(repo: &Path, runtimes: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    let prefix = body.split("[runtimes.codex]").next().unwrap();
    fs::write(path, format!("{prefix}{runtimes}")).unwrap();
}

fn resolve(repo: &Path, state: &Path) -> ResolveProfileResult {
    resolve_profile(ResolveProfileRequest {
        repo_root: repo.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        profile: "github-researcher".to_string(),
    })
    .unwrap()
}

fn runtime_model<'a>(result: &'a ResolveProfileResult, runtime: &str) -> Option<&'a str> {
    result
        .runtime_configs
        .iter()
        .find(|config| config.id == runtime)
        .and_then(|config| config.model.as_deref())
}

fn has_code(result: &ResolveProfileResult, code: &str) -> bool {
    result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == code)
}

#[test]
fn runtime_precedence_applies_all_default_layers_and_profile_override() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    write(
        repo.path(),
        "defaults/runtimes.toml",
        r#"
        [runtimes.codex]
        model = "repo-model"
        "#,
    );
    write(
        state.path(),
        "config.toml",
        r#"
        [runtimes.codex]
        model = "user-model"
        "#,
    );

    let user_result = resolve(repo.path(), state.path());
    assert_eq!(runtime_model(&user_result, "codex"), Some("user-model"));

    write(
        repo.path(),
        "catalog/runtime-settings/codex-defaults/config.toml",
        r#"
        [runtimes.codex]
        model = "capability-model"
        "#,
    );
    let capability_result = resolve(repo.path(), state.path());
    assert_eq!(
        runtime_model(&capability_result, "codex"),
        Some("capability-model")
    );

    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.codex]
enabled = true
model = "profile-model"
"#,
    );
    let profile_result = resolve(repo.path(), state.path());
    assert_eq!(
        runtime_model(&profile_result, "codex"),
        Some("profile-model")
    );
    assert_eq!(profile_result.selected_runtime.as_deref(), Some("codex"));
    assert_eq!(profile_result.diagnostics, Vec::new());
}

#[test]
fn single_enabled_runtime_is_selected_without_default() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime.as_deref(), Some("codex"));
    assert_eq!(
        result
            .runtime_configs
            .iter()
            .map(|config| config.id.as_str())
            .collect::<Vec<_>>(),
        vec!["codex"]
    );
}

#[test]
fn profile_default_wins_over_user_default() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    write(state.path(), "config.toml", r#"default_runtime = "claude""#);
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes]
default = "codex"

[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = true
"#,
    );

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime.as_deref(), Some("codex"));
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn ambiguous_runtime_without_default_is_error() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = true
"#,
    );

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "profile.runtime.ambiguous-default")
        .expect("ambiguous runtime diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(diagnostic.message.contains("codex"));
    assert!(diagnostic.message.contains("claude"));
}

#[test]
fn disabled_runtime_block_is_ignored() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = false
model = "ignored"
"#,
    );

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime.as_deref(), Some("codex"));
    assert_eq!(runtime_model(&result, "claude"), None);
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn unknown_enabled_runtime_is_diagnostic() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.zed]
enabled = true
"#,
    );

    let result = resolve(repo.path(), state.path());

    assert!(has_code(&result, "profile.runtime.unknown"));
    assert_eq!(result.selected_runtime, None);
}
