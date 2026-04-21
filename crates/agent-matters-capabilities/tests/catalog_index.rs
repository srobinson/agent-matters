mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::capabilities::{ListCapabilitiesRequest, list_capabilities};
use agent_matters_capabilities::catalog::{
    CatalogIndexStatus, LoadCatalogIndexRequest, catalog_index_path, load_or_refresh_catalog_index,
};
use agent_matters_capabilities::profiles::{ListProfilesRequest, list_profiles};
use tempfile::TempDir;

use support::fixture_path;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

#[test]
fn rebuild_from_fixture_catalog_writes_stable_index() {
    let repo_root = fixture_path("catalogs/valid");
    let state = TempDir::new().unwrap();

    let first = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: repo_root.clone(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(first.status, CatalogIndexStatus::RebuiltMissing);
    assert_eq!(first.diagnostics, Vec::new());
    assert_eq!(first.index.capabilities.len(), 6);
    assert_eq!(first.index.profiles.len(), 1);
    assert!(first.index_path.ends_with("indexes/catalog.json"));

    let first_json = fs::read_to_string(&first.index_path).unwrap();
    let second = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root,
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();
    let second_json = fs::read_to_string(&second.index_path).unwrap();

    assert_eq!(second.status, CatalogIndexStatus::Fresh);
    assert_eq!(first_json, second_json);
}

#[test]
fn exact_lookup_by_capability_id_and_profile_id() {
    let repo_root = fixture_path("catalogs/valid");
    let state = TempDir::new().unwrap();

    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root,
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    let capability = loaded.index.capability("skill:playwright").unwrap();
    assert_eq!(capability.kind, "skill");
    assert!(capability.runtimes["codex"].supported);
    assert_eq!(
        capability.requirements,
        agent_matters_core::catalog::RequirementSummary::default()
    );

    let profile = loaded.index.profile("github-researcher").unwrap();
    assert_eq!(profile.kind, "persona");
    assert_eq!(profile.capability_count, 4);
    assert_eq!(profile.instruction_count, 2);
}

#[test]
fn equivalent_catalogs_in_different_roots_have_same_fingerprint() {
    let first_repo = TempDir::new().unwrap();
    let second_repo = TempDir::new().unwrap();
    let first_state = TempDir::new().unwrap();
    let second_state = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), first_repo.path());
    copy_dir(&fixture_path("catalogs/valid"), second_repo.path());

    let first = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: first_repo.path().to_path_buf(),
        user_state_dir: first_state.path().to_path_buf(),
    })
    .unwrap();
    let second = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: second_repo.path().to_path_buf(),
        user_state_dir: second_state.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(
        first.index.content_fingerprint,
        second.index.content_fingerprint
    );
}

#[test]
fn stale_index_after_manifest_modification_rebuilds_deterministically() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let state = TempDir::new().unwrap();

    let first = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();
    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("Focused research agent", "Changed research agent");
    fs::write(&manifest_path, updated).unwrap();

    let second = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    assert_ne!(
        first.index.content_fingerprint,
        second.index.content_fingerprint
    );
    assert_eq!(second.status, CatalogIndexStatus::RebuiltStale);
    assert_eq!(
        second.index.profile("github-researcher").unwrap().summary,
        "Changed research agent for inspecting GitHub repositories."
    );
}

#[test]
fn corrupt_index_is_reported_and_recovered_by_rebuild() {
    let repo_root = fixture_path("catalogs/valid");
    let state = TempDir::new().unwrap();
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(index_path.parent().unwrap()).unwrap();
    fs::write(&index_path, "{not valid json").unwrap();

    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root,
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(loaded.status, CatalogIndexStatus::RecoveredCorrupt);
    assert!(
        loaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog.index-corrupt")
    );
    assert!(loaded.index.capability("mcp:linear").is_some());
}

#[test]
fn duplicate_ids_are_not_added_to_exact_lookup_index() {
    let repo_root = fixture_path("catalogs/duplicate-capability");
    let state = TempDir::new().unwrap();

    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root,
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    assert!(loaded.index.capability("skill:dupe").is_none());
    assert!(
        loaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog.duplicate-id")
    );
}

#[test]
fn list_use_cases_read_from_generated_index() {
    let repo_root = fixture_path("catalogs/valid");
    let state = TempDir::new().unwrap();

    let profiles = list_profiles(ListProfilesRequest {
        repo_root: repo_root.clone(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();
    let capabilities = list_capabilities(ListCapabilitiesRequest {
        repo_root,
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(profiles.profiles[0].id, "github-researcher");
    assert!(
        capabilities
            .capabilities
            .iter()
            .any(|record| record.id == "skill:playwright")
    );
}
