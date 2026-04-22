use predicates::str::contains;
use tempfile::TempDir;

use crate::common::{bin, fixture_path};

#[test]
fn profiles_resolve_json_returns_session_cache_profile() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args([
            "profiles",
            "resolve",
            "linear",
            "--runtime",
            "codex",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"kind\": \"jit-profile\""))
        .stdout(contains("\"profile_manifest_path\""))
        .stdout(contains("\"mcp:linear\""));
}

#[test]
fn profiles_list_reads_generated_index() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "list"])
        .assert()
        .success()
        .stdout(contains("github-researcher"));

    assert!(state.path().join("indexes/catalog.json").exists());
}

#[test]
fn profiles_list_json_includes_index_metadata() {
    let state = TempDir::new().unwrap();

    let output = bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["index_status"], "rebuilt-missing");
    assert!(
        json["index_path"]
            .as_str()
            .unwrap()
            .ends_with("catalog.json")
    );
    assert_eq!(json["diagnostics"].as_array().unwrap().len(), 0);
    assert!(
        json["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|profile| profile["id"] == "github-researcher")
    );
}

#[test]
fn profiles_list_reuses_generated_index_as_json() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "list", "--json"])
        .assert()
        .success();

    let output = bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["index_status"], "fresh");
}

#[test]
fn profiles_list_human_includes_scope_and_summary() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "list"])
        .assert()
        .success()
        .stdout(contains(
            "github-researcher\tpersona\tcodex\tnone\tFocused research agent for inspecting GitHub repositories.",
        ));
}

#[test]
fn profiles_show_renders_resolution_details() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "show", "github-researcher"])
        .assert()
        .success()
        .stdout(contains("Profile: github-researcher"))
        .stdout(contains("resolved capabilities:"))
        .stdout(contains(
            "skill:playwright\tskill\tcodex\tcatalog/skills/renamed-skill-dir",
        ))
        .stdout(contains("ordered instructions:"))
        .stdout(contains(
            "instruction:helioy-core\tinstruction\tcatalog/instructions/helioy-core",
        ))
        .stdout(contains("resolved runtime config:"))
        .stdout(contains("codex selected"));
}

#[test]
fn profiles_show_json_includes_resolution_details() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "show", "github-researcher", "--json"])
        .assert()
        .success()
        .stdout(contains("\"profile\": \"github-researcher\""))
        .stdout(contains("\"effective_capabilities\""))
        .stdout(contains("\"instruction_fragments\""))
        .stdout(contains("\"selected_runtime\": \"codex\""));
}

#[test]
fn profiles_show_missing_id_exits_with_actionable_error() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "show", "missing-profile"])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("profile.resolve-not-found"))
        .stderr(contains("exact profile ids"));
}

#[test]
fn profiles_compile_requires_runtime() {
    bin()
        .args(["profiles", "compile", "my-profile"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("--runtime"));
}
