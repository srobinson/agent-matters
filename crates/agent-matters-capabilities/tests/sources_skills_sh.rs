use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::catalog::CatalogIndexStatus;
use agent_matters_capabilities::sources::{
    CommandOutput, ImportSourceAdapterRequest, ImportSourceError, SkillsShAdapter, SkillsShCommand,
    SourceAdapter, SourceAdapterError, SourceImportRequest, SourceImportStorageError,
    SourceSearchRequest, import_source_from_adapter,
};
use agent_matters_core::domain::Provenance;
use agent_matters_core::manifest::CapabilityManifest;
use serde_json::Value;
use tempfile::TempDir;

#[derive(Clone)]
struct MockSkillsCommand {
    find: CommandOutput,
    add: CommandOutput,
    files: Vec<(PathBuf, String)>,
}

impl MockSkillsCommand {
    fn search(stdout: &str) -> Self {
        Self {
            find: CommandOutput {
                code: 0,
                stdout: stdout.to_string(),
                stderr: String::new(),
            },
            add: success_output(),
            files: skill_files(),
        }
    }

    fn import(files: Vec<(PathBuf, String)>) -> Self {
        Self {
            find: success_output(),
            add: success_output(),
            files,
        }
    }
}

impl SkillsShCommand for MockSkillsCommand {
    fn find(&self, _query: &str) -> io::Result<CommandOutput> {
        Ok(self.find.clone())
    }

    fn add(&self, _package: &str, skill: &str, workdir: &Path) -> io::Result<CommandOutput> {
        let skill_dir = workdir.join(".agents/skills").join(skill);
        for (relative, contents) in &self.files {
            let path = skill_dir.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, contents)?;
        }
        Ok(self.add.clone())
    }
}

#[derive(Clone)]
struct RejectingAddCommand;

impl SkillsShCommand for RejectingAddCommand {
    fn find(&self, _query: &str) -> io::Result<CommandOutput> {
        Ok(success_output())
    }

    fn add(&self, _package: &str, _skill: &str, _workdir: &Path) -> io::Result<CommandOutput> {
        Err(io::Error::other("add should not run for invalid locators"))
    }
}

#[test]
fn skills_sh_search_parses_mocked_npx_success() {
    let adapter = SkillsShAdapter::with_command(MockSkillsCommand::search(
        "currents-dev/playwright-best-practices-skill@playwright-best-practices 30.3K installs\n\
         └ https://skills.sh/currents-dev/playwright-best-practices-skill/playwright-best-practices\n\
         microsoft/playwright-cli@playwright-cli 23.5K installs\n\
         └ https://skills.sh/microsoft/playwright-cli/playwright-cli\n",
    ));

    let result = adapter
        .search(SourceSearchRequest {
            query: "playwright".to_string(),
        })
        .unwrap();

    assert_eq!(result.source, "skills.sh");
    assert_eq!(result.entries.len(), 2);
    assert_eq!(
        result.entries[0].locator,
        "currents-dev/playwright-best-practices-skill@playwright-best-practices"
    );
    assert_eq!(result.entries[0].summary.as_deref(), Some("30.3K installs"));
    assert_eq!(
        result.entries[0].raw["url"],
        "https://skills.sh/currents-dev/playwright-best-practices-skill/playwright-best-practices"
    );
}

#[test]
fn skills_sh_search_handles_no_results() {
    let adapter = SkillsShAdapter::with_command(MockSkillsCommand::search(
        "No skills found for \"zzz-unlikely-query\"",
    ));

    let result = adapter
        .search(SourceSearchRequest {
            query: "zzz-unlikely-query".to_string(),
        })
        .unwrap();

    assert!(result.entries.is_empty());
}

#[test]
fn skills_sh_search_reports_command_failure() {
    let mut command = MockSkillsCommand::search("");
    command.find = CommandOutput {
        code: 1,
        stdout: String::new(),
        stderr: "npm registry unavailable".to_string(),
    };
    let adapter = SkillsShAdapter::with_command(command);

    let err = adapter
        .search(SourceSearchRequest {
            query: "playwright".to_string(),
        })
        .unwrap_err();

    assert!(matches!(err, SourceAdapterError::SearchFailed { .. }));
    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.code, "source.search-failed");
    assert!(diagnostic.message.contains("npm registry unavailable"));
}

