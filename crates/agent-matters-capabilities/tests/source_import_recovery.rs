use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::sources::{
    ImportSourceError, SourceImportFile, SourceImportResult, WriteSourceImportRequest,
    WriteSourceImportStatus, write_source_import,
};
use agent_matters_core::catalog::MANIFEST_FILE_NAME;
use agent_matters_core::domain::{CapabilityId, CapabilityKind, Provenance, RuntimeId};
use agent_matters_core::manifest::{
    CapabilityFilesManifest, CapabilityManifest, CapabilityRuntimeManifest,
    CapabilityRuntimesManifest,
};
use tempfile::TempDir;

#[test]
fn write_source_import_completes_half_published_new_import_on_retry() {
    let repo = TempDir::new().unwrap();
    let import = source_import("playwright");
    write_half_published_capability(repo.path(), &import);

    let written = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import,
        replace_existing: false,
    })
    .unwrap();

    assert_eq!(written.status, WriteSourceImportStatus::Created);
    assert!(
        written
            .manifest_path
            .ends_with("catalog/skills/playwright/manifest.toml")
    );
    assert!(written.vendor_dir.ends_with("vendor/skills.sh/playwright"));
    assert_eq!(
        fs::read_to_string(repo.path().join("catalog/skills/playwright/SKILL.md")).unwrap(),
        "# Playwright\n"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("vendor/skills.sh/playwright/record.json")).unwrap(),
        "{\"name\":\"playwright\"}\n"
    );
}

#[test]
fn write_source_import_is_idempotent_when_complete_import_matches() {
    let repo = TempDir::new().unwrap();

    write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import("playwright"),
        replace_existing: false,
    })
    .unwrap();
    let written = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import("playwright"),
        replace_existing: false,
    })
    .unwrap();

    assert_eq!(written.status, WriteSourceImportStatus::Unchanged);
}

#[test]
fn write_source_import_requires_update_when_complete_import_differs() {
    let repo = TempDir::new().unwrap();

    write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import("playwright"),
        replace_existing: false,
    })
    .unwrap();
    let err = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import_with_contents(
            "playwright",
            "# Updated Playwright\n",
            "{\"name\":\"playwright\",\"version\":2}\n",
        ),
        replace_existing: false,
    })
    .unwrap_err();
    let diagnostic = ImportSourceError::Storage(err).to_diagnostic();

    assert_eq!(diagnostic.code, "source.import-conflict");
    assert!(
        diagnostic
            .recovery_hint
            .as_deref()
            .unwrap()
            .contains("--update")
    );
}

#[test]
fn write_source_import_updates_existing_import_when_requested() {
    let repo = TempDir::new().unwrap();

    write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import("playwright"),
        replace_existing: false,
    })
    .unwrap();
    let written = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import_with_contents(
            "playwright",
            "# Updated Playwright\n",
            "{\"name\":\"playwright\",\"version\":2}\n",
        ),
        replace_existing: true,
    })
    .unwrap();

    assert_eq!(written.status, WriteSourceImportStatus::Updated);
    assert_eq!(
        fs::read_to_string(repo.path().join("catalog/skills/playwright/SKILL.md")).unwrap(),
        "# Updated Playwright\n"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("vendor/skills.sh/playwright/record.json")).unwrap(),
        "{\"name\":\"playwright\",\"version\":2}\n"
    );
}

