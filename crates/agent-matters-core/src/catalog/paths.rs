//! Catalog path constants shared by loaders, doctor, and compiler code.

use std::{
    fs, io,
    path::{Component, Path},
};

use crate::config::REPO_DEFAULTS_DIR_NAME;
use crate::domain::CapabilityKind;

pub const CATALOG_DIR_NAME: &str = "catalog";
pub const PROFILES_DIR_NAME: &str = "profiles";
pub const VENDOR_DIR_NAME: &str = "vendor";
pub const OVERLAYS_DIR_NAME: &str = "overlays";
pub const DEFAULTS_DIR_NAME: &str = REPO_DEFAULTS_DIR_NAME;
pub const MANIFEST_FILE_NAME: &str = "manifest.toml";

pub const CAPABILITY_SKILLS_DIR_NAME: &str = "skills";
pub const CAPABILITY_MCP_DIR_NAME: &str = "mcp";
pub const CAPABILITY_HOOKS_DIR_NAME: &str = "hooks";
pub const CAPABILITY_INSTRUCTIONS_DIR_NAME: &str = "instructions";
pub const CAPABILITY_AGENTS_DIR_NAME: &str = "agents";
pub const CAPABILITY_RUNTIME_SETTINGS_DIR_NAME: &str = "runtime-settings";

pub const fn capability_kind_dir_name(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Skill => CAPABILITY_SKILLS_DIR_NAME,
        CapabilityKind::Mcp => CAPABILITY_MCP_DIR_NAME,
        CapabilityKind::Hook => CAPABILITY_HOOKS_DIR_NAME,
        CapabilityKind::Instruction => CAPABILITY_INSTRUCTIONS_DIR_NAME,
        CapabilityKind::Agent => CAPABILITY_AGENTS_DIR_NAME,
        CapabilityKind::RuntimeSetting => CAPABILITY_RUNTIME_SETTINGS_DIR_NAME,
    }
}

pub const fn known_capability_dir_names() -> &'static [&'static str] {
    &[
        CAPABILITY_SKILLS_DIR_NAME,
        CAPABILITY_MCP_DIR_NAME,
        CAPABILITY_HOOKS_DIR_NAME,
        CAPABILITY_INSTRUCTIONS_DIR_NAME,
        CAPABILITY_AGENTS_DIR_NAME,
        CAPABILITY_RUNTIME_SETTINGS_DIR_NAME,
    ]
}

pub fn path_is_in_repo_vendor(repo_root: &Path, path: &Path) -> bool {
    let vendor_root = repo_root.join(VENDOR_DIR_NAME);
    path_is_structurally_in_vendor(&vendor_root, path)
        && path_resolves_inside_repo_vendor(repo_root, &vendor_root, path)
}

fn path_is_structurally_in_vendor(vendor_root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(vendor_root) else {
        return false;
    };
    let mut components = relative.components().peekable();
    components.peek().is_some()
        && components.all(|component| matches!(component, Component::Normal(_)))
}

fn path_resolves_inside_repo_vendor(repo_root: &Path, vendor_root: &Path, path: &Path) -> bool {
    let Ok(canonical_vendor_root) = fs::canonicalize(vendor_root) else {
        return true;
    };
    match fs::canonicalize(repo_root) {
        Ok(canonical_repo_root) if !canonical_vendor_root.starts_with(&canonical_repo_root) => {
            return false;
        }
        _ => {}
    }
    let Some(existing_path) = nearest_existing_path(path) else {
        return false;
    };
    let Ok(canonical_existing_path) = fs::canonicalize(existing_path) else {
        return false;
    };
    canonical_existing_path.starts_with(&canonical_vendor_root)
}

fn nearest_existing_path(path: &Path) -> Option<&Path> {
    let mut candidate = Some(path);
    while let Some(path) = candidate {
        match fs::symlink_metadata(path) {
            Ok(_) => return Some(path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                candidate = path.parent();
            }
            Err(_) => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_kind_dirs_match_repository_shape() {
        assert_eq!(
            capability_kind_dir_name(CapabilityKind::Skill),
            CAPABILITY_SKILLS_DIR_NAME
        );
        assert_eq!(
            capability_kind_dir_name(CapabilityKind::Mcp),
            CAPABILITY_MCP_DIR_NAME
        );
        assert_eq!(
            capability_kind_dir_name(CapabilityKind::Hook),
            CAPABILITY_HOOKS_DIR_NAME
        );
        assert_eq!(
            capability_kind_dir_name(CapabilityKind::Instruction),
            CAPABILITY_INSTRUCTIONS_DIR_NAME
        );
        assert_eq!(
            capability_kind_dir_name(CapabilityKind::Agent),
            CAPABILITY_AGENTS_DIR_NAME
        );
        assert_eq!(
            capability_kind_dir_name(CapabilityKind::RuntimeSetting),
            CAPABILITY_RUNTIME_SETTINGS_DIR_NAME
        );
    }

    #[test]
    fn known_capability_dirs_cover_all_mvp_kinds() {
        assert_eq!(
            known_capability_dir_names().len(),
            CapabilityKind::all().len()
        );
    }

    #[test]
    fn vendor_paths_must_stay_structurally_inside_vendor_storage() {
        let repo_root = Path::new("/repo");

        assert!(path_is_in_repo_vendor(
            repo_root,
            Path::new("/repo/vendor/skills.sh/playwright")
        ));
        assert!(!path_is_in_repo_vendor(
            repo_root,
            Path::new("/repo/vendor")
        ));
        assert!(!path_is_in_repo_vendor(
            repo_root,
            Path::new("/repo/vendor/skills.sh/../../outside")
        ));
        assert!(!path_is_in_repo_vendor(
            repo_root,
            Path::new("/repo/outside")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn vendor_paths_must_resolve_inside_vendor_storage() {
        use std::os::unix::fs::symlink;

        let repo = tempfile::TempDir::new().unwrap();
        let vendor_source = repo.path().join("vendor/skills.sh");
        let outside = repo.path().join("outside");
        fs::create_dir_all(&vendor_source).unwrap();
        fs::create_dir_all(outside.join("playwright")).unwrap();
        symlink(&outside, vendor_source.join("escaped")).unwrap();

        assert!(!path_is_in_repo_vendor(
            repo.path(),
            &vendor_source.join("escaped/playwright")
        ));
        assert!(!path_is_in_repo_vendor(
            repo.path(),
            &vendor_source.join("escaped/missing")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_vendor_symlinks_are_not_accepted() {
        use std::os::unix::fs::symlink;

        let repo = tempfile::TempDir::new().unwrap();
        let vendor_source = repo.path().join("vendor/skills.sh");
        fs::create_dir_all(&vendor_source).unwrap();
        symlink(
            repo.path().join("outside-missing"),
            vendor_source.join("dangling"),
        )
        .unwrap();

        assert!(!path_is_in_repo_vendor(
            repo.path(),
            &vendor_source.join("dangling/playwright")
        ));
    }
}
