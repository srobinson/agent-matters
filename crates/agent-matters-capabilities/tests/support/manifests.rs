use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use agent_matters_core::domain::{
    CapabilityId, EnvVarRequirement, Requirements, RuntimeId, ScopeConstraints,
};
use agent_matters_core::manifest::{
    CapabilityManifest, CapabilityRuntimeManifest, ProfileManifest, ProfileRuntimeManifest,
    ProfileRuntimesManifest,
};

pub(crate) struct ProfileRuntimeFixture<'a> {
    id: &'a str,
    enabled: bool,
    model: Option<&'a str>,
}

impl<'a> ProfileRuntimeFixture<'a> {
    pub(crate) fn enabled(id: &'a str) -> Self {
        Self {
            id,
            enabled: true,
            model: None,
        }
    }

    pub(crate) fn disabled(id: &'a str) -> Self {
        Self {
            id,
            enabled: false,
            model: None,
        }
    }

    pub(crate) fn disabled_with_model(id: &'a str, model: &'a str) -> Self {
        Self {
            id,
            enabled: false,
            model: Some(model),
        }
    }

    pub(crate) fn enabled_with_model(id: &'a str, model: &'a str) -> Self {
        Self {
            id,
            enabled: true,
            model: Some(model),
        }
    }
}

pub(crate) fn add_required_env(repo: &Path, manifest: &str, name: &str) {
    let path = repo.join(manifest);
    let mut manifest: CapabilityManifest = read_toml(&path);
    manifest
        .requires
        .get_or_insert_with(Requirements::default)
        .env
        .push(EnvVarRequirement::new(name).unwrap());
    write_toml(&path, &manifest);
}

pub(crate) fn add_required_capability(repo: &Path, manifest: &str, id: &str) {
    let path = repo.join(manifest);
    let mut manifest: CapabilityManifest = read_toml(&path);
    manifest
        .requires
        .get_or_insert_with(Requirements::default)
        .capabilities
        .push(id.parse::<CapabilityId>().unwrap());
    write_toml(&path, &manifest);
}

pub(crate) fn add_capability_file_mapping(repo: &Path, manifest: &str, key: &str, value: &str) {
    let path = repo.join(manifest);
    let mut manifest: CapabilityManifest = read_toml(&path);
    manifest
        .files
        .entries
        .insert(key.to_string(), value.to_string());
    write_toml(&path, &manifest);
}

pub(crate) fn remove_profile_capability(repo: &Path, manifest: &str, id: &str) {
    let path = repo.join(manifest);
    let mut manifest: ProfileManifest = read_toml(&path);
    let id = id.parse::<CapabilityId>().unwrap();
    manifest.capabilities.retain(|capability| capability != &id);
    write_toml(&path, &manifest);
}

pub(crate) fn set_profile_scope(repo: &Path, manifest: &str, scope: ScopeConstraints) {
    let path = repo.join(manifest);
    let mut manifest: ProfileManifest = read_toml(&path);
    manifest.scope = Some(scope);
    write_toml(&path, &manifest);
}

pub(crate) fn set_profile_runtimes(
    repo: &Path,
    manifest: &str,
    default: Option<&str>,
    runtimes: &[ProfileRuntimeFixture<'_>],
) {
    let path = repo.join(manifest);
    let mut manifest: ProfileManifest = read_toml(&path);
    let entries = runtimes
        .iter()
        .map(|runtime| {
            (
                runtime.id.parse::<RuntimeId>().unwrap(),
                ProfileRuntimeManifest {
                    enabled: runtime.enabled,
                    model: runtime.model.map(str::to_string),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    manifest.runtimes = Some(ProfileRuntimesManifest {
        default: default.map(|runtime| runtime.parse::<RuntimeId>().unwrap()),
        entries,
    });
    write_toml(&path, &manifest);
}

pub(crate) fn set_capability_runtime_support(
    repo: &Path,
    manifest: &str,
    runtime: &str,
    supported: bool,
) {
    let path = repo.join(manifest);
    let mut manifest: CapabilityManifest = read_toml(&path);
    manifest.runtimes.entries.insert(
        runtime.parse::<RuntimeId>().unwrap(),
        CapabilityRuntimeManifest { supported },
    );
    write_toml(&path, &manifest);
}

fn read_toml<T>(path: &Path) -> T
where
    T: serde::de::DeserializeOwned,
{
    toml::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_toml<T>(path: &Path, manifest: &T)
where
    T: serde::Serialize,
{
    fs::write(path, toml::to_string_pretty(manifest).unwrap()).unwrap();
}
