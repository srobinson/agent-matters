use predicates::str::contains;

use crate::common::bin;

#[test]
fn completions_bash_emits_script() {
    bin()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("_agent-matters"));
}
