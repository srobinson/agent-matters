mod support;

use agent_matters_capabilities::capabilities::{
    ListCapabilitiesRequest, ShowCapabilityRequest, ShowCapabilityResult, list_capabilities,
    show_capability,
};
use agent_matters_core::catalog::CapabilityIndexRecord;
use agent_matters_core::domain::Diagnostic;
use serde_json::json;
use tempfile::TempDir;

use support::fixtures::fixture_path;

fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn list_fixture_capabilities_include_picker_metadata() {
    let state = TempDir::new().unwrap();
    let result = list_capabilities(ListCapabilitiesRequest {
        repo_root: fixture_path("catalogs/valid"),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    let record = result
        .capabilities
        .iter()
        .find(|record| record.id == "skill:playwright")
        .unwrap();

    assert_eq!(record.kind, "skill");
    assert_eq!(record.summary, "Playwright browser automation skill.");
    assert_eq!(
        record.files.get("source").map(String::as_str),
        Some("SKILL.md")
    );
    assert_eq!(record.source.kind, "local");
    assert!(record.runtimes["codex"].supported);
}

#[test]
fn show_local_capability_by_exact_id() {
    let record = show_record("catalogs/valid", "skill:playwright");

    assert_eq!(record.id, "skill:playwright");
    assert_eq!(record.source_path, "catalog/skills/renamed-skill-dir");
    assert_eq!(record.source.kind, "local");
    assert_eq!(
        record.source.normalized_path.as_deref(),
        Some("catalog/skills/renamed-skill-dir")
    );
    assert_eq!(record.provenance.kind, "local");
    assert_eq!(
        record.files.get("source").map(String::as_str),
        Some("SKILL.md")
    );
}

#[test]
fn show_imported_capability_reports_provenance_and_vendor_path() {
    let record = show_record("catalogs/imported", "skill:playwright");

    assert_eq!(record.source.kind, "imported");
    assert_eq!(
        record.source.vendor_path.as_deref(),
        Some("vendor/skills.sh/playwright")
    );
    assert_eq!(record.source.overlay_path, None);
    assert_eq!(record.provenance.kind, "external");
    assert_eq!(record.provenance.source.as_deref(), Some("skills.sh"));
    assert_eq!(record.provenance.locator.as_deref(), Some("playwright"));
    assert_eq!(record.provenance.version.as_deref(), Some("1.0.0"));
}

#[test]
fn show_overlaid_capability_reports_effective_overlay_state() {
    let record = show_record("catalogs/imported-overlaid", "skill:playwright");

    assert_eq!(record.summary, "Local Playwright skill overlay.");
    assert_eq!(record.source_path, "overlays/skills/playwright");
    assert_eq!(record.source.kind, "overlaid");
    assert_eq!(
        record.source.normalized_path.as_deref(),
        Some("catalog/skills/playwright")
    );
    assert_eq!(
        record.source.overlay_path.as_deref(),
        Some("overlays/skills/playwright")
    );
    assert_eq!(
        record.source.vendor_path.as_deref(),
        Some("vendor/skills.sh/playwright")
    );
    assert!(record.runtimes["claude"].supported);
}

#[test]
fn missing_capability_returns_actionable_error() {
    let result = show_fixture("catalogs/valid", "skill:missing");

    assert!(result.record.is_none());
    assert!(result.has_error_diagnostics());
    assert!(has_code(&result.diagnostics, "capability.show-not-found"));
    assert_eq!(
        result.diagnostics[0]
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("capability")
    );
    assert_eq!(
        result.diagnostics[0].recovery_hint.as_deref(),
        Some("run `agent-matters capabilities list` to inspect exact capability ids")
    );
}

#[test]
fn show_json_shape_is_stable_for_overlaid_capability() {
    let result = show_fixture("catalogs/imported-overlaid", "skill:playwright");
    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(
        encoded,
        json!({
            "capability": "skill:playwright",
            "record": {
                "id": "skill:playwright",
                "kind": "skill",
                "summary": "Local Playwright skill overlay.",
                "files": {
                    "source": "SKILL.md"
                },
                "source_path": "overlays/skills/playwright",
                "source": {
                    "kind": "overlaid",
                    "normalized_path": "catalog/skills/playwright",
                    "overlay_path": "overlays/skills/playwright",
                    "vendor_path": "vendor/skills.sh/playwright"
                },
                "runtimes": {
                    "claude": {
                        "supported": true
                    },
                    "codex": {
                        "supported": true
                    }
                },
                "provenance": {
                    "kind": "external",
                    "source": "skills.sh",
                    "locator": "playwright",
                    "version": "1.0.0"
                },
                "requirements": {}
            },
            "diagnostics": []
        })
    );
}

fn show_record(relative: &str, capability: &str) -> CapabilityIndexRecord {
    show_fixture(relative, capability).record.unwrap()
}

fn show_fixture(relative: &str, capability: &str) -> ShowCapabilityResult {
    let state = TempDir::new().unwrap();
    show_capability(ShowCapabilityRequest {
        repo_root: fixture_path(relative),
        user_state_dir: state.path().to_path_buf(),
        capability: capability.to_string(),
    })
    .unwrap()
}