#[test]
fn write_source_import_recovers_interrupted_update_before_replacing() {
    let repo = TempDir::new().unwrap();

    write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import("playwright"),
        replace_existing: false,
    })
    .unwrap();

    let capability_dir = repo.path().join("catalog/skills/playwright");
    let vendor_dir = repo.path().join("vendor/skills.sh/playwright");
    let capability_backup = durable_sibling(&capability_dir, "source-import-backup");
    let vendor_backup = durable_sibling(&vendor_dir, "source-import-backup");
    fs::rename(&capability_dir, &capability_backup).unwrap();
    fs::rename(&vendor_dir, &vendor_backup).unwrap();

    let written = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: source_import_with_contents(
            "playwright",
            "# Updated Playwright\n",
            "{\"name\":\"playwright\",\"version\":2}\n",
        ),
        replace_existing: true,
    })
    .unwrap();

    assert_eq!(written.status, WriteSourceImportStatus::Updated);
    assert_eq!(
        fs::read_to_string(capability_dir.join("SKILL.md")).unwrap(),
        "# Updated Playwright\n"
    );
    assert_eq!(
        fs::read_to_string(vendor_dir.join("record.json")).unwrap(),
        "{\"name\":\"playwright\",\"version\":2}\n"
    );
    assert!(!capability_backup.exists());
    assert!(!vendor_backup.exists());
}

#[test]
fn write_source_import_reports_repair_needed_when_partial_catalog_differs() {
    let repo = TempDir::new().unwrap();
    let import = source_import("playwright");
    write_half_published_capability(repo.path(), &import);
    fs::write(
        repo.path().join("catalog/skills/playwright/SKILL.md"),
        "# Local Playwright\n",
    )
    .unwrap();

    let err = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import,
        replace_existing: false,
    })
    .unwrap_err();
    let diagnostic = ImportSourceError::Storage(err).to_diagnostic();

    assert_eq!(diagnostic.code, "source.import-repair-needed");
    assert!(diagnostic.message.contains("partially published"));
    assert!(
        diagnostic
            .recovery_hint
            .as_deref()
            .unwrap()
            .contains("--update")
    );
    assert!(
        !repo
            .path()
            .join("vendor/skills.sh/playwright/record.json")
            .exists()
    );
}

fn source_import(locator: &str) -> SourceImportResult {
    source_import_with_contents(locator, "# Playwright\n", "{\"name\":\"playwright\"}\n")
}

fn source_import_with_contents(
    locator: &str,
    skill_contents: &str,
    vendor_contents: &str,
) -> SourceImportResult {
    let mut files = BTreeMap::new();
    files.insert("source".to_string(), "SKILL.md".to_string());

    let mut runtimes = BTreeMap::new();
    runtimes.insert(
        RuntimeId::new("codex").unwrap(),
        CapabilityRuntimeManifest { supported: true },
    );

    SourceImportResult {
        source: "skills.sh".to_string(),
        locator: locator.to_string(),
        manifest: CapabilityManifest {
            id: CapabilityId::new(CapabilityKind::Skill, locator).unwrap(),
            kind: CapabilityKind::Skill,
            summary: "Playwright browser automation skill.".to_string(),
            files: CapabilityFilesManifest { entries: files },
            runtimes: CapabilityRuntimesManifest { entries: runtimes },
            requires: None,
            origin: Some(Provenance::external(
                "skills.sh",
                locator,
                Some("1.0.0".to_string()),
            )),
        },
        catalog_files: vec![SourceImportFile {
            relative_path: PathBuf::from("SKILL.md"),
            contents: skill_contents.to_string(),
        }],
        vendor_files: vec![SourceImportFile {
            relative_path: PathBuf::from("record.json"),
            contents: vendor_contents.to_string(),
        }],
        diagnostics: Vec::new(),
    }
}

fn write_half_published_capability(repo: &Path, import: &SourceImportResult) {
    let capability_dir = repo
        .join("catalog")
        .join("skills")
        .join(import.manifest.id.body());
    fs::create_dir_all(&capability_dir).unwrap();
    fs::write(
        capability_dir.join(MANIFEST_FILE_NAME),
        toml::to_string_pretty(&import.manifest).unwrap(),
    )
    .unwrap();
    for file in &import.catalog_files {
        let path = capability_dir.join(&file.relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &file.contents).unwrap();
    }
}

fn durable_sibling(path: &Path, label: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source-import");
    path.with_file_name(format!(".{name}.{label}"))
}
