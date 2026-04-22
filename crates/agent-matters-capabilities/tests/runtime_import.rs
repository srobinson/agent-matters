use std::fs;

use agent_matters_capabilities::sources::{
    ImportRuntimeHomeRequest, RuntimeHomeImportStatus, import_runtime_home,
};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

#[test]
fn runtime_import_infers_claude_runtime_and_profile_from_home_path() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let source = repo.path().join(".claude");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("CLAUDE.md"), "# Claude\n").unwrap();
    fs::write(
        source.join("settings.json"),
        r#"{"model":"claude-sonnet-4"}"#,
    )
    .unwrap();

    let result = import_runtime_home(ImportRuntimeHomeRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        runtime: None,
        source_home: source,
        profile: None,
        write: false,
    })
    .unwrap();

    assert_eq!(result.status, RuntimeHomeImportStatus::DryRun);
    assert_eq!(result.runtime, "claude");
    assert_eq!(result.profile_id, "imported-claude");
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.severity != DiagnosticSeverity::Error })
    );
    assert!(result.capabilities.iter().any(|capability| {
        capability.kind == "instruction" && capability.source_path == "CLAUDE.md"
    }));
}

#[test]
fn runtime_import_uses_runtime_override_for_generic_path() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let source = repo.path().join("custom runtime");
    fs::create_dir_all(source.join("skills/review")).unwrap();
    fs::write(source.join("skills/review/SKILL.md"), "# Review\n").unwrap();

    let result = import_runtime_home(ImportRuntimeHomeRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        runtime: Some("codex".to_string()),
        source_home: source,
        profile: None,
        write: false,
    })
    .unwrap();

    assert_eq!(result.runtime, "codex");
    assert_eq!(result.profile_id, "imported-custom-runtime");
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.severity != DiagnosticSeverity::Error })
    );
    assert!(result.capabilities.iter().any(|capability| {
        capability.kind == "skill" && capability.source_path == "skills/review/SKILL.md"
    }));
}

#[test]
fn runtime_import_reports_undetected_runtime_without_override() {
    let repo = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let source = repo.path().join("runtime-home");
    fs::create_dir_all(source.join("skills/review")).unwrap();
    fs::write(source.join("skills/review/SKILL.md"), "# Review\n").unwrap();

    let result = import_runtime_home(ImportRuntimeHomeRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        runtime: None,
        source_home: source,
        profile: None,
        write: false,
    })
    .unwrap();

    assert_eq!(result.status, RuntimeHomeImportStatus::DryRun);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.code == "source.runtime-import-runtime-undetected"
    }));
}
