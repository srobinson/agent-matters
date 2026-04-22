//! Integration tests that exercise the config loaders against real
//! fixture directories. Proves both the fixture support helper and the
//! capabilities config loader work end to end.

mod support;

use agent_matters_capabilities::config::{load_markers, load_runtime_defaults, load_user_config};

#[test]
fn staged_home_fixture_loads_user_config() {
    let home = support::fixtures::fixture_path("homes/user-with-codex-default");
    let cfg = load_user_config(&home).expect("user config loads");
    assert_eq!(cfg.default_runtime.as_deref(), Some("codex"));
    assert_eq!(
        cfg.runtimes.get("codex").and_then(|r| r.model.as_deref()),
        Some("gpt-5.4")
    );
    assert_eq!(
        cfg.runtimes.get("claude").and_then(|r| r.enabled),
        Some(true)
    );
}

#[test]
fn staged_repo_fixture_loads_runtime_defaults() {
    let repo = support::fixtures::fixture_path("repos/repo-with-defaults");
    let defaults = load_runtime_defaults(&repo).expect("runtime defaults load");
    assert_eq!(
        defaults
            .runtimes
            .get("claude")
            .and_then(|r| r.model.as_deref()),
        Some("claude-sonnet-4.5")
    );
}

#[test]
fn staged_repo_fixture_loads_markers() {
    let repo = support::fixtures::fixture_path("repos/repo-with-defaults");
    let markers = load_markers(&repo).expect("markers load");
    assert!(markers.project_markers.iter().any(|m| m == ".git"));
    assert!(markers.project_markers.iter().any(|m| m == "Cargo.toml"));
}
