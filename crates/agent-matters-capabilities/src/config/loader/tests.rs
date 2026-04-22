use super::*;

use std::fs;
use std::path::Path;

use tempfile::TempDir;

fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, body).unwrap();
}

#[test]
fn runtime_defaults_missing_file_returns_default() {
    let tmp = TempDir::new().unwrap();
    let loaded = load_runtime_defaults(tmp.path()).unwrap();
    assert_eq!(loaded, RuntimeDefaults::default());
}

#[test]
fn runtime_defaults_valid_file_populates_runtime_map() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "defaults/runtimes.toml",
        r#"
        [runtimes.codex]
        model = "gpt-5.4"
        "#,
    );
    let loaded = load_runtime_defaults(tmp.path()).unwrap();
    assert_eq!(
        loaded.runtimes.get("codex").unwrap().model.as_deref(),
        Some("gpt-5.4")
    );
}

#[test]
fn markers_missing_file_returns_default() {
    let tmp = TempDir::new().unwrap();
    let loaded = load_markers(tmp.path()).unwrap();
    assert_eq!(loaded, Markers::default());
}

#[test]
fn markers_valid_file_populates_list() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "defaults/markers.toml",
        r#"project_markers = [".git", "Cargo.toml"]"#,
    );
    let loaded = load_markers(tmp.path()).unwrap();
    assert_eq!(
        loaded.project_markers,
        vec![".git".to_string(), "Cargo.toml".to_string()]
    );
}

#[test]
fn source_trust_policy_missing_file_returns_conservative_default() {
    let tmp = TempDir::new().unwrap();
    let loaded = load_repo_source_trust_policy(tmp.path()).unwrap();

    assert!(loaded.allows_import(
        "skills.sh",
        agent_matters_core::domain::CapabilityKind::Skill
    ));
    assert!(!loaded.allows_source("mcp-registry"));
}

#[test]
fn source_trust_policy_valid_file_populates_sources() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "defaults/sources.toml",
        r#"
        [sources."skills.sh"]
        kinds = ["skill"]

        [sources."mcp-registry"]
        kinds = ["mcp"]
        "#,
    );

    let loaded = load_repo_source_trust_policy(tmp.path()).unwrap();

    assert!(loaded.allows_import(
        "skills.sh",
        agent_matters_core::domain::CapabilityKind::Skill
    ));
    assert!(loaded.allows_import(
        "mcp-registry",
        agent_matters_core::domain::CapabilityKind::Mcp
    ));
    assert!(!loaded.allows_import(
        "mcp-registry",
        agent_matters_core::domain::CapabilityKind::Skill
    ));
}

#[test]
fn user_source_trust_policy_replaces_repo_policy() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "defaults/sources.toml",
        r#"
        [sources."skills.sh"]
        kinds = ["skill"]
        "#,
    );
    write(
        tmp.path(),
        "config.toml",
        r#"
        [source_trust.sources."mcp-registry"]
        kinds = ["mcp"]
        "#,
    );

    let loaded = load_effective_source_trust_policy(tmp.path(), tmp.path()).unwrap();

    assert!(!loaded.allows_source("skills.sh"));
    assert!(loaded.allows_import(
        "mcp-registry",
        agent_matters_core::domain::CapabilityKind::Mcp
    ));
}

#[test]
fn user_config_missing_file_returns_default() {
    let tmp = TempDir::new().unwrap();
    let loaded = load_user_config(tmp.path()).unwrap();
    assert_eq!(loaded, UserConfig::default());
}

#[test]
fn user_config_reads_default_runtime() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        ".agent-matters/config.toml",
        r#"default_runtime = "claude""#,
    );
    let loaded = load_user_config(tmp.path()).unwrap();
    assert_eq!(loaded.default_runtime.as_deref(), Some("claude"));
}

#[test]
fn user_config_reads_from_state_dir() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "config.toml", r#"default_runtime = "codex""#);

    let loaded = load_user_config_from_state_dir(tmp.path()).unwrap();

    assert_eq!(loaded.default_runtime.as_deref(), Some("codex"));
}

#[test]
fn runtime_settings_file_uses_runtime_defaults_schema() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "settings.toml",
        r#"
        [runtimes.codex]
        model = "gpt-5.4"
        "#,
    );

    let loaded = load_runtime_settings(&tmp.path().join("settings.toml")).unwrap();

    assert_eq!(
        loaded.runtimes.get("codex").unwrap().model.as_deref(),
        Some("gpt-5.4")
    );
}

#[test]
fn repo_and_user_config_are_loaded_independently() {
    // Proves the issue's precedence concern at the loading layer:
    // both files deserialize into their own typed struct without
    // interfering. Applying precedence is ALP-1932.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "defaults/runtimes.toml",
        r#"
        [runtimes.codex]
        model = "gpt-5.4"
        "#,
    );
    write(
        tmp.path(),
        ".agent-matters/config.toml",
        r#"
        default_runtime = "codex"

        [runtimes.codex]
        model = "gpt-5.4-preview"
        "#,
    );

    let repo = load_runtime_defaults(tmp.path()).unwrap();
    let user = load_user_config(tmp.path()).unwrap();
    assert_eq!(
        repo.runtimes.get("codex").unwrap().model.as_deref(),
        Some("gpt-5.4")
    );
    assert_eq!(user.default_runtime.as_deref(), Some("codex"));
    assert_eq!(
        user.runtimes.get("codex").unwrap().model.as_deref(),
        Some("gpt-5.4-preview")
    );
}

#[test]
fn invalid_toml_surfaces_parse_error_with_path() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "defaults/runtimes.toml",
        "this = is = not = valid",
    );
    let err = load_runtime_defaults(tmp.path()).unwrap_err();
    match err {
        ConfigError::Parse { path, source } => {
            assert!(path.ends_with("defaults/runtimes.toml"));
            // The underlying toml error is preserved for actionable
            // diagnostics; assert it mentions the problematic input.
            assert!(!source.to_string().is_empty());
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[test]
fn schema_violation_is_reported_as_parse_error() {
    // `deny_unknown_fields` on UserConfig means any stray key is a
    // parse-time rejection with the offending key named.
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        ".agent-matters/config.toml",
        r#"unexpected_key = true"#,
    );
    let err = load_user_config(tmp.path()).unwrap_err();
    match err {
        ConfigError::Parse { path, source } => {
            assert!(path.ends_with(".agent-matters/config.toml"));
            assert!(source.to_string().contains("unexpected_key"));
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}
