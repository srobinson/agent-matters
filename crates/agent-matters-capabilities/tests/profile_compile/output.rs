use std::collections::BTreeMap;
use std::fs;

use agent_matters_capabilities::profiles::compile_profile_build;
use serde_json::Value;
use tempfile::TempDir;

use crate::common::{add_required_env, compile, compile_request, file_snapshot, valid_repo};

#[test]
fn compile_does_not_mutate_authored_catalog_files() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let before = file_snapshot(repo.path());

    let _ = compile(repo.path(), state.path());

    assert_eq!(file_snapshot(repo.path()), before);
}

#[test]
fn compile_json_output_does_not_include_env_values() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    add_required_env(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "LINEAR_API_KEY",
    );
    let mut request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.clone().unwrap();
    fs::write(
        native_home.join(".codex/auth.json"),
        br#"{"token":"credential-secret-never-rendered"}"#,
    )
    .unwrap();
    request.env = BTreeMap::from([(
        "LINEAR_API_KEY".to_string(),
        "secret-value-never-rendered".to_string(),
    )]);

    let result = compile_profile_build(request).unwrap();
    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(encoded["profile"], "github-researcher");
    assert_eq!(encoded["build"]["runtime"], "codex");
    assert_eq!(
        encoded["build"]["credential_symlinks"][0]["target_path"],
        "auth.json"
    );
    assert!(
        encoded["build"]["credential_symlinks"][0]["source_path"]
            .as_str()
            .is_some()
    );
    assert_eq!(encoded["diagnostics"], Value::Array(Vec::new()));
    assert!(!encoded.to_string().contains("secret-value-never-rendered"));
    assert!(
        !encoded
            .to_string()
            .contains("credential-secret-never-rendered")
    );
}
