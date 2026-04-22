#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::catalog::catalog_index_path;
use agent_matters_core::domain::{
    CapabilityId, EnvVarRequirement, Requirements, RuntimeId, ScopeConstraints,
};
use agent_matters_core::manifest::{
    CapabilityManifest, CapabilityRuntimeManifest, ProfileManifest, ProfileRuntimeManifest,
    ProfileRuntimesManifest, ScopeEnforcement,
};
use assert_cmd::Command;
use tempfile::TempDir;

pub(crate) fn bin() -> Command {
    Command::cargo_bin("agent-matters").expect("cargo bin available in tests")
}

pub(crate) fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent-matters-capabilities/tests/fixtures")
        .join(relative)
}

pub(crate) fn copy_dir(from: &Path, to: &Path) {
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

pub(crate) fn valid_catalog_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

pub(crate) fn native_home_with_codex_auth() -> TempDir {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::write(home.path().join(".codex/auth.json"), br#"{"token":"test"}"#).unwrap();
    home
}

pub(crate) fn write_corrupt_catalog_index(state: &TempDir) -> PathBuf {
    let path = catalog_index_path(state.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{not valid json").unwrap();
    path
}

pub(crate) fn add_required_env(repo: &Path, manifest: &str, name: &str) {
    let path = repo.join(manifest);
    let raw = fs::read_to_string(&path).unwrap();
    let mut manifest: CapabilityManifest = toml::from_str(&raw).unwrap();
    manifest
        .requires
        .get_or_insert_with(empty_requirements)
        .env
        .push(EnvVarRequirement::new(name).unwrap());
    fs::write(path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
}

pub(crate) fn add_required_capability(repo: &Path, manifest: &str, id: &str) {
    let path = repo.join(manifest);
    let raw = fs::read_to_string(&path).unwrap();
    let mut manifest: CapabilityManifest = toml::from_str(&raw).unwrap();
    manifest
        .requires
        .get_or_insert_with(empty_requirements)
        .capabilities
        .push(id.parse::<CapabilityId>().unwrap());
    fs::write(path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
}

pub(crate) fn replace_profile_instruction(repo: &Path, manifest: &str, old: &str, new: &str) {
    let path = repo.join(manifest);
    let raw = fs::read_to_string(&path).unwrap();
    let mut manifest: ProfileManifest = toml::from_str(&raw).unwrap();
    let old = old.parse::<CapabilityId>().unwrap();
    let new = new.parse::<CapabilityId>().unwrap();
    let instruction = manifest
        .instructions
        .iter_mut()
        .find(|instruction| **instruction == old)
        .expect("profile instruction fixture exists");
    *instruction = new;
    fs::write(path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
}

pub(crate) fn set_profile_path_scope(
    repo: &Path,
    manifest: &str,
    paths: Vec<String>,
    enforcement: ScopeEnforcement,
) {
    let path = repo.join(manifest);
    let raw = fs::read_to_string(&path).unwrap();
    let mut manifest: ProfileManifest = toml::from_str(&raw).unwrap();
    manifest.scope = Some(ScopeConstraints {
        paths,
        github_repos: Vec::new(),
        enforcement,
    });
    fs::write(path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
}

pub(crate) fn set_profile_runtimes(repo: &Path, manifest: &str, runtimes: &[(&str, bool)]) {
    let path = repo.join(manifest);
    let raw = fs::read_to_string(&path).unwrap();
    let mut manifest: ProfileManifest = toml::from_str(&raw).unwrap();
    let default = manifest
        .runtimes
        .as_ref()
        .and_then(|runtimes| runtimes.default.clone());
    let entries = runtimes
        .iter()
        .map(|(runtime, enabled)| {
            (
                runtime.parse::<RuntimeId>().unwrap(),
                ProfileRuntimeManifest {
                    enabled: *enabled,
                    model: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    manifest.runtimes = Some(ProfileRuntimesManifest { default, entries });
    fs::write(path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
}

pub(crate) fn set_capability_runtime_support(
    repo: &Path,
    manifest: &str,
    runtime: &str,
    supported: bool,
) {
    let path = repo.join(manifest);
    let raw = fs::read_to_string(&path).unwrap();
    let mut manifest: CapabilityManifest = toml::from_str(&raw).unwrap();
    manifest.runtimes.entries.insert(
        runtime.parse::<RuntimeId>().unwrap(),
        CapabilityRuntimeManifest { supported },
    );
    fs::write(path, toml::to_string_pretty(&manifest).unwrap()).unwrap();
}

pub(crate) fn write_fake_skills_bin(dir: &TempDir) -> PathBuf {
    write_script(
        dir,
        "fake-skills",
        r#"#!/bin/sh
set -eu
case "$1" in
  find)
    if [ "${2:-}" = "none" ]; then
      printf 'No skills found for "none"\n'
      exit 0
    fi
    printf 'owner/repo@playwright 2 installs\n'
    printf '%s\n' '-> https://skills.sh/owner/repo/playwright'
    ;;
  add)
    skill=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "--skill" ]; then
        shift
        skill="$1"
      fi
      shift || true
    done
    mkdir -p ".agents/skills/$skill/docs"
    cat > ".agents/skills/$skill/SKILL.md" <<'SKILL'
---
name: playwright
description: Mock Playwright skill.
metadata:
  version: "2.0.0"
---

# Playwright
SKILL
    printf 'Details.\n' > ".agents/skills/$skill/docs/usage.md"
    printf 'installed\n'
    ;;
  *)
    printf 'unsupported command\n' >&2
    exit 2
    ;;
esac
"#,
    )
}

pub(crate) fn write_script(dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, contents).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn empty_requirements() -> Requirements {
    Requirements {
        capabilities: Vec::new(),
        env: Vec::new(),
    }
}
