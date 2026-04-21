//! CLI help surface integration tests.
//!
//! Covers ALP-1919 (smoke help) and ALP-1920 (noun first structure and
//! generated help pattern).

use assert_cmd::Command;
use predicates::str::contains;

fn bin() -> Command {
    Command::cargo_bin("agent-matters").expect("cargo bin available in tests")
}

#[test]
fn help_prints_command_groups() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("profiles"))
        .stdout(contains("capabilities"))
        .stdout(contains("sources"))
        .stdout(contains("doctor"))
        .stdout(contains("completions"));
}

#[test]
fn version_prints_crate_version() {
    bin()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("agent-matters"));
}

#[test]
fn bare_invocation_prints_long_help() {
    bin()
        .assert()
        .success()
        .stdout(contains("profiles"))
        .stdout(contains("capabilities"))
        .stdout(contains("sources"))
        .stdout(contains("doctor"));
}
