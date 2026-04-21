//! Pure JSON index schema for exact catalog lookups.
//!
//! The index is generated state. It is safe to delete and rebuild from
//! authored TOML manifests.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CATALOG_INDEX_SCHEMA_VERSION: u16 = 4;
pub const INDEXES_DIR_NAME: &str = "indexes";
pub const CATALOG_INDEX_FILE_NAME: &str = "catalog.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogIndex {
    pub schema_version: u16,
    pub content_fingerprint: String,
    pub capabilities: BTreeMap<String, CapabilityIndexRecord>,
    pub profiles: BTreeMap<String, ProfileIndexRecord>,
}

impl CatalogIndex {
    pub fn new(
        content_fingerprint: impl Into<String>,
        capabilities: BTreeMap<String, CapabilityIndexRecord>,
        profiles: BTreeMap<String, ProfileIndexRecord>,
    ) -> Self {
        Self {
            schema_version: CATALOG_INDEX_SCHEMA_VERSION,
            content_fingerprint: content_fingerprint.into(),
            capabilities,
            profiles,
        }
    }

    pub fn capability(&self, id: &str) -> Option<&CapabilityIndexRecord> {
        self.capabilities.get(id)
    }

    pub fn profile(&self, id: &str) -> Option<&ProfileIndexRecord> {
        self.profiles.get(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityIndexRecord {
    pub id: String,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    pub source_path: String,
    pub source: CapabilitySourceSummary,
    pub runtimes: BTreeMap<String, RuntimeCompatibilitySummary>,
    pub provenance: ProvenanceSummary,
    pub requirements: RequirementSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileIndexRecord {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub capabilities: Vec<String>,
    pub instructions: Vec<String>,
    pub source_path: String,
    pub runtimes: BTreeMap<String, RuntimeCompatibilitySummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_runtime: Option<String>,
    pub capability_count: usize,
    pub instruction_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCompatibilitySummary {
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSummary {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySourceSummary {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_path: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementSummary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_exact_lookup_uses_record_ids() {
        let capability = CapabilityIndexRecord {
            id: "skill:playwright".to_string(),
            kind: "skill".to_string(),
            summary: "Browser automation".to_string(),
            files: BTreeMap::from([("source".to_string(), "SKILL.md".to_string())]),
            source_path: "catalog/skills/playwright".to_string(),
            source: CapabilitySourceSummary {
                kind: "local".to_string(),
                normalized_path: Some("catalog/skills/playwright".to_string()),
                overlay_path: None,
                vendor_path: None,
            },
            runtimes: BTreeMap::new(),
            provenance: ProvenanceSummary {
                kind: "local".to_string(),
                source: None,
                locator: None,
                version: None,
            },
            requirements: RequirementSummary::default(),
        };
        let profile = ProfileIndexRecord {
            id: "github-researcher".to_string(),
            kind: "persona".to_string(),
            summary: "GitHub research".to_string(),
            capabilities: vec!["skill:playwright".to_string()],
            instructions: vec!["instruction:helioy-core".to_string()],
            source_path: "profiles/github-researcher".to_string(),
            runtimes: BTreeMap::new(),
            default_runtime: Some("codex".to_string()),
            capability_count: 1,
            instruction_count: 1,
        };
        let index = CatalogIndex::new(
            "fnv64:1234",
            BTreeMap::from([(capability.id.clone(), capability)]),
            BTreeMap::from([(profile.id.clone(), profile)]),
        );

        assert!(index.capability("skill:playwright").is_some());
        assert!(index.profile("github-researcher").is_some());
        assert!(index.capability("skill:missing").is_none());
    }

    #[test]
    fn empty_summaries_omit_optional_fields() {
        let summary = RequirementSummary::default();

        let encoded = serde_json::to_value(summary).unwrap();

        assert_eq!(encoded, serde_json::json!({}));
    }
}
