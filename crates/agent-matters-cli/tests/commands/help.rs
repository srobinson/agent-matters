use predicates::str::contains;

use crate::common::bin;

#[test]
fn profiles_help_lists_all_verbs() {
    bin()
        .args(["profiles", "--help"])
        .assert()
        .success()
        .stdout(contains("list"))
        .stdout(contains("show"))
        .stdout(contains("resolve"))
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
        .stdout(contains("preserves raw source material under `vendor`"))
        .stdout(contains("skills.sh:owner/repo@skill-name"))
        .stdout(contains("--json"));
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
        .stdout(contains("--json"))
        .stdout(contains("codex"))
        .stdout(contains("claude"));
}

#[test]
fn profiles_resolve_accepts_runtime_and_json_flags() {
    bin()
        .args(["profiles", "resolve", "--help"])
        .assert()
        .success()
        .stdout(contains("--runtime"))
        .stdout(contains("--json"))
        .stdout(contains("session cache"))
        .stdout(contains("agent-matters profiles resolve"));
}

#[test]
fn profiles_use_advertises_runtime_and_json_flags() {
    bin()
        .args(["profiles", "use", "--help"])
        .assert()
        .success()
        .stdout(contains("--runtime"))
        .stdout(contains("--json"))
        .stdout(contains("Defaults through profile"))
        .stdout(contains("agent-matters profiles use my-profile"));
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