#[test]
fn skills_sh_search_reports_malformed_output() {
    let adapter = SkillsShAdapter::with_command(MockSkillsCommand::search(
        "Install with npx skills add <owner/repo@skill>\n",
    ));

    let err = adapter
        .search(SourceSearchRequest {
            query: "playwright".to_string(),
        })
        .unwrap_err();

    assert!(matches!(err, SourceAdapterError::InvalidRecord { .. }));
    assert_eq!(err.to_diagnostic().code, "source.record-invalid");
}

#[test]
fn skills_sh_import_rejects_unsafe_locator_before_command() {
    for locator in ["../repo@playwright", "owner/repo@play_wright"] {
        let adapter = SkillsShAdapter::with_command(RejectingAddCommand);

        let err = adapter
            .import_capability(SourceImportRequest {
                locator: locator.to_string(),
            })
            .unwrap_err();

        assert!(matches!(err, SourceAdapterError::InvalidRecord { .. }));
    }
}

#[test]
fn skills_sh_import_writes_vendor_catalog_and_refreshes_index() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let adapter = SkillsShAdapter::with_command(MockSkillsCommand::import(skill_files()));

    let result = import_source_from_adapter(ImportSourceAdapterRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        locator: "owner/repo@playwright".to_string(),
        replace_existing: false,
        adapter: &adapter,
    })
    .unwrap();

    assert_eq!(result.capability_id, "skill:playwright");
    assert_eq!(result.index_status, CatalogIndexStatus::RebuiltMissing);
    assert!(result.index_path.exists());

    let manifest_path = repo.path().join("catalog/skills/playwright/manifest.toml");
    let manifest: CapabilityManifest =
        toml::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(
        manifest.origin,
        Some(Provenance::external(
            "skills.sh",
            "owner/repo@playwright",
            Some("2.0.0".to_string())
        ))
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("catalog/skills/playwright/SKILL.md")).unwrap(),
        skill_files()[0].1
    );

    let vendor_dir = repo.path().join("vendor/skills.sh/owner/repo@playwright");
    let record: Value =
        serde_json::from_str(&fs::read_to_string(vendor_dir.join("record.json")).unwrap()).unwrap();
    assert_eq!(record["locator"], "owner/repo@playwright");
    assert_eq!(record["version"], "2.0.0");
    assert!(vendor_dir.join("files/SKILL.md").exists());

    let index = fs::read_to_string(state.path().join("indexes/catalog.json")).unwrap();
    assert!(index.contains("\"skill:playwright\""));
}

#[test]
fn skills_sh_import_rejects_existing_capability_without_overwrite() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let adapter = SkillsShAdapter::with_command(MockSkillsCommand::import(skill_files()));

    import_source_from_adapter(ImportSourceAdapterRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        locator: "owner/repo@playwright".to_string(),
        replace_existing: false,
        adapter: &adapter,
    })
    .unwrap();
    let err = import_source_from_adapter(ImportSourceAdapterRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        locator: "owner/repo@playwright".to_string(),
        replace_existing: false,
        adapter: &adapter,
    })
    .unwrap_err();

    assert!(matches!(
        err,
        ImportSourceError::Storage(SourceImportStorageError::AlreadyExists { .. })
    ));
    assert_eq!(err.to_diagnostic().code, "source.import-conflict");
}

fn success_output() -> CommandOutput {
    CommandOutput {
        code: 0,
        stdout: "installed".to_string(),
        stderr: String::new(),
    }
}

fn skill_files() -> Vec<(PathBuf, String)> {
    vec![
        (
            PathBuf::from("SKILL.md"),
            r#"---
name: playwright
description: Mock Playwright skill.
metadata:
  version: "2.0.0"
---

# Playwright
"#
            .to_string(),
        ),
        (
            PathBuf::from("docs/usage.md"),
            "Use Playwright.\n".to_string(),
        ),
    ]
}
