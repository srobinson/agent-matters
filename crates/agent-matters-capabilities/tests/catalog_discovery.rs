mod support;

use agent_matters_capabilities::catalog::{CapabilityDiscoverySource, discover_catalog};
use agent_matters_core::domain::{CapabilityKind, Diagnostic, DiagnosticSeverity};

use support::fixture_path;

fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn fixture_catalog_discovers_profiles_and_all_capability_kinds() {
    let root = fixture_path("catalogs/valid");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.diagnostics, Vec::new());
    assert_eq!(discovery.profiles.len(), 1);
    assert_eq!(discovery.capabilities.len(), CapabilityKind::all().len());

    let profile = &discovery.profiles[0];
    assert_eq!(profile.manifest.id.as_str(), "github-researcher");
    assert!(
        profile
            .directory_path
            .ends_with("profiles/renamed-profile-dir")
    );
    assert!(
        profile
            .manifest_path
            .ends_with("profiles/renamed-profile-dir/manifest.toml")
    );

    let skill = discovery
        .capabilities
        .iter()
        .find(|entry| entry.manifest.id.to_string() == "skill:playwright")
        .expect("skill capability discovered");
    assert!(
        skill
            .directory_path
            .ends_with("catalog/skills/renamed-skill-dir")
    );

    for kind in CapabilityKind::all() {
        assert!(
            discovery
                .capabilities
                .iter()
                .any(|entry| entry.manifest.kind == *kind),
            "missing capability kind {kind}"
        );
    }
}

#[test]
fn duplicate_capability_ids_are_reported_without_dropping_manifests() {
    let root = fixture_path("catalogs/duplicate-capability");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.capabilities.len(), 2);

    let duplicate = discovery
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.duplicate-id")
        .expect("duplicate id diagnostic");
    assert_eq!(duplicate.severity, DiagnosticSeverity::Error);
    assert!(duplicate.message.contains("skill:dupe"));
    let location = duplicate.location.as_ref().expect("duplicate location");
    assert_eq!(location.field.as_deref(), Some("id"));
    assert!(
        location
            .manifest_path
            .as_ref()
            .expect("duplicate manifest path")
            .ends_with("catalog/skills/second/manifest.toml")
    );
}

#[test]
fn capability_kind_mismatches_are_reported_without_dropping_manifests() {
    let root = fixture_path("catalogs/kind-mismatch");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.capabilities.len(), 2);

    let manifest_mismatch = discovery
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.manifest-kind-mismatch")
        .expect("manifest kind mismatch diagnostic");
    assert_eq!(manifest_mismatch.severity, DiagnosticSeverity::Error);
    assert_eq!(
        manifest_mismatch
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("id")
    );

    let directory_mismatch = discovery
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.directory-kind-mismatch")
        .expect("directory kind mismatch diagnostic");
    assert_eq!(directory_mismatch.severity, DiagnosticSeverity::Error);
    assert_eq!(
        directory_mismatch
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("kind")
    );
}

#[test]
fn malformed_toml_missing_manifest_and_unknown_folder_are_reported() {
    let root = fixture_path("catalogs/broken");
    let discovery = discover_catalog(&root);

    assert!(has_code(&discovery.diagnostics, "catalog.manifest-invalid"));
    assert!(has_code(&discovery.diagnostics, "catalog.manifest-missing"));
    assert!(has_code(&discovery.diagnostics, "catalog.unknown-folder"));
    assert_eq!(discovery.capabilities.len(), 0);
}

#[test]
fn generated_agent_matters_state_is_not_source_discovery() {
    let root = fixture_path("catalogs/generated-only");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.capabilities.len(), 0);
    assert_eq!(discovery.profiles.len(), 0);
    assert_eq!(discovery.diagnostics, Vec::new());
}

#[test]
fn imported_capability_without_overlay_preserves_vendor_pointer() {
    let root = fixture_path("catalogs/imported");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.diagnostics, Vec::new());
    let capability = discovery
        .capabilities
        .iter()
        .find(|entry| entry.manifest.id.to_string() == "skill:playwright")
        .expect("imported capability discovered");

    match &capability.source {
        CapabilityDiscoverySource::Imported { vendor_path } => assert!(
            vendor_path
                .as_ref()
                .expect("vendor pointer")
                .ends_with("vendor/skills.sh/playwright")
        ),
        other => panic!("expected imported source, got {other:?}"),
    }
}

#[test]
fn imported_capability_with_overlay_uses_overlay_as_effective_manifest() {
    let root = fixture_path("catalogs/imported-overlaid");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.diagnostics, Vec::new());
    assert_eq!(discovery.capabilities.len(), 1);
    let capability = &discovery.capabilities[0];

    assert_eq!(
        capability.manifest.summary,
        "Local Playwright skill overlay."
    );
    assert!(
        capability
            .manifest_path
            .ends_with("overlays/skills/playwright/manifest.toml")
    );
    match &capability.source {
        CapabilityDiscoverySource::Overlay {
            target_manifest_path,
            target_origin,
            vendor_path,
            ..
        } => {
            assert!(target_manifest_path.ends_with("catalog/skills/playwright/manifest.toml"));
            assert!(
                target_origin
                    .as_ref()
                    .is_some_and(|origin| { origin.requires_vendor_record() })
            );
            assert!(
                vendor_path
                    .as_ref()
                    .expect("vendor pointer")
                    .ends_with("vendor/skills.sh/playwright")
            );
        }
        other => panic!("expected overlay source, got {other:?}"),
    }
}

#[test]
fn overlay_target_missing_is_reported_without_adding_capability() {
    let root = fixture_path("catalogs/overlay-target-missing");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.capabilities.len(), 0);
    assert!(has_code(
        &discovery.diagnostics,
        "catalog.overlay-target-missing"
    ));
}

#[test]
fn invalid_overlay_manifest_is_reported() {
    let root = fixture_path("catalogs/overlay-invalid");
    let discovery = discover_catalog(&root);

    assert_eq!(discovery.capabilities.len(), 1);
    assert!(has_code(&discovery.diagnostics, "catalog.manifest-invalid"));
    assert_eq!(
        discovery.capabilities[0].manifest.summary,
        "Upstream Playwright skill."
    );
}
