use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use agent_matters_capabilities::catalog::{CapabilityDiscoverySource, discover_catalog};
use agent_matters_capabilities::sources::{
    ImportSourceAdapterRequest, ImportSourceRequest, SourceAdapter, SourceAdapterError,
    SourceImportFile, SourceImportRequest, SourceImportResult, SourceImportStorageError,
    SourceSearchEntry, SourceSearchRequest, SourceSearchResult, WriteSourceImportRequest,
    import_source, import_source_from_adapter_with_policy, write_source_import,
};
use agent_matters_core::config::{SourceTrustPolicy, SourceTrustRule};
use agent_matters_core::domain::{
    CapabilityId, CapabilityKind, Diagnostic, DiagnosticSeverity, Provenance, RuntimeId,
};
use agent_matters_core::manifest::{
    CapabilityFilesManifest, CapabilityManifest, CapabilityRuntimeManifest,
    CapabilityRuntimesManifest,
};
use serde_json::json;
use tempfile::TempDir;

struct FakeSourceAdapter;

impl SourceAdapter for FakeSourceAdapter {
    fn source_id(&self) -> &str {
        "skills.sh"
    }

    fn search(
        &self,
        request: SourceSearchRequest,
    ) -> Result<SourceSearchResult, SourceAdapterError> {
        Ok(SourceSearchResult {
            source: self.source_id().to_string(),
            query: request.query,
            entries: vec![SourceSearchEntry {
                locator: "playwright".to_string(),
                summary: Some("Automate browsers from coding agents.".to_string()),
                version: Some("1.0.0".to_string()),
                raw: json!({
                    "name": "playwright",
                    "description": "Automate browsers from coding agents.",
                    "dist-tags": { "latest": "1.0.0" }
                }),
            }],
            diagnostics: Vec::new(),
        })
    }

    fn import_capability(
        &self,
        request: SourceImportRequest,
    ) -> Result<SourceImportResult, SourceAdapterError> {
        Ok(SourceImportResult {
            source: self.source_id().to_string(),
            locator: request.locator.clone(),
            manifest: fake_manifest(&request.locator),
            catalog_files: vec![SourceImportFile {
                relative_path: PathBuf::from("SKILL.md"),
                contents: "# Playwright\n".to_string(),
            }],
            vendor_files: vec![SourceImportFile {
                relative_path: PathBuf::from("record.json"),
                contents: json!({
                    "name": request.locator,
                    "version": "1.0.0"
                })
                .to_string(),
            }],
            diagnostics: Vec::new(),
        })
    }
}

struct BlockedSourceAdapter {
    import_called: Cell<bool>,
}

impl SourceAdapter for BlockedSourceAdapter {
    fn source_id(&self) -> &str {
        "blocked-source"
    }

    fn search(
        &self,
        request: SourceSearchRequest,
    ) -> Result<SourceSearchResult, SourceAdapterError> {
        Ok(SourceSearchResult {
            source: self.source_id().to_string(),
            query: request.query,
            entries: Vec::new(),
            diagnostics: Vec::new(),
        })
    }

    fn import_capability(
        &self,
        request: SourceImportRequest,
    ) -> Result<SourceImportResult, SourceAdapterError> {
        self.import_called.set(true);
        Ok(SourceImportResult {
            source: self.source_id().to_string(),
            locator: request.locator,
            manifest: fake_manifest("playwright"),
            catalog_files: Vec::new(),
            vendor_files: Vec::new(),
            diagnostics: Vec::new(),
        })
    }
}

#[test]
fn fake_source_adapter_search_result_preserves_raw_record() {
    let adapter = FakeSourceAdapter;

    let result = adapter
        .search(SourceSearchRequest {
            query: "playwright".to_string(),
        })
        .unwrap();

    assert_eq!(result.source, "skills.sh");
    assert_eq!(result.query, "playwright");
    assert_eq!(result.entries[0].locator, "playwright");
    assert_eq!(result.entries[0].raw["name"], "playwright");
    assert_eq!(result.entries[0].raw["dist-tags"]["latest"], json!("1.0.0"));
}

#[test]
fn fake_source_adapter_import_result_normalizes_manifest_and_vendor_files() {
    let adapter = FakeSourceAdapter;

    let result = adapter
        .import_capability(SourceImportRequest {
            locator: "playwright".to_string(),
        })
        .unwrap();

    assert_eq!(result.source, "skills.sh");
    assert_eq!(result.locator, "playwright");
    assert_eq!(result.manifest.id.to_string(), "skill:playwright");
    assert_eq!(result.manifest.kind, CapabilityKind::Skill);
    assert_eq!(
        result.catalog_files[0].relative_path,
        PathBuf::from("SKILL.md")
    );
    assert_eq!(
        result.vendor_files[0].relative_path,
        PathBuf::from("record.json")
    );
    assert_eq!(
        result.manifest.origin,
        Some(Provenance::external(
            "skills.sh",
            "playwright",
            Some("1.0.0".to_string())
        ))
    );
}

