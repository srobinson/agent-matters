mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::capabilities::{
    CapabilityDiffStatus, DiffCapabilityRequest, diff_capability,
};
use agent_matters_core::domain::Diagnostic;
use serde_json::json;
use tempfile::TempDir;

use support::fixture_path;

fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn imported_capability_without_overlay_reports_no_overlay() {
    let result = diff_fixture("catalogs/imported");

    assert!(result.files.is_empty());
    assert!(has_code(&result.diagnostics, "capability.diff-no-overlay"));
}

#[test]
fn changed_overlay_file_is_reported() {
    let repo = overlay_repo(
        &[("SKILL.md", "upstream\n")],
        &[("SKILL.md", "local\n")],
        true,
    );

    let result = diff_repo(repo.path());

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].path, "SKILL.md");
    assert_eq!(result.files[0].status, CapabilityDiffStatus::Changed);
}

#[test]
fn added_and_removed_overlay_files_are_reported() {
    let repo = overlay_repo(
        &[("SKILL.md", "same\n"), ("removed.md", "upstream only\n")],
        &[("SKILL.md", "same\n"), ("added.md", "overlay only\n")],
        true,
    );

    let result = diff_repo(repo.path());

    assert_eq!(result.diagnostics, Vec::new());
    assert!(
        result
            .files
            .iter()
            .any(|file| { file.path == "added.md" && file.status == CapabilityDiffStatus::Added })
    );
    assert!(
        result.files.iter().any(|file| {
            file.path == "removed.md" && file.status == CapabilityDiffStatus::Removed
        })
    );
}

#[test]
fn missing_vendor_source_reports_diagnostic() {
    let repo = overlay_repo(
        &[("SKILL.md", "upstream\n")],
        &[("SKILL.md", "local\n")],
        false,
    );

    let result = diff_repo(repo.path());

    assert!(result.files.is_empty());
    assert!(has_code(
        &result.diagnostics,
        "capability.diff-vendor-source-missing"
    ));
}

#[test]
fn json_shape_is_stable_for_changed_file() {
    let repo = overlay_repo(
        &[("SKILL.md", "upstream\n")],
        &[("SKILL.md", "local\n")],
        true,
    );

    let result = diff_repo(repo.path());
    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(
        encoded,
        json!({
            "capability": "skill:playwright",
            "base_path": "catalog/skills/playwright",
            "overlay_path": "overlays/skills/playwright",
            "vendor_path": "vendor/skills.sh/playwright",
            "files": [
                {
                    "path": "SKILL.md",
                    "status": "changed",
                    "base_bytes": 9,
                    "overlay_bytes": 6
                }
            ],
            "diagnostics": []
        })
    );
}

#[test]
fn large_changed_files_are_summarized() {
    let upstream = "a".repeat(65_537);
    let overlay = "b".repeat(65_537);
    let repo = overlay_repo(&[("SKILL.md", &upstream)], &[("SKILL.md", &overlay)], true);

    let result = diff_repo(repo.path());

    assert_eq!(
        result.files[0].note.as_deref(),
        Some("content diff omitted because file exceeds 65536 bytes")
    );
}

fn diff_fixture(relative: &str) -> agent_matters_capabilities::capabilities::DiffCapabilityResult {
    diff_repo(&fixture_path(relative))
}

fn diff_repo(repo_root: &Path) -> agent_matters_capabilities::capabilities::DiffCapabilityResult {
    let state = TempDir::new().unwrap();
    diff_capability(DiffCapabilityRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        capability: "skill:playwright".to_string(),
    })
    .unwrap()
}

fn overlay_repo(
    base_files: &[(&str, &str)],
    overlay_files: &[(&str, &str)],
    include_vendor: bool,
) -> TempDir {
    let repo = TempDir::new().unwrap();
    let base = repo.path().join("catalog/skills/playwright");
    let overlay = repo.path().join("overlays/skills/playwright");
    fs::create_dir_all(&base).unwrap();
    fs::create_dir_all(&overlay).unwrap();
    fs::write(base.join("manifest.toml"), capability_manifest()).unwrap();
    fs::write(overlay.join("manifest.toml"), capability_manifest()).unwrap();

    for (path, content) in base_files {
        write_file(&base.join(path), content);
    }
    for (path, content) in overlay_files {
        write_file(&overlay.join(path), content);
    }

    if include_vendor {
        let vendor = repo.path().join("vendor/skills.sh/playwright");
        fs::create_dir_all(&vendor).unwrap();
        fs::write(vendor.join("record.json"), r#"{"name":"playwright"}"#).unwrap();
    }

    repo
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn capability_manifest() -> &'static str {
    r#"id = "skill:playwright"
kind = "skill"
summary = "Playwright skill."

[origin]
type = "external"
source = "skills.sh"
locator = "playwright"
version = "1.0.0"

[files]
source = "SKILL.md"

[runtimes.codex]
supported = true
"#
}
