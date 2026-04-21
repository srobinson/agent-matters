mod support;

use std::fs;

use agent_matters_core::domain::{CapabilityKind, ProfileKind};
use agent_matters_core::manifest::{CapabilityManifest, InstructionMarkers, ProfileManifest};

use support::fixture_path;

fn read_fixture(relative: &str) -> String {
    fs::read_to_string(fixture_path(relative)).unwrap()
}

#[test]
fn valid_profile_manifest_fixture_parses() {
    let src = read_fixture("manifests/profiles/github-researcher/manifest.toml");

    let manifest: ProfileManifest = toml::from_str(&src).unwrap();

    assert_eq!(manifest.id.as_str(), "github-researcher");
    assert_eq!(manifest.kind, ProfileKind::Persona);
    assert_eq!(manifest.capabilities.len(), 2);
    assert_eq!(manifest.instructions.len(), 2);

    let runtimes = manifest.runtimes.unwrap();
    assert_eq!(runtimes.default.unwrap().as_str(), "codex");
    assert!(
        runtimes
            .entries
            .get(&"codex".parse().unwrap())
            .unwrap()
            .enabled
    );

    let output = manifest.instructions_output.unwrap();
    assert_eq!(output.markers, Some(InstructionMarkers::HtmlComments));
}

#[test]
fn valid_capability_manifest_fixture_parses() {
    let src = read_fixture("manifests/capabilities/mcp-linear/manifest.toml");

    let manifest: CapabilityManifest = toml::from_str(&src).unwrap();

    assert_eq!(manifest.id.to_string(), "mcp:linear");
    assert_eq!(manifest.kind, CapabilityKind::Mcp);
    assert_eq!(
        manifest.files.entries.get("manifest").map(String::as_str),
        Some("server.toml")
    );
    assert!(
        manifest
            .runtimes
            .entries
            .get(&"claude".parse().unwrap())
            .unwrap()
            .supported
    );
    assert_eq!(
        manifest.requires.unwrap().env,
        vec!["LINEAR_API_KEY".to_string()]
    );
}

#[test]
fn profile_missing_required_field_reports_field_name() {
    let src = read_fixture("manifests/profiles/missing-id/manifest.toml");

    let err = toml::from_str::<ProfileManifest>(&src).unwrap_err();

    assert!(err.to_string().contains("id"));
}

#[test]
fn capability_missing_required_field_reports_field_name() {
    let src = read_fixture("manifests/capabilities/missing-files/manifest.toml");

    let err = toml::from_str::<CapabilityManifest>(&src).unwrap_err();

    assert!(err.to_string().contains("files"));
}

#[test]
fn profile_runtime_table_requires_enabled_boolean() {
    let src = read_fixture("manifests/profiles/runtime-missing-enabled/manifest.toml");

    let err = toml::from_str::<ProfileManifest>(&src).unwrap_err();

    assert!(err.to_string().contains("enabled"));
}

#[test]
fn capability_runtime_table_requires_supported_boolean() {
    let src = read_fixture("manifests/capabilities/runtime-missing-supported/manifest.toml");

    let err = toml::from_str::<CapabilityManifest>(&src).unwrap_err();

    assert!(err.to_string().contains("supported"));
}

#[test]
fn profile_runtimes_default_subtable_is_rejected() {
    let src = read_fixture("manifests/profiles/runtimes-default-table/manifest.toml");

    let err = toml::from_str::<ProfileManifest>(&src).unwrap_err();

    assert!(err.to_string().contains("default"));
}
