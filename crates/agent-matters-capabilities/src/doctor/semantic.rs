use std::collections::BTreeMap;
use std::path::Path;

use agent_matters_core::domain::Diagnostic;

use crate::catalog::{CatalogDiscovery, DiscoveredCapabilityManifest};

mod capabilities;
mod diagnostics;
mod env;
mod imports;
mod profiles;

type CapabilityEntries<'a> = BTreeMap<String, &'a DiscoveredCapabilityManifest>;

pub(super) fn validate_catalog_semantics(
    repo_root: &Path,
    discovery: &CatalogDiscovery,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let capability_entries = capabilities::unique_capability_entries(discovery);
    let env_names = env::observed_env_names();

    capabilities::validate_capability_files(discovery, diagnostics);
    capabilities::validate_capability_requirements(discovery, &capability_entries, diagnostics);
    profiles::validate_profile_references(discovery, &capability_entries, diagnostics);
    profiles::validate_profile_requirement_satisfaction(
        discovery,
        &capability_entries,
        diagnostics,
    );
    env::validate_env_requirements(discovery, &env_names, diagnostics);
    imports::validate_import_sources(repo_root, discovery, diagnostics);
}