#[test]
fn raw_plus_normalized_storage_contract_writes_catalog_and_vendor() {
    let repo = TempDir::new().unwrap();
    let adapter = FakeSourceAdapter;
    let import = adapter
        .import_capability(SourceImportRequest {
            locator: "playwright".to_string(),
        })
        .unwrap();

    let written = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import,
        replace_existing: false,
    })
    .unwrap();

    assert!(
        written
            .manifest_path
            .ends_with("catalog/skills/playwright/manifest.toml")
    );
    assert!(written.vendor_dir.ends_with("vendor/skills.sh/playwright"));
    assert_eq!(written.diagnostics, Vec::<Diagnostic>::new());

    let manifest_path = repo.path().join("catalog/skills/playwright/manifest.toml");
    let manifest_raw = fs::read_to_string(&manifest_path).unwrap();
    let manifest: CapabilityManifest = toml::from_str(&manifest_raw).unwrap();
    assert_eq!(manifest.id.to_string(), "skill:playwright");
    assert_eq!(
        manifest.origin,
        Some(Provenance::external(
            "skills.sh",
            "playwright",
            Some("1.0.0".to_string())
        ))
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("catalog/skills/playwright/SKILL.md")).unwrap(),
        "# Playwright\n"
    );

    let vendor_raw =
        fs::read_to_string(repo.path().join("vendor/skills.sh/playwright/record.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&vendor_raw).unwrap(),
        json!({
            "name": "playwright",
            "version": "1.0.0"
        })
    );

    let discovery = discover_catalog(repo.path());
    assert_eq!(discovery.diagnostics, Vec::new());
    match &discovery.capabilities[0].source {
        CapabilityDiscoverySource::Imported { vendor_path } => {
            assert!(
                vendor_path
                    .as_ref()
                    .expect("vendor path")
                    .ends_with("vendor/skills.sh/playwright")
            );
        }
        other => panic!("expected imported capability, got {other:?}"),
    }
}

#[test]
fn import_storage_stages_writes_so_mid_write_failure_is_retryable() {
    let repo = TempDir::new().unwrap();
    let adapter = FakeSourceAdapter;
    let mut import = adapter
        .import_capability(SourceImportRequest {
            locator: "playwright".to_string(),
        })
        .unwrap();
    import.catalog_files = vec![
        SourceImportFile {
            relative_path: PathBuf::from("blocked"),
            contents: "file first\n".to_string(),
        },
        SourceImportFile {
            relative_path: PathBuf::from("blocked/child.md"),
            contents: "then fail\n".to_string(),
        },
    ];

    let err = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import,
        replace_existing: false,
    })
    .unwrap_err();

    assert!(matches!(
        err,
        SourceImportStorageError::CreateDirectory { .. }
    ));
    assert!(!repo.path().join("catalog/skills/playwright").exists());
    assert!(!repo.path().join("vendor/skills.sh/playwright").exists());
    assert_empty_or_missing(&repo.path().join("catalog/skills"));
    assert_empty_or_missing(&repo.path().join("vendor/skills.sh"));

    let retry_import = adapter
        .import_capability(SourceImportRequest {
            locator: "playwright".to_string(),
        })
        .unwrap();
    write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import: retry_import,
        replace_existing: false,
    })
    .unwrap();

    assert!(
        repo.path()
            .join("catalog/skills/playwright/manifest.toml")
            .exists()
    );
    assert!(
        repo.path()
            .join("vendor/skills.sh/playwright/record.json")
            .exists()
    );
}

#[test]
fn source_adapter_error_maps_to_source_diagnostic() {
    let diagnostic =
        SourceAdapterError::import_failed("skills.sh", "missing", "upstream command exited 1")
            .to_diagnostic();

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, "source.import-failed");
    assert!(diagnostic.message.contains("skills.sh"));
    assert!(diagnostic.message.contains("missing"));
    assert!(!diagnostic.code.starts_with("catalog."));
}

