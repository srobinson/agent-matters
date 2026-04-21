//! Noun first command surface integration tests (ALP-1920).
//!
//! Asserts the clap tree shape, `--json` availability on inspect commands,
//! clap error handling for unknown commands, and stub behavior for the
//! not yet implemented verbs.

use assert_cmd::Command;
use predicates::str::contains;

fn bin() -> Command {
    Command::cargo_bin("agent-matters").expect("cargo bin available in tests")
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
fn not_implemented_verbs_fail_with_clear_message() {
    bin()
        .args(["profiles", "list"])
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
