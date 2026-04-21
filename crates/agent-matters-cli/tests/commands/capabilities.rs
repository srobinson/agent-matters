use predicates::str::contains;
use tempfile::TempDir;

use crate::common::{bin, fixture_path};

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
