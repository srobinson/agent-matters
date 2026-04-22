use std::fs;

use predicates::str::contains;
use tempfile::TempDir;

use crate::common::bin;

#[test]
fn import_help_uses_top_level_runtime_import_api() {
    bin()
        .args(["import", "--help"])
        .assert()
        .success()
        .stdout(contains("agent-matters import ~/.claude"))
        .stdout(contains("--profile"))
        .stdout(contains("--runtime"))
        .stdout(contains("--write"))
        .stdout(contains("--json"));
}

#[test]
fn import_dry_run_infers_claude_profile_from_path() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let source = repo.path().join(".claude");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("CLAUDE.md"), "# Claude\n").unwrap();

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["import", source.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Dry run runtime profile imported-claude"))
        .stdout(contains("runtime\tclaude"))
        .stdout(contains("profile\tprofiles/imported-claude/manifest.toml"))
        .stdout(contains("next\tRun again with --write"));
}

#[test]
fn import_write_accepts_runtime_and_profile_overrides() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let source = repo.path().join("runtime-home");
    fs::create_dir_all(source.join("skills/review")).unwrap();
    fs::write(source.join("skills/review/SKILL.md"), "# Review\n").unwrap();

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args([
            "import",
            source.to_str().unwrap(),
            "--runtime",
            "codex",
            "--profile",
            "workspace-review",
            "--write",
        ])
        .assert()
        .success()
        .stdout(contains("Imported runtime profile workspace-review"))
        .stdout(contains("runtime\tcodex"));

    assert!(
        repo.path()
            .join("profiles/workspace-review/manifest.toml")
            .exists()
    );
    assert!(
        repo.path()
            .join("catalog/skills/review/manifest.toml")
            .exists()
    );
}
