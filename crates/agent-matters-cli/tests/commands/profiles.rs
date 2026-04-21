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
