use std::fs;

use predicates::str::contains;
use tempfile::TempDir;

use crate::common::{bin, write_fake_skills_bin};

#[test]
fn sources_search_renders_mocked_skills_results() {
    let tools = TempDir::new().unwrap();
    let skills_bin = write_fake_skills_bin(&tools);

    bin()
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "search", "skills.sh", "playwright"])
        .assert()
        .success()
        .stdout(contains("owner/repo@playwright"))
        .stdout(contains("2 installs"));
}

#[test]
fn sources_search_json_renders_mocked_skills_results() {
    let tools = TempDir::new().unwrap();
    let skills_bin = write_fake_skills_bin(&tools);

    bin()
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "search", "skills.sh", "playwright", "--json"])
        .assert()
        .success()
        .stdout(contains("\"source\": \"skills.sh\""))
        .stdout(contains("\"locator\": \"owner/repo@playwright\""));
}

#[test]
fn sources_search_failure_emits_diagnostic() {
    bin()
        .args(["sources", "search", "unknown-source", "playwright"])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("source.search-failed"))
        .stderr(contains("unsupported source"));
}

#[test]
fn sources_import_writes_catalog_vendor_and_index() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let tools = TempDir::new().unwrap();
    let skills_bin = write_fake_skills_bin(&tools);

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_DIR", state.path())
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "import", "skills.sh:owner/repo@playwright"])
        .assert()
        .success()
        .stdout(contains("Imported skill:playwright"))
        .stdout(contains(
            "manifest\tcatalog/skills/playwright/manifest.toml",
        ))
        .stdout(contains("vendor\tvendor/skills.sh/owner/repo@playwright"));

    assert!(
        state
            .path()
            .join("catalog/skills/playwright/manifest.toml")
            .exists()
    );
    assert!(
        state
            .path()
            .join("vendor/skills.sh/owner/repo@playwright/record.json")
            .exists()
    );
    assert!(state.path().join("indexes/catalog.json").exists());
    assert!(!repo.path().join("catalog").exists());
    assert!(!repo.path().join("vendor").exists());
}

#[test]
fn sources_import_is_idempotent_and_update_refreshes_existing_import() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let tools = TempDir::new().unwrap();
    let skills_bin = write_fake_skills_bin(&tools);
    let locator = "skills.sh:owner/repo@playwright";

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_DIR", state.path())
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "import", locator])
        .assert()
        .success()
        .stdout(contains("Imported skill:playwright"));

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_DIR", state.path())
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "import", locator])
        .assert()
        .success()
        .stdout(contains("Already up to date skill:playwright"));

    fs::write(
        state.path().join("catalog/skills/playwright/SKILL.md"),
        "# Local Playwright\n",
    )
    .unwrap();

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_DIR", state.path())
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "import", locator])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("source.import-conflict"))
        .stderr(contains("--update"));

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_DIR", state.path())
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args(["sources", "import", locator, "--update"])
        .assert()
        .success()
        .stdout(contains("Updated skill:playwright"));
    assert_eq!(
        fs::read_to_string(state.path().join("catalog/skills/playwright/SKILL.md")).unwrap(),
        "---\nname: playwright\ndescription: Mock Playwright skill.\nmetadata:\n  version: \"2.0.0\"\n---\n\n# Playwright\n"
    );
    assert!(!repo.path().join("catalog").exists());
    assert!(!repo.path().join("vendor").exists());
}

#[test]
fn sources_import_json_reports_policy_diagnostic() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let tools = TempDir::new().unwrap();
    let skills_bin = write_fake_skills_bin(&tools);
    fs::write(
        state.path().join("config.toml"),
        r#"
        [source_trust.sources."skills.sh"]
        kinds = ["mcp"]
        "#,
    )
    .unwrap();

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_DIR", state.path())
        .env("AGENT_MATTERS_SKILLS_BIN", &skills_bin)
        .args([
            "sources",
            "import",
            "skills.sh:owner/repo@playwright",
            "--json",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("\"diagnostics\""))
        .stdout(contains("\"code\": \"source.trust-blocked\""))
        .stdout(contains("skills.sh"))
        .stdout(contains("skill"));

    assert!(!repo.path().join("catalog").exists());
    assert!(!repo.path().join("vendor").exists());
    assert!(!state.path().join("catalog").exists());
    assert!(!state.path().join("vendor").exists());
}
