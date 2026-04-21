//! Noun first command surface integration tests (ALP-1920).
//!
//! Asserts the clap tree shape, `--json` availability on inspect commands,
//! clap error handling for unknown commands, and stub behavior for the
//! not yet implemented verbs.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("agent-matters").expect("cargo bin available in tests")
}

fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent-matters-capabilities/tests/fixtures")
        .join(relative)
}

#[test]
fn profiles_help_lists_all_verbs() {
    bin()
        .args(["profiles", "--help"])
        .assert()
        .success()
        .stdout(contains("list"))
        .stdout(contains("show"))
        .stdout(contains("compile"))
        .stdout(contains("use"));
}

#[test]
fn capabilities_help_lists_all_verbs() {
    bin()
        .args(["capabilities", "--help"])
        .assert()
        .success()
        .stdout(contains("list"))
        .stdout(contains("show"))
        .stdout(contains("diff"));
}

#[test]
fn sources_help_lists_all_verbs() {
    bin()
        .args(["sources", "--help"])
        .assert()
        .success()
        .stdout(contains("search"))
        .stdout(contains("import"));
}

#[test]
fn profiles_list_advertises_json_flag() {
    bin()
        .args(["profiles", "list", "--help"])
        .assert()
        .success()
        .stdout(contains("--json"));
}

#[test]
fn profiles_list_help_uses_generated_text_and_examples() {
    bin()
        .args(["profiles", "list", "--help"])
        .assert()
        .success()
        .stdout(contains("Each profile line shows the profile id"))
        .stdout(contains("agent-matters profiles list --json"));
}

#[test]
fn capabilities_show_advertises_json_flag() {
    bin()
        .args(["capabilities", "show", "--help"])
        .assert()
        .success()
        .stdout(contains("--json"));
}

#[test]
fn sources_search_advertises_json_flag() {
    bin()
        .args(["sources", "search", "--help"])
        .assert()
        .success()
        .stdout(contains("--json"));
}

#[test]
fn sources_import_help_uses_generated_text_and_examples() {
    bin()
        .args(["sources", "import", "--help"])
        .assert()
        .success()
        .stdout(contains("vendor record plus an empty overlay"))
        .stdout(contains("skills.sh://author/name@1.2.0"));
}

#[test]
fn doctor_advertises_json_flag() {
    bin()
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(contains("--json"));
}

#[test]
fn profiles_compile_accepts_runtime_value() {
    bin()
        .args(["profiles", "compile", "--help"])
        .assert()
        .success()
        .stdout(contains("--runtime"))
        .stdout(contains("codex"))
        .stdout(contains("claude"));
}

#[test]
fn unknown_top_level_command_is_clap_error() {
    bin()
        .arg("bogus")
        .assert()
        .failure()
        .code(2)
        .stderr(contains("unrecognized subcommand"));
}

#[test]
fn unknown_nested_command_is_clap_error() {
    bin().args(["profiles", "bogus"]).assert().failure().code(2);
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
fn capabilities_list_reads_generated_index_as_json() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["capabilities", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"skill:playwright\""));
}

#[test]
fn capabilities_list_human_includes_provenance_and_summary() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/imported-overlaid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["capabilities", "list"])
        .assert()
        .success()
        .stdout(contains(
            "skill:playwright\tskill\tclaude,codex\toverlaid external:skills.sh/playwright@1.0.0\tLocal Playwright skill overlay.",
        ));
}

#[test]
fn capabilities_show_renders_overlay_details() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/imported-overlaid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["capabilities", "show", "skill:playwright"])
        .assert()
        .success()
        .stdout(contains("Capability: skill:playwright"))
        .stdout(contains("overlay state: overlaid"))
        .stdout(contains("vendor: vendor/skills.sh/playwright"))
        .stdout(contains("source\tSKILL.md"));
}

#[test]
fn capabilities_show_json_includes_record_details() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["capabilities", "show", "skill:playwright", "--json"])
        .assert()
        .success()
        .stdout(contains("\"record\""))
        .stdout(contains("\"files\""))
        .stdout(contains("\"source\": \"SKILL.md\""));
}

#[test]
fn capabilities_show_missing_id_exits_with_actionable_error() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["capabilities", "show", "skill:missing"])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("capability.show-not-found"))
        .stderr(contains("exact capability ids"));
}

#[test]
fn capabilities_diff_reports_overlay_changes() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/imported-overlaid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["capabilities", "diff", "skill:playwright"])
        .assert()
        .success()
        .stdout(contains("Capability overlay diff: skill:playwright"))
        .stdout(contains("changed\tmanifest.toml"));
}

#[test]
fn remaining_not_implemented_verbs_fail_with_clear_message() {
    bin()
        .args(["profiles", "show", "github-researcher"])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("not yet implemented"));
}

#[test]
fn completions_bash_emits_script() {
    bin()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("_agent-matters"));
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
