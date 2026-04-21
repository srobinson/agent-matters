//! CLI integration smoke test. Covers ALP-1919 acceptance criterion that
//! `agent-matters --help` prints a useful top level help screen.

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
        .stdout(contains("doctor"));
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
fn no_command_exits_non_zero() {
    bin().assert().failure().code(2);
}