#[test]
fn source_trust_policy_blocks_unknown_source_import() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();

    let err = import_source(ImportSourceRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        locator: "unknown:playwright".to_string(),
        replace_existing: false,
    })
    .unwrap_err();

    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, "source.trust-blocked");
    assert!(diagnostic.message.contains("unknown"));
    assert!(diagnostic.message.contains("capability kind `unknown`"));
    assert!(!repo.path().join("catalog").exists());
}

#[test]
fn source_trust_policy_blocks_adapter_source_before_invocation() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let adapter = BlockedSourceAdapter {
        import_called: Cell::new(false),
    };

    let err = import_source_from_adapter_with_policy(
        ImportSourceAdapterRequest {
            repo_root: repo.path().to_path_buf(),
            user_state_dir: state.path().to_path_buf(),
            locator: "playwright".to_string(),
            replace_existing: false,
            adapter: &adapter,
        },
        &SourceTrustPolicy::conservative_default(),
    )
    .unwrap_err();

    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.code, "source.trust-blocked");
    assert!(diagnostic.message.contains("blocked-source"));
    assert!(diagnostic.message.contains("capability kind `unknown`"));
    assert!(!adapter.import_called.get());
    assert!(!repo.path().join("catalog").exists());
    assert!(!repo.path().join("vendor").exists());
}

#[test]
fn source_trust_policy_blocks_disallowed_capability_kind_before_writing() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let adapter = FakeSourceAdapter;
    let policy = SourceTrustPolicy {
        sources: BTreeMap::from([(
            "skills.sh".to_string(),
            SourceTrustRule {
                kinds: vec![CapabilityKind::Mcp],
            },
        )]),
    };

    let err = import_source_from_adapter_with_policy(
        ImportSourceAdapterRequest {
            repo_root: repo.path().to_path_buf(),
            user_state_dir: state.path().to_path_buf(),
            locator: "playwright".to_string(),
            replace_existing: false,
            adapter: &adapter,
        },
        &policy,
    )
    .unwrap_err();

    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.code, "source.trust-blocked");
    assert!(diagnostic.message.contains("skills.sh"));
    assert!(diagnostic.message.contains("skill"));
    assert!(!repo.path().join("catalog").exists());
    assert!(!repo.path().join("vendor").exists());
}

#[test]
fn import_storage_rejects_escape_paths_before_writing_manifest() {
    let repo = TempDir::new().unwrap();
    let adapter = FakeSourceAdapter;
    let mut import = adapter
        .import_capability(SourceImportRequest {
            locator: "playwright".to_string(),
        })
        .unwrap();
    import.catalog_files[0].relative_path = PathBuf::from("../SKILL.md");

    let err = write_source_import(WriteSourceImportRequest {
        repo_root: repo.path().to_path_buf(),
        import,
        replace_existing: false,
    })
    .unwrap_err();

    assert!(matches!(
        err,
        SourceImportStorageError::InvalidRelativePath { .. }
    ));
    assert!(!repo.path().join("catalog").exists());
    assert!(
        !repo
            .path()
            .join("catalog/skills/playwright/manifest.toml")
            .exists()
    );
}

#[test]
fn import_storage_rejects_escape_source_or_locator_before_writing() {
    for (source, locator) in [
        ("../skills.sh", "playwright"),
        ("skills.sh", "../playwright"),
    ] {
        let repo = TempDir::new().unwrap();
        let adapter = FakeSourceAdapter;
        let mut import = adapter
            .import_capability(SourceImportRequest {
                locator: "playwright".to_string(),
            })
            .unwrap();
        import.source = source.to_string();
        import.locator = locator.to_string();
        import.manifest.origin = Some(Provenance::external(
            source,
            locator,
            Some("1.0.0".to_string()),
        ));

        let err = write_source_import(WriteSourceImportRequest {
            repo_root: repo.path().to_path_buf(),
            import,
            replace_existing: false,
        })
        .unwrap_err();

        assert!(matches!(
            err,
            SourceImportStorageError::InvalidRelativePath { .. }
        ));
        assert!(!repo.path().join("catalog").exists());
        assert!(!repo.path().join("vendor").exists());
    }
}

fn fake_manifest(locator: &str) -> CapabilityManifest {
    let mut files = BTreeMap::new();
    files.insert("source".to_string(), "SKILL.md".to_string());

    let mut runtimes = BTreeMap::new();
    runtimes.insert(
        RuntimeId::new("codex").unwrap(),
        CapabilityRuntimeManifest { supported: true },
    );

    CapabilityManifest {
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
    }
}

fn assert_empty_or_missing(path: &std::path::Path) {
    if path.exists() {
        assert!(fs::read_dir(path).unwrap().next().is_none());
    }
}
