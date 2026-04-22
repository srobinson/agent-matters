use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

use crate::common::bin;

#[test]
fn completions_bash_emits_script() {
    bin()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("_agent-matters"))
        .stdout(contains("profiles capabilities sources doctor completions help").not())
        .stdout(contains("list show resolve compile use help").not());
}
