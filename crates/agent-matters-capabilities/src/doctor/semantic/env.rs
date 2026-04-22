use std::collections::BTreeSet;

use agent_matters_core::domain::Diagnostic;

use crate::catalog::CatalogDiscovery;

use super::diagnostics::missing_required_env;

pub(super) fn observed_env_names() -> BTreeSet<String> {
    std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .collect()
}

pub(super) fn validate_env_requirements(
    discovery: &CatalogDiscovery,
    env_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.capabilities {
        let Some(requires) = &entry.manifest.requires else {
            continue;
        };
        for required in &requires.env {
            if !env_names.contains(required.name()) {
                diagnostics.push(missing_required_env(entry, required.name()));
            }
        }
    }
}
