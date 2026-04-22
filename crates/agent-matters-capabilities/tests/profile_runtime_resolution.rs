use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    ResolveProfileRequest, ResolveProfileResult, resolve_profile,
};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

mod support;

use support::fixtures::valid_catalog_repo;
use support::manifests::{ProfileRuntimeFixture, set_profile_runtimes};

const PROFILE_MANIFEST: &str = "profiles/renamed-profile-dir/manifest.toml";

fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, body).unwrap();
}

fn valid_repo() -> TempDir {
    valid_catalog_repo()
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
        PROFILE_MANIFEST,
        None,
        &[ProfileRuntimeFixture::enabled_with_model(
            "codex",
            "profile-model",
        )],
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
        PROFILE_MANIFEST,
        Some("codex"),
        &[
            ProfileRuntimeFixture::enabled("codex"),
            ProfileRuntimeFixture::enabled("claude"),
        ],
    );

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime.as_deref(), Some("codex"));
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn profile_default_must_be_enabled_for_the_profile() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        Some("claude"),
        &[ProfileRuntimeFixture::enabled("codex")],
    );

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "profile.runtime.default-unavailable")
        .expect("default unavailable diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(diagnostic.message.contains("profile default runtime"));
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("runtimes.default")
    );
}

#[test]
fn user_default_must_be_enabled_when_profile_runtime_is_ambiguous() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    write(state.path(), "config.toml", r#"default_runtime = "zed""#);
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[
            ProfileRuntimeFixture::enabled("codex"),
            ProfileRuntimeFixture::enabled("claude"),
        ],
    );

    let result = resolve(repo.path(), state.path());

    assert_eq!(result.selected_runtime, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "profile.runtime.default-unavailable")
        .expect("default unavailable diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(diagnostic.message.contains("user default runtime"));
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("default_runtime")
    );
}

#[test]
fn ambiguous_runtime_without_default_is_error() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[
            ProfileRuntimeFixture::enabled("codex"),
            ProfileRuntimeFixture::enabled("claude"),
        ],
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
        PROFILE_MANIFEST,
        None,
        &[
            ProfileRuntimeFixture::enabled("codex"),
            ProfileRuntimeFixture::disabled_with_model("claude", "ignored"),
        ],
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
        PROFILE_MANIFEST,
        None,
        &[ProfileRuntimeFixture::enabled("zed")],
    );

    let result = resolve(repo.path(), state.path());

    assert!(has_code(&result, "profile.runtime.unknown"));
    assert_eq!(result.selected_runtime, None);
}
