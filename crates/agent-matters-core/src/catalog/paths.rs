//! Catalog path constants shared by loaders, doctor, and compiler code.

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
}
